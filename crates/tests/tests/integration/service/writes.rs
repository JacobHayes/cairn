//! Domain patches through the service, over the memory and SQLite stores alike (A11, A12,
//! A17, A19, B1, B11, D7, E6, H2, H3, H5, H6, Journey durability).
#![cfg(test)]

use crate::service::support;

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cairn_engine::Records;
use cairn_schema::{
    Domain, EventType, JourneyId, Rejection, RevisionOf, Subject, TouchedSet, ViolationCode,
};
use cairn_service::{DomainPatch, Service, WriteError, Written};
use cairn_store::{EventQuery, InProcessNotifier, Take, Tick, Watch};

use support::{
    applied, call, domain, engine_step, patch, publish_fixture_route, rejected, scenario,
    step_call, step_patch, vendor_after,
};

/// Runs each case once over a fresh memory store and once over a fresh SQLite store.
macro_rules! on_both_stores {
    ($($case:ident),* $(,)?) => {
        mod memory {
            $(
                #[test]
                fn $case() {
                    let (service, notifier) = crate::service::support::memory();
                    crate::service::support::run(super::$case(&service, &notifier));
                }
            )*
        }
        mod sqlite {
            $(
                #[test]
                fn $case() {
                    crate::service::support::run(async {
                        let (service, notifier) = crate::service::support::sqlite(crate::service::writes::TMP.as_ref());
                        super::$case(&service, &notifier).await;
                    });
                }
            )*
        }
    };
}

on_both_stores!(
    a_fixture_scenario_lands_as_the_engine_alone_would,
    a_resubmission_after_another_patch_is_answered_from_its_receipt,
    a_subtree_removal_conflicts_with_an_edit_inside_it,
    a_removal_made_stale_by_an_insertion_beneath_it_is_not_widened,
    a_stale_role_removal_never_clears_a_fill_made_meanwhile,
    a_lost_answer_after_a_write_applied_over_another_is_answered_from_its_receipt,
    a_write_is_applied_over_a_deployment_change_that_moved_only_the_deployment,
    a_merge_beside_an_unrelated_deployment_change_lands_over_it,
    a_merge_that_breaks_a_referencing_journey_is_rejected_naming_it,
    a_merge_moves_emails_and_an_email_stays_on_one_identity,
    a_merge_is_stale_once_a_journey_it_checked_moves_or_a_new_one_refers,
    an_identity_is_deleted_only_once_nothing_refers_to_it,
    an_unreferenced_identity_is_deleted_with_its_emails,
    a_group_merge_folds_several_identities_into_one_survivor,
    a_route_naming_an_identity_follows_its_merge_and_keeps_the_survivor_from_deletion,
    a_delete_rejection_lists_what_refers_up_to_its_limit_and_counts_the_rest,
    a_merge_then_delete_rejection_counts_each_omitted_reference_once,
    deletes_sharing_their_references_each_count_the_remainder_once,
    a_route_with_many_versions_naming_an_identity_is_one_reference,
    deletes_naming_different_graphs_of_one_route_each_name_the_route,
    a_group_merge_resubmitted_with_other_identities_is_another_merge,
    publishing_a_route_never_touches_its_journeys,
    an_archived_journey_takes_only_a_status_move_or_deletion_and_its_id_never_returns,
    a_route_a_journey_follows_on_an_old_version_is_deleted_only_with_it,
    a_segment_inserted_only_in_a_published_version_is_not_deleted,
    a_users_patch_and_their_agents_land_alike_but_for_the_stamp,
    every_commit_is_announced_with_each_revision_it_moved,
    consequences_report_what_the_patch_caused_and_not_what_midnight_did,
    a_hard_delete_announces_the_deployment_and_each_journey_it_deleted,
);

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the SQLite cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

/// The events a scenario's patches emit: one per mutation (J2).
fn scenario_event_count(steps: &[cairn_schema::ScenarioStep]) -> usize {
    steps.iter().map(|step| step.patch.mutations.len()).sum()
}

fn journey_id(id: &str) -> JourneyId {
    id.parse().unwrap()
}

/// Creates an empty journey `id` with the nodes given (as YAML flow mappings), at revision 1.
async fn create(service: &Service, id: &str, nodes: &[&str]) {
    let mut mutations = String::from("- op: create_journey\n  name: Test\n");
    for node in nodes {
        mutations += "- op: add_node\n  node: ";
        mutations += node;
        mutations += "\n";
    }
    let created = patch(
        &format!("p_create_{id}"),
        &format!("{{journey: {id}}}"),
        0,
        &mutations,
    );
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
}

fn codes(rejection: &Rejection) -> Vec<ViolationCode> {
    match rejection {
        Rejection::Invalid { violations } => violations.as_slice().iter().map(|v| v.code).collect(),
        other => panic!("expected an invalid rejection, got {other:#?}"),
    }
}

/// A17, B1, H2: the vendor evaluation's route published and its whole scenario submitted
/// through the service leave the store holding exactly what the engine alone produces, with
/// every event stamped with its step's actor.
async fn a_fixture_scenario_lands_as_the_engine_alone_would(
    service: &Service,
    _: &InProcessNotifier,
) {
    let (people_call, people) = crate::people::people_write();
    applied(service.patch(&people_call, &people).await);
    let people_step = crate::people::people_step();
    let mut reference = engine_step(
        &Records::default(),
        &people_step.patch,
        &people_call,
        Some(people_step.note.as_str()),
    );
    let seed = publish_fixture_route("vendor-evaluation");
    let seed_call = call("u_author", "2026-09-01T12:00:00Z");
    applied(service.patch(&seed_call, &domain(seed.clone())).await);
    reference = engine_step(&reference, &seed, &seed_call, None);
    let steps = scenario("vendor-evaluation").steps;
    for step in steps.as_slice() {
        let call = step_call(step);
        assert_eq!(service.settings().today(call.now), step.today);
        applied(service.patch(&call, &step_patch(step)).await);
        reference = engine_step(&reference, &step.patch, &call, Some(step.note.as_str()));
    }
    let id = journey_id("j_vendor_eval");
    assert_eq!(
        service.journey(&id).await.unwrap().as_ref(),
        reference.journeys.get(&id)
    );
    assert_eq!(service.deployment().await.unwrap(), reference.deployment);
    let query = EventQuery {
        log: Some(Domain::Journey(id)),
        ..EventQuery::default()
    };
    let events = service.events(&query).await.unwrap().items;
    assert_eq!(events.len(), scenario_event_count(steps.as_slice()));
    for logged in events {
        let step = steps
            .as_slice()
            .iter()
            .find(|step| step.patch.id == logged.event.patch_id)
            .unwrap();
        assert_eq!(logged.event.actor, step.actor, "H2");
    }
}

/// H5: a lost response resubmitted by patch id, after another patch advanced the journey,
/// is answered from its receipt and never re-applied; the id with other content is refused.
async fn a_resubmission_after_another_patch_is_answered_from_its_receipt(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(service, "j_h5", &[]).await;
    let first = patch(
        "p_first",
        "{journey: j_h5}",
        1,
        "- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n",
    );
    let original = applied(
        service
            .patch(&call("u_lead", AT), &domain(first.clone()))
            .await,
    );
    let second = patch(
        "p_second",
        "{journey: j_h5}",
        2,
        "- op: add_node\n  node: {key: n_b, id: b, kind: action, title: B}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(second)).await);

    let again = service.patch(&call("u_lead", AT), &domain(first)).await;
    assert_eq!(
        again,
        Ok(Written::AlreadyApplied {
            receipt: original.receipt().clone()
        })
    );
    let journey = service.journey(&journey_id("j_h5")).await.unwrap().unwrap();
    assert_eq!(journey.revision.get(), 3, "nothing was re-applied");
    assert_eq!(journey.graph.nodes.len(), 2);

    let reused = patch(
        "p_first",
        "{journey: j_h5}",
        3,
        "- op: add_node\n  node: {key: n_c, id: c, kind: action, title: C}\n",
    );
    let answer = rejected(service.patch(&call("u_lead", AT), &domain(reused)).await);
    assert!(
        matches!(answer, Rejection::PatchIdReused { .. }),
        "{answer:#?}"
    );
}

const GROUP: &str = "{key: n_g, id: g, kind: group, title: G}";
const CHILD: &str = "{key: n_c, id: c, kind: action, title: C, parent: n_g}";
const REMOVAL: &str = "- op: remove_node\n  removal: {node: n_g, descendants: [n_c]}\n";

/// H5: a subtree removal drafted at revision 1 is stale once an edit inside the subtree
/// lands; the rejection carries what the edit touched, which overlaps what the removal
/// would write, so the server answers it stale.
async fn a_subtree_removal_conflicts_with_an_edit_inside_it(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(service, "j_tree", &[GROUP, CHILD]).await;
    let edit = patch(
        "p_edit",
        "{journey: j_tree}",
        1,
        "- op: set_node_field\n  node: n_c\n  value: {title: Renamed}\n",
    );
    applied(service.patch(&call("u_other", AT), &domain(edit)).await);
    let removal = patch("p_remove", "{journey: j_tree}", 1, REMOVAL);
    let Rejection::Stale {
        conflicts,
        intervening,
        touched,
    } = rejected(service.patch(&call("u_lead", AT), &domain(removal)).await)
    else {
        panic!("the removal is stale");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].of,
        RevisionOf::Domain(Domain::Journey(journey_id("j_tree")))
    );
    assert_eq!(
        (conflicts[0].expected.get(), conflicts[0].current.get()),
        (1, 2)
    );
    assert!(!intervening.as_set().is_empty());
    assert!(intervening.overlaps(&touched), "not safe to apply past");
}

/// H5, A18: when what intervened is an insertion beneath the subtree, the removal would not
/// apply at the new revision (it names only what its author saw), so the stale answer says
/// it touches the whole journey and the server answers it stale; resubmitted there by hand,
/// it is rejected rather than widened to the new node.
async fn a_removal_made_stale_by_an_insertion_beneath_it_is_not_widened(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(service, "j_tree", &[GROUP, CHILD]).await;
    let insert = patch(
        "p_insert",
        "{journey: j_tree}",
        1,
        "- op: add_node\n  node: {key: n_d, id: d, kind: action, title: D, parent: n_g}\n",
    );
    applied(service.patch(&call("u_other", AT), &domain(insert)).await);
    let removal = patch("p_remove", "{journey: j_tree}", 1, REMOVAL);
    let Rejection::Stale { touched, .. } =
        rejected(service.patch(&call("u_lead", AT), &domain(removal)).await)
    else {
        panic!("the removal is stale");
    };
    let journey = Domain::Journey(journey_id("j_tree"));
    assert_eq!(
        touched,
        TouchedSet::whole(journey),
        "never applied past the insertion"
    );
    let retry = patch("p_remove_retry", "{journey: j_tree}", 2, REMOVAL);
    let refused = rejected(service.patch(&call("u_lead", AT), &domain(retry)).await);
    assert!(
        codes(&refused).contains(&ViolationCode::RemovalWidened),
        "{refused:#?}"
    );
    let journey = service
        .journey(&journey_id("j_tree"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(journey.graph.nodes.len(), 3, "nothing was removed");
}

/// The answer to a role removal drafted at revision 1 after a fill moved the journey to 2:
/// applied past the fill when it was another role's, stale when it was the removed role's.
fn removal_answer_is(journey: &str, lands: bool, answer: Result<Written, WriteError>) {
    if lands {
        let Ok(Written::Applied {
            receipt,
            over: Some(over),
            ..
        }) = answer
        else {
            panic!("{journey}: applied over the fill, got {answer:#?}");
        };
        assert_eq!(receipt.revision.get(), 3, "{journey}");
        let conflicts: Vec<_> = over
            .conflicts
            .iter()
            .map(|conflict| {
                (
                    &conflict.of,
                    conflict.expected.get(),
                    conflict.current.get(),
                )
            })
            .collect();
        assert_eq!(
            conflicts,
            [(
                &RevisionOf::Domain(Domain::Journey(journey_id(journey))),
                1,
                2
            )],
            "{journey}"
        );
    } else {
        assert!(
            matches!(rejected(answer), Rejection::Stale { .. }),
            "{journey}"
        );
    }
}

/// H5: a role removal drafted while the role was empty is stale once someone fills it. What
/// the removal would write now (the fill too) comes from applying it, so the server answers
/// it stale instead of applying a removal that silently clears the fill; when the fill was
/// another role's, it applies the removal over the fill, which stays.
async fn a_stale_role_removal_never_clears_a_fill_made_meanwhile(
    service: &Service,
    _: &InProcessNotifier,
) {
    let (people_call, people) = crate::people::people_write();
    applied(service.patch(&people_call, &people).await);
    let deployment = service.deployment().await.unwrap().revision;
    for (journey, filled, lands) in [("j_same", "r_lead", false), ("j_other", "r_alt", true)] {
        let target = format!("{{journey: {journey}}}");
        let roles = "\
- op: create_journey\n  name: Roles\n\
- op: add_role\n  role: {key: r_lead, id: lead, title: Lead}\n\
- op: add_role\n  role: {key: r_alt, id: alt, title: Alt}\n";
        let created = patch(&format!("p_create_{journey}"), &target, 0, roles);
        applied(service.patch(&call("u_lead", AT), &domain(created)).await);
        let fill =
            format!("- op: fill_role\n  role: {filled}\n  identities: [who_e7h2jv3nxc4yaw6t]\n");
        let fill = cairn_schema::Patch {
            deployment_revision: Some(deployment),
            ..patch(&format!("p_fill_{journey}"), &target, 1, &fill)
        };
        applied(service.patch(&call("u_other", AT), &domain(fill)).await);
        let removal = patch(
            &format!("p_remove_{journey}"),
            &target,
            1,
            "- op: remove_role\n  role: r_lead\n",
        );
        let answer = service.patch(&call("u_lead", AT), &domain(removal)).await;
        removal_answer_is(journey, lands, answer);
        let held = service
            .journey(&journey_id(journey))
            .await
            .unwrap()
            .unwrap();
        let fills: Vec<String> = held
            .graph
            .state
            .role_fills
            .keys()
            .map(ToString::to_string)
            .collect();
        assert_eq!(fills, [filled], "{journey}: the fill made meanwhile stays");
    }
}

/// H5: a write applied over another's, whose answer is lost, is answered from its receipt
/// when resubmitted unchanged (the receipt records the patch as submitted, not as moved),
/// and nothing is applied twice.
async fn a_lost_answer_after_a_write_applied_over_another_is_answered_from_its_receipt(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(service, "j_lost", &[]).await;
    let node = |id: &str, key: &str| {
        patch(
            id,
            "{journey: j_lost}",
            1,
            &format!("- op: add_node\n  node: {{key: {key}, id: {key}, kind: action, title: T}}\n"),
        )
    };
    applied(
        service
            .patch(&call("u_other", AT), &domain(node("p_b", "n_b")))
            .await,
    );
    let mine = domain(node("p_a", "n_a"));
    let Written::Applied { receipt, over, .. } =
        applied(service.patch(&call("u_lead", AT), &mine).await)
    else {
        unreachable!("applied");
    };
    assert_eq!(receipt.revision.get(), 3);
    assert!(over.is_some(), "it was applied over the other write");
    let again = service.patch(&call("u_lead", AT), &mine).await;
    assert_eq!(again, Ok(Written::AlreadyApplied { receipt }));
    let journey = service
        .journey(&journey_id("j_lost"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(journey.revision.get(), 3, "nothing was applied twice");
    assert_eq!(journey.graph.nodes.len(), 2);
}

/// H5, E6: a journey write that names the deployment revision it was checked against is
/// applied past an unrelated identity change that moved only the deployment: the journey has
/// not moved, the answer names only the deployment, and the commit holds the current one.
async fn a_write_is_applied_over_a_deployment_change_that_moved_only_the_deployment(
    service: &Service,
    _: &InProcessNotifier,
) {
    let (people_call, people) = crate::people::people_write();
    applied(service.patch(&people_call, &people).await);
    let deployment = service.deployment().await.unwrap().revision;
    let roles = "\
- op: create_journey\n  name: Roles\n\
- op: add_role\n  role: {key: r_lead, id: lead, title: Lead}\n";
    let created = patch("p_create_j_dep", "{journey: j_dep}", 0, roles);
    applied(service.patch(&call("u_lead", AT), &domain(created)).await);
    let unrelated = patch(
        "p_unrelated",
        "deployment",
        deployment.get(),
        "- op: create_identity\n  identity: {key: who_third, name: Third}\n",
    );
    applied(
        service
            .patch(&call("u_other", AT), &domain(unrelated))
            .await,
    );
    let fill = cairn_schema::Patch {
        deployment_revision: Some(deployment),
        ..patch(
            "p_fill",
            "{journey: j_dep}",
            1,
            "- op: fill_role\n  role: r_lead\n  identities: [who_e7h2jv3nxc4yaw6t]\n",
        )
    };
    let Written::Applied { receipt, over, .. } =
        applied(service.patch(&call("u_lead", AT), &domain(fill)).await)
    else {
        unreachable!("applied");
    };
    assert_eq!(receipt.revision.get(), 2, "the journey had not moved");
    let over = over.expect("the deployment moved");
    let moved: Vec<_> = over
        .conflicts
        .iter()
        .map(|conflict| (&conflict.of, conflict.expected, conflict.current))
        .collect();
    assert_eq!(
        moved,
        [(
            &RevisionOf::Domain(Domain::Deployment),
            deployment,
            service.deployment().await.unwrap().revision
        )]
    );
}

/// E6, H5: a merge drafted against the journeys that refer to its identities lands beside an
/// unrelated change to the deployment, which the engine checks it against again at apply.
async fn a_merge_beside_an_unrelated_deployment_change_lands_over_it(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let deployment = service.deployment().await.unwrap().revision;
    let unrelated = patch(
        "p_unrelated",
        "deployment",
        deployment.get(),
        "- op: create_identity\n  identity: {key: who_third, name: Third}\n",
    );
    applied(
        service
            .patch(&call("u_other", AT), &domain(unrelated))
            .await,
    );
    let merge = patch(
        "p_merge",
        "deployment",
        deployment.get(),
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 2}\n",
    );
    let Written::Applied { over, .. } =
        applied(service.patch(&call("u_lead", AT), &domain(merge)).await)
    else {
        unreachable!("applied");
    };
    let over = over.expect("the deployment moved");
    assert_eq!(over.conflicts.len(), 1);
    let identities = service.deployment().await.unwrap().identities;
    assert!(
        identities
            .get(&"who_e7h2jv3nxc4yaw6t".parse().unwrap())
            .is_none()
    );
}

/// E6: a merge whose aliases make a contradictory chain apply in a referencing journey is
/// rejected, naming the journey.
async fn a_merge_that_breaks_a_referencing_journey_is_rejected_naming_it(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let prepare = patch(
        "p_prepare",
        "{journey: j_vendor_eval}",
        2,
        "\
- op: create_identity\n  identity: {key: who_other, name: Other}\n\
- op: add_node\n  node: {key: n_start, id: start, kind: milestone, title: Start}\n\
- op: set_pin\n  node: n_start\n  date: \"2026-11-10\"\n\
- op: add_node\n  node: {key: n_long, id: long, kind: action, title: Long, estimate: 30, relevant_when: {equals: {decision: n_who_owns, value: who_other}}, not_before: {after: n_start}, due_by: {before: n_decision_meeting}}\n",
    );
    applied(
        service
            .patch(&call("u_lead", "2026-10-02T12:00:00Z"), &domain(prepare))
            .await,
    );
    let merge = patch(
        "p_merge",
        "deployment",
        2,
        "- op: merge_identities\n  survivor: who_other\n  merged: [who_k3d7qm2xw5tn6pfa]\n  journeys: {j_vendor_eval: 3}\n",
    );
    let refused = rejected(
        service
            .patch(&call("u_lead", "2026-10-02T12:00:00Z"), &domain(merge))
            .await,
    );
    let Rejection::Invalid { violations } = &refused else {
        panic!("{refused:#?}");
    };
    let breaks = violations
        .as_slice()
        .iter()
        .find(|found| found.code == ViolationCode::MergeBreaksJourney)
        .unwrap();
    assert!(
        breaks
            .related
            .contains(&Subject::Journey(journey_id("j_vendor_eval")))
    );
    assert_eq!(
        service.deployment().await.unwrap().revision.get(),
        2,
        "nothing committed"
    );
}

/// H3, E6: a merge moves the merged identity's emails to the survivor and leaves its key as an
/// alias; an email already on another identity cannot be given to a second one.
async fn a_merge_moves_emails_and_an_email_stays_on_one_identity(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let taken = patch(
        "p_taken",
        "deployment",
        1,
        "- op: edit_identity\n  identity: {key: who_e7h2jv3nxc4yaw6t, name: Eli Park, emails: [Ana@Example.org]}\n",
    );
    let refused = rejected(service.patch(&call("u_lead", AT), &domain(taken)).await);
    assert!(matches!(refused, Rejection::Invalid { .. }), "{refused:#?}");

    let give = patch(
        "p_give",
        "deployment",
        1,
        "- op: edit_identity\n  identity: {key: who_e7h2jv3nxc4yaw6t, name: Eli Park, emails: [b@example.org]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(give)).await);
    let merge = patch(
        "p_merge",
        "deployment",
        2,
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 2}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(merge)).await);
    let deployment = service.deployment().await.unwrap();
    let survivor_key = "who_d4m6uk2eqf5rbhzs".parse().unwrap();
    assert_eq!(
        deployment
            .aliases
            .get(&"who_e7h2jv3nxc4yaw6t".parse().unwrap()),
        Some(&survivor_key),
        "the alias resolves"
    );
    let survivor = deployment.identities.get(&survivor_key).unwrap();
    assert!(survivor.emails.contains(&"b@example.org".parse().unwrap()));
}

/// E6, H5: a merge names the journeys it was checked against at their revisions; one of them
/// edited since, or a journey newly referring to either identity, makes it stale.
async fn a_merge_is_stale_once_a_journey_it_checked_moves_or_a_new_one_refers(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let merge = |id: &str| {
        domain(patch(
            id,
            "deployment",
            1,
            "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 2}\n",
        ))
    };
    let edit = patch(
        "p_edit",
        "{journey: j_vendor_eval}",
        2,
        "- op: edit_journey\n  name: Renamed\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(edit)).await);
    let Rejection::Stale { conflicts, .. } = rejected(
        service
            .patch(&call("u_lead", AT), &merge("p_merge_one"))
            .await,
    ) else {
        panic!("a journey it checked moved");
    };
    let moved = RevisionOf::Domain(Domain::Journey(journey_id("j_vendor_eval")));
    assert!(conflicts.iter().any(|conflict| conflict.of == moved));

    let refer = patch(
        "p_refer",
        "{journey: j_new}",
        0,
        "\
- op: create_journey\n  name: New\n\
- op: add_role\n  role: {key: r_lead, id: lead, title: Lead}\n\
- op: fill_role\n  role: r_lead\n  identities: [who_e7h2jv3nxc4yaw6t]\n",
    );
    let refer = cairn_schema::Patch {
        deployment_revision: Some(cairn_schema::Revision::NONE.next()),
        ..refer
    };
    applied(service.patch(&call("u_lead", AT), &domain(refer)).await);
    let fresh = domain(patch(
        "p_merge_two",
        "deployment",
        1,
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 3}\n",
    ));
    let Rejection::Stale { conflicts, .. } =
        rejected(service.patch(&call("u_lead", AT), &fresh).await)
    else {
        panic!("a journey newly refers to a merged identity");
    };
    let newcomer = RevisionOf::Domain(Domain::Journey(journey_id("j_new")));
    assert!(
        conflicts.iter().any(|conflict| conflict.of == newcomer),
        "{conflicts:#?}"
    );
}

/// E6, A19: the references read names the journeys that refer to an identity, directly or
/// through an alias, and deleting an identity something refers to is rejected naming them.
async fn an_identity_is_deleted_only_once_nothing_refers_to_it(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let eli = "who_e7h2jv3nxc4yaw6t".parse().unwrap();
    let ana = "who_d4m6uk2eqf5rbhzs".parse().unwrap();
    let journeys = |references: Option<cairn_service::IdentityReferences>| {
        references
            .unwrap()
            .journeys
            .into_iter()
            .map(|journey| journey.id.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        journeys(service.identity_references(&eli).await.unwrap()),
        ["j_vendor_eval"]
    );
    assert!(
        service
            .identity_references(&"who_nobody".parse().unwrap())
            .await
            .unwrap()
            .is_none()
    );

    let revision = service.deployment().await.unwrap().revision.get();
    let refused = rejected(
        service
            .patch(
                &call("u_lead", AT),
                &domain(patch(
                    "p_delete_eli",
                    "deployment",
                    revision,
                    "- op: delete_identity\n  identity: who_e7h2jv3nxc4yaw6t\n",
                )),
            )
            .await,
    );
    let Rejection::Invalid { violations } = &refused else {
        panic!("{refused:#?}")
    };
    assert_eq!(violations.as_slice()[0].code, ViolationCode::IdentityInUse);
    assert!(
        violations.as_slice()[0]
            .related
            .contains(&Subject::Journey(journey_id("j_vendor_eval"))),
        "the rejection names the journey"
    );

    let merge = patch(
        "p_merge_eli",
        "deployment",
        revision,
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {j_vendor_eval: 2}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(merge)).await);
    assert!(
        journeys(service.identity_references(&ana).await.unwrap())
            .contains(&"j_vendor_eval".to_owned()),
        "the survivor is referred to through the merged key's alias"
    );
}

/// A19, E6: deleting an identity nothing refers to removes it with its emails, logs the delete,
/// and frees the address.
async fn an_unreferenced_identity_is_deleted_with_its_emails(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let revision = service.deployment().await.unwrap().revision.get();
    let spare = patch(
        "p_spare",
        "deployment",
        revision,
        "- op: create_identity\n  identity: {key: who_u5k3mq7wd2fz6ncx, name: Spare, emails: [spare@example.org]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(spare)).await);
    let revision = service.deployment().await.unwrap().revision.get();
    let delete = patch(
        "p_delete_spare",
        "deployment",
        revision,
        "- op: delete_identity\n  identity: who_u5k3mq7wd2fz6ncx\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(delete)).await);
    let query = EventQuery {
        log: Some(Domain::Deployment),
        ..EventQuery::default()
    };
    let logged = service.events(&query).await.unwrap().items;
    assert!(
        logged
            .iter()
            .any(|entry| entry.event.event_type == EventType::IdentityDeleted)
    );
    let deployment = service.deployment().await.unwrap();
    assert!(
        deployment
            .identities
            .get(&"who_u5k3mq7wd2fz6ncx".parse().unwrap())
            .is_none()
    );
    let reuse = patch(
        "p_reuse",
        "deployment",
        deployment.revision.get(),
        "- op: create_identity\n  identity: {key: who_p4n7rk2xc6vh3dmf, name: Again, emails: [spare@example.org]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(reuse)).await);
}

/// E6: one merge folds several identities into a survivor; every journey that refers to any of
/// them is named, and each merged key becomes an alias of the survivor.
async fn a_group_merge_folds_several_identities_into_one_survivor(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let revision = service.deployment().await.unwrap().revision.get();
    let spare = patch(
        "p_spare",
        "deployment",
        revision,
        "- op: create_identity\n  identity: {key: who_u5k3mq7wd2fz6ncx, name: Spare, emails: [spare@example.org]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(spare)).await);
    let revision = service.deployment().await.unwrap().revision.get();
    let merge = patch(
        "p_group",
        "deployment",
        revision,
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t, who_u5k3mq7wd2fz6ncx]\n  journeys: {j_vendor_eval: 2}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(merge)).await);
    let deployment = service.deployment().await.unwrap();
    let survivor: cairn_schema::IdentityKey = "who_d4m6uk2eqf5rbhzs".parse().unwrap();
    for merged in ["who_e7h2jv3nxc4yaw6t", "who_u5k3mq7wd2fz6ncx"] {
        let merged = merged.parse().unwrap();
        assert!(deployment.identities.get(&merged).is_none());
        assert_eq!(deployment.aliases.get(&merged), Some(&survivor));
    }
    assert!(
        deployment
            .identities
            .get(&survivor)
            .unwrap()
            .emails
            .contains(&"spare@example.org".parse().unwrap())
    );
}

/// H5, E6: a group merge resubmitted under its patch id with only some of its identities is not
/// the merge that id committed.
async fn a_group_merge_resubmitted_with_other_identities_is_another_merge(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let revision = service.deployment().await.unwrap().revision.get();
    let spare = patch(
        "p_spare",
        "deployment",
        revision,
        "- op: create_identity\n  identity: {key: who_u5k3mq7wd2fz6ncx, name: Spare}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(spare)).await);
    let revision = service.deployment().await.unwrap().revision.get();
    let merge = |merged: &str| {
        domain(patch(
            "p_group",
            "deployment",
            revision,
            &format!(
                "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [{merged}]\n  journeys: {{j_vendor_eval: 2}}\n"
            ),
        ))
    };
    let both = "who_e7h2jv3nxc4yaw6t, who_u5k3mq7wd2fz6ncx";
    applied(service.patch(&call("u_lead", AT), &merge(both)).await);
    let again = service
        .patch(&call("u_lead", AT), &merge(both))
        .await
        .unwrap();
    assert!(
        matches!(again, cairn_service::Written::AlreadyApplied { .. }),
        "{again:?}"
    );
    let fewer = rejected(
        service
            .patch(&call("u_lead", AT), &merge("who_e7h2jv3nxc4yaw6t"))
            .await,
    );
    assert!(
        matches!(fewer, Rejection::PatchIdReused { .. }),
        "{fewer:?}"
    );
}

/// Creates `count` journeys that each fill a role with the identities `who_e7h2jv3nxc4yaw6t` and
/// `who_d4m6uk2eqf5rbhzs`, and
/// answers the deployment revision they were created against.
async fn refer_to_e7h2(service: &Service, count: usize) -> cairn_schema::Revision {
    let deployment = service.deployment().await.unwrap().revision;
    for at in 0..count {
        let refer = patch(
            &format!("p_refer_{at}"),
            &format!("{{journey: j_ref_{at}}}"),
            0,
            "\
- op: create_journey\n  name: Refers\n\
- op: add_role\n  role: {key: r_lead, id: lead, title: Lead}\n\
- op: fill_role\n  role: r_lead\n  identities: [who_e7h2jv3nxc4yaw6t]\n\
- op: add_role\n  role: {key: r_alt, id: alt, title: Alt}\n\
- op: fill_role\n  role: r_alt\n  identities: [who_d4m6uk2eqf5rbhzs]\n",
        );
        let refer = cairn_schema::Patch {
            deployment_revision: Some(deployment),
            ..refer
        };
        applied(service.patch(&call("u_lead", AT), &domain(refer)).await);
    }
    deployment
}

/// E6: a patch that merges an identity and then deletes its survivor counts what a rejection
/// leaves out once: the journeys the merge loaded are not counted again as unlisted.
async fn a_merge_then_delete_rejection_counts_each_omitted_reference_once(
    service: &Service,
    _: &InProcessNotifier,
) {
    let limit = cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let extra = 3;
    vendor_after(service, 2).await;
    let deployment = refer_to_e7h2(service, limit + extra).await;
    let mut named = String::new();
    for at in 0..limit + extra {
        write!(named, "j_ref_{at}: 1, ").unwrap();
    }
    let patch = patch(
        "p_merge_then_delete",
        "deployment",
        deployment.get(),
        &format!(
            "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [who_e7h2jv3nxc4yaw6t]\n  journeys: {{{named}j_vendor_eval: 2}}\n\
- op: delete_identity\n  identity: who_d4m6uk2eqf5rbhzs\n"
        ),
    );
    let Rejection::Invalid { violations } =
        rejected(service.patch(&call("u_lead", AT), &domain(patch)).await)
    else {
        panic!("the delete is refused")
    };
    let refused = &violations.as_slice()[0];
    assert_eq!(refused.code, ViolationCode::IdentityInUse);
    assert_eq!(refused.related.len(), limit);
    let more = format!("{} more", extra + 1);
    assert!(refused.message.contains(&more), "{}", refused.message);
}

/// E6: two identities deleted in one patch that the same journeys refer to each get their own
/// remainder: journeys loaded for the second are not added to the first's count of unlisted.
async fn deletes_sharing_their_references_each_count_the_remainder_once(
    service: &Service,
    _: &InProcessNotifier,
) {
    let limit = cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let extra = 3;
    vendor_after(service, 2).await;
    let deployment = refer_to_e7h2(service, limit + extra).await;
    let delete = patch(
        "p_delete_both",
        "deployment",
        deployment.get(),
        "- op: delete_identity\n  identity: who_e7h2jv3nxc4yaw6t\n\
- op: delete_identity\n  identity: who_d4m6uk2eqf5rbhzs\n",
    );
    let Rejection::Invalid { violations } =
        rejected(service.patch(&call("u_lead", AT), &domain(delete)).await)
    else {
        panic!("the deletes are refused")
    };
    let more = format!("{} more", extra + 1);
    assert_eq!(violations.as_slice().len(), 2, "{violations:#?}");
    for refused in violations.as_slice() {
        assert!(refused.message.contains(&more), "{}", refused.message);
    }
}

/// E6: a route that many published versions name is one reference, not one per version: the
/// rejection names the route once and counts nothing as left out.
async fn a_route_with_many_versions_naming_an_identity_is_one_reference(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 1).await;
    let route = "{route: vendor-evaluation}";
    let first = "- op: open_draft\n  source: edit\n- op: set_participation\n  node: n_purpose\n  kind: k_informed\n  source: [who_d4m6uk2eqf5rbhzs]\n";
    applied(
        service
            .patch(
                &call("u_author", AT),
                &domain(patch("p_v_0", route, 1, first)),
            )
            .await,
    );
    let mut revision = 2;
    for at in 0..cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX {
        let publish = patch(
            &format!("p_pub_{at}"),
            route,
            revision,
            "- op: publish_draft\n",
        );
        applied(service.patch(&call("u_author", AT), &domain(publish)).await);
        let open = patch(
            &format!("p_open_{at}"),
            route,
            revision + 1,
            "- op: open_draft\n  source: edit\n",
        );
        applied(service.patch(&call("u_author", AT), &domain(open)).await);
        revision += 2;
    }
    let Rejection::Invalid { violations } = rejected(
        deployment_patch(
            service,
            "p_delete_dara",
            "- op: delete_identity\n  identity: who_d4m6uk2eqf5rbhzs\n",
        )
        .await,
    ) else {
        panic!("the delete is refused")
    };
    let refused = &violations.as_slice()[0];
    let routes = refused
        .related
        .iter()
        .filter(|s| matches!(s, Subject::Route(_)))
        .count();
    assert_eq!(routes, 1, "{refused:#?}");
    assert!(
        !refused.message.contains("not listed"),
        "{}",
        refused.message
    );
}

/// E6: two identities deleted in one patch that different graphs of one route name are each
/// refused with the route: loading the route for the first does not hide the second's graph.
async fn deletes_naming_different_graphs_of_one_route_each_name_the_route(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 1).await;
    let route = "{route: vendor-evaluation}";
    let participation = |who: &str| {
        format!(
            "- op: set_participation\n  node: n_purpose\n  kind: k_informed\n  source: [{who}]\n"
        )
    };
    let steps = [
        (
            "p_open_a",
            format!(
                "- op: open_draft\n  source: edit\n{}",
                participation("who_e7h2jv3nxc4yaw6t")
            ),
        ),
        ("p_pub_a", "- op: publish_draft\n".to_string()),
        (
            "p_open_b",
            format!(
                "- op: open_draft\n  source: edit\n{}",
                participation("who_d4m6uk2eqf5rbhzs")
            ),
        ),
    ];
    for (revision, (id, ops)) in (1..).zip(&steps) {
        let step = patch(id, route, revision, ops);
        applied(service.patch(&call("u_author", AT), &domain(step)).await);
    }
    let Rejection::Invalid { violations } = rejected(
        deployment_patch(
            service,
            "p_delete_two",
            "- op: delete_identity\n  identity: who_e7h2jv3nxc4yaw6t\n\
- op: delete_identity\n  identity: who_d4m6uk2eqf5rbhzs\n",
        )
        .await,
    ) else {
        panic!("the deletes are refused")
    };
    assert_eq!(violations.as_slice().len(), 2);
    for refused in violations.as_slice() {
        assert!(
            refused
                .related
                .iter()
                .any(|s| matches!(s, Subject::Route(_))),
            "{refused:#?}"
        );
    }
}

/// E6: a delete refused because more graphs refer to the identity than a rejection lists names
/// that many, and says how many it leaves out.
async fn a_delete_rejection_lists_what_refers_up_to_its_limit_and_counts_the_rest(
    service: &Service,
    _: &InProcessNotifier,
) {
    let limit = cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let extra = 3;
    vendor_after(service, 2).await;
    let deployment = refer_to_e7h2(service, limit + extra).await;
    let delete = patch(
        "p_delete_many",
        "deployment",
        deployment.get(),
        "- op: delete_identity\n  identity: who_e7h2jv3nxc4yaw6t\n",
    );
    let Rejection::Invalid { violations } =
        rejected(service.patch(&call("u_lead", AT), &domain(delete)).await)
    else {
        panic!("the delete is refused")
    };
    let refused = &violations.as_slice()[0];
    assert_eq!(refused.code, ViolationCode::IdentityInUse);
    assert_eq!(refused.related.len(), limit, "what refers, up to the limit");
    // The journey the vendor scenario made also refers: everything beyond the limit is counted.
    let more = (limit + extra + 1 - limit).to_string();
    assert!(refused.message.contains(&more), "{}", refused.message);
}

/// Patches the deployment as the lead at its current revision.
async fn deployment_patch(
    service: &Service,
    id: &str,
    ops: &str,
) -> Result<Written, cairn_service::WriteError> {
    let revision = service.deployment().await.unwrap().revision.get();
    service
        .patch(
            &call("u_lead", AT),
            &domain(patch(id, "deployment", revision, ops)),
        )
        .await
}

/// The reference a route holds to an identity, if any.
async fn route_reference(
    service: &Service,
    route_id: &cairn_schema::RouteId,
    key: &cairn_schema::IdentityKey,
) -> Option<cairn_service::RouteReference> {
    service
        .identity_references(key)
        .await
        .unwrap()
        .unwrap()
        .routes
        .into_iter()
        .find(|reference| &reference.header.id == route_id)
}

/// E6: a route's draft and published versions can name an identity (a participation); a merge
/// needs neither a rewrite nor a check of them, since a route graph holds no answers and its
/// stored key resolves through the alias, and the survivor stays referred to by them: the
/// references read lists the route, and the survivor cannot be deleted while a version does.
async fn a_route_naming_an_identity_follows_its_merge_and_keeps_the_survivor_from_deletion(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 1).await;
    let route_id: cairn_schema::RouteId = "vendor-evaluation".parse().unwrap();
    let dara = "who_d4m6uk2eqf5rbhzs".parse().unwrap();
    let spare = "who_u5k3mq7wd2fz6ncx";
    let create =
        format!("- op: create_identity\n  identity: {{key: {spare}, name: Route Owner}}\n");
    applied(deployment_patch(service, "p_spare", &create).await);
    let route = "{route: vendor-evaluation}";
    let draft = patch(
        "p_draft",
        route,
        1,
        &format!(
            "- op: open_draft\n  source: edit\n- op: set_participation\n  node: n_purpose\n  kind: k_informed\n  source: [{spare}]\n"
        ),
    );
    applied(service.patch(&call("u_author", AT), &domain(draft)).await);
    let held = route_reference(service, &route_id, &spare.parse().unwrap())
        .await
        .expect("the draft refers");
    assert!(held.draft);

    let merge = format!(
        "- op: merge_identities\n  survivor: who_d4m6uk2eqf5rbhzs\n  merged: [{spare}]\n  journeys: {{}}\n"
    );
    applied(deployment_patch(service, "p_merge_route", &merge).await);
    let followed = route_reference(service, &route_id, &dara)
        .await
        .expect("the survivor is referred to by the route");
    assert!(
        followed.draft,
        "the draft's stored key resolves through the alias"
    );

    let publish = patch("p_publish_route", route, 2, "- op: publish_draft\n");
    applied(service.patch(&call("u_author", AT), &domain(publish)).await);
    let published = route_reference(service, &route_id, &dara).await.unwrap();
    assert!(
        !published.versions.is_empty(),
        "a published version names the merged key"
    );
    let delete = "- op: delete_identity\n  identity: who_d4m6uk2eqf5rbhzs\n";
    let Rejection::Invalid { violations } =
        rejected(deployment_patch(service, "p_delete_survivor", delete).await)
    else {
        panic!("the survivor is still referred to")
    };
    assert!(
        violations.as_slice()[0]
            .related
            .contains(&Subject::Route(route_id.clone())),
        "{violations:#?}"
    );
}

/// A11, Journey durability: a second draft is refused while one is open, and publishing a
/// new version leaves a journey on the old one exactly as it was.
async fn publishing_a_route_never_touches_its_journeys(service: &Service, _: &InProcessNotifier) {
    vendor_after(service, 1).await;
    let id = journey_id("j_vendor_eval");
    let before = service.journey(&id).await.unwrap();
    let route = "{route: vendor-evaluation}";
    let open =
        |id: &str, base: u32| domain(patch(id, route, base, "- op: open_draft\n  source: edit\n"));
    applied(
        service
            .patch(&call("u_author", AT), &open("p_open", 1))
            .await,
    );
    let again = rejected(
        service
            .patch(&call("u_author", AT), &open("p_open_again", 2))
            .await,
    );
    assert!(
        codes(&again).contains(&ViolationCode::DraftExists),
        "{again:#?}"
    );
    let publish = patch(
        "p_publish",
        route,
        2,
        "- op: set_node_field\n  node: n_purpose\n  value: {title: Why buy}\n- op: publish_draft\n",
    );
    applied(service.patch(&call("u_author", AT), &domain(publish)).await);
    let detail = service
        .route_detail(&"vendor-evaluation".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    let on_each: Vec<(u32, usize)> = detail
        .versions
        .iter()
        .map(|version| (version.version.get(), version.journeys.len()))
        .collect();
    assert_eq!(on_each, vec![(1, 1), (2, 0)]);
    assert_eq!(
        service.journey(&id).await.unwrap(),
        before,
        "the journey is untouched"
    );
}

/// B11, A19: an archived journey takes only a move to active or completed, or a hard delete; the delete removes
/// it, the deployment log keeps who deleted it, and its id is never reused.
async fn an_archived_journey_takes_only_a_status_move_or_deletion_and_its_id_never_returns(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(
        service,
        "j_done",
        &["{key: n_a, id: a, kind: action, title: A}"],
    )
    .await;
    let target = "{journey: j_done}";
    let status = |id: &str, base: u32, status: &str| {
        domain(patch(
            id,
            target,
            base,
            &format!("- op: set_journey_status\n  status: {status}\n"),
        ))
    };
    applied(
        service
            .patch(&call("u_lead", AT), &status("p_complete", 1, "completed"))
            .await,
    );
    applied(
        service
            .patch(&call("u_lead", AT), &status("p_archive", 2, "archived"))
            .await,
    );
    let edit = domain(patch(
        "p_edit",
        target,
        3,
        "- op: edit_journey\n  name: Again\n",
    ));
    let refused = rejected(service.patch(&call("u_lead", AT), &edit).await);
    assert_eq!(codes(&refused), vec![ViolationCode::ArchivedJourney]);
    for (base, id, to) in [(3, "p_reopen", "active"), (4, "p_rearchive", "archived")] {
        applied(
            service
                .patch(&call("u_lead", AT), &status(id, base, to))
                .await,
        );
    }
    let delete = deletion(service, "p_delete", &[("j_done", 5)], &[]).await;
    applied(service.patch(&call("u_lead", AT), &delete).await);
    assert_eq!(service.journey(&journey_id("j_done")).await.unwrap(), None);
    let query = EventQuery {
        log: Some(Domain::Deployment),
        ..EventQuery::default()
    };
    let logged = service.events(&query).await.unwrap().items;
    assert!(
        logged
            .iter()
            .any(|entry| entry.event.event_type == EventType::JourneyDeleted)
    );
    let recreate = domain(patch(
        "p_recreate",
        target,
        0,
        "- op: create_journey\n  name: Back\n",
    ));
    let refused = rejected(service.patch(&call("u_lead", AT), &recreate).await);
    assert_eq!(codes(&refused), vec![ViolationCode::DeletedId]);
}

/// A deployment patch deleting each journey and route at the revision named, against the
/// deployment's current revision.
async fn deletion(
    service: &Service,
    id: &str,
    journeys: &[(&str, u32)],
    routes: &[(&str, u32)],
) -> DomainPatch {
    let mut mutations = String::new();
    for (journey, revision) in journeys {
        write!(
            mutations,
            "- op: delete_journey\n  journey: {journey}\n  revision: {revision}\n"
        )
        .unwrap();
    }
    for (route, revision) in routes {
        write!(
            mutations,
            "- op: delete_route\n  route: {route}\n  revision: {revision}\n"
        )
        .unwrap();
    }
    let base = service.deployment().await.unwrap().revision.get();
    domain(patch(id, "deployment", base, &mutations))
}

fn route_in_use(rejection: &Rejection) -> Vec<Subject> {
    let Rejection::Invalid { violations } = rejection else {
        panic!("expected an invalid rejection, got {rejection:#?}")
    };
    let [found] = violations.as_slice() else {
        panic!("{violations:#?}")
    };
    assert_eq!(found.code, ViolationCode::RouteInUse);
    found.related.clone()
}

/// A19: a route whose old version a journey still follows is refused, naming the journey,
/// though no journey is on its latest version; deleted with the journey in one patch, both go.
async fn a_route_a_journey_follows_on_an_old_version_is_deleted_only_with_it(
    service: &Service,
    _: &InProcessNotifier,
) {
    vendor_after(service, 1).await;
    let route = "{route: vendor-evaluation}";
    let republish = "- op: open_draft\n  source: edit\n- op: publish_draft\n";
    applied(
        service
            .patch(
                &call("u_author", AT),
                &domain(patch("p_v2", route, 1, republish)),
            )
            .await,
    );
    let id = "vendor-evaluation".parse().unwrap();
    let detail = service.route_detail(&id).await.unwrap().unwrap();
    assert!(detail.in_use);
    let journey = service
        .journey(&journey_id("j_vendor_eval"))
        .await
        .unwrap()
        .unwrap();
    let alone = deletion(service, "p_route", &[], &[("vendor-evaluation", 2)]).await;
    let refused = rejected(service.patch(&call("u_lead", AT), &alone).await);
    assert_eq!(
        route_in_use(&refused),
        [Subject::Journey(journey_id("j_vendor_eval"))]
    );
    let both = deletion(
        service,
        "p_both",
        &[("j_vendor_eval", journey.revision.get())],
        &[("vendor-evaluation", 2)],
    )
    .await;
    applied(service.patch(&call("u_lead", AT), &both).await);
    assert_eq!(service.route_detail(&id).await.unwrap(), None);
    let query = EventQuery {
        log: Some(Domain::Deployment),
        ..EventQuery::default()
    };
    let logged: Vec<EventType> = service
        .events(&query)
        .await
        .unwrap()
        .items
        .iter()
        .map(|entry| entry.event.event_type)
        .filter(|kind| matches!(kind, EventType::JourneyDeleted | EventType::RouteDeleted))
        .collect();
    assert_eq!(logged, [EventType::JourneyDeleted, EventType::RouteDeleted]);
}

/// A19, A21: a segment that only a published version of another route inserts is in use: no
/// journey and no draft holds it, and the delete is still refused, naming the route.
async fn a_segment_inserted_only_in_a_published_version_is_not_deleted(
    service: &Service,
    _: &InProcessNotifier,
) {
    let segment = "{route: reuse}";
    let made = "- op: create_route\n  name: Reuse\n  kind: segment\n- op: open_draft\n  source: import\n\
                - op: add_node\n  node: {key: n_reuse, id: reuse, kind: group, title: Reuse}\n\
                - op: add_node\n  node: {key: n_check, id: check, parent: n_reuse, kind: action, title: Check}\n\
                - op: publish_draft\n";
    applied(
        service
            .patch(
                &call("u_author", AT),
                &domain(patch("p_segment", segment, 0, made)),
            )
            .await,
    );
    let host = "{route: host}";
    let inserted = "- op: create_route\n  name: Host\n- op: open_draft\n  source: import\n\
                    - op: add_node\n  node: {key: n_slot, id: slot, kind: group, title: Slot}\n\
                    - op: insert_segment\n  insertion: i_reuse\n  segment: {route: reuse, version: 1}\n  parent: n_slot\n\
                    - op: publish_draft\n";
    applied(
        service
            .patch(
                &call("u_author", AT),
                &domain(patch("p_host", host, 0, inserted)),
            )
            .await,
    );
    let reuse = "reuse".parse().unwrap();
    assert!(service.route_detail(&reuse).await.unwrap().unwrap().in_use);
    let alone = deletion(service, "p_segment_gone", &[], &[("reuse", 1)]).await;
    let refused = rejected(service.patch(&call("u_lead", AT), &alone).await);
    assert_eq!(
        route_in_use(&refused),
        [Subject::Route("host".parse().unwrap())]
    );
    let both = deletion(service, "p_both", &[], &[("reuse", 1), ("host", 1)]).await;
    applied(service.patch(&call("u_lead", AT), &both).await);
    assert_eq!(service.route_detail(&reuse).await.unwrap(), None);
}

/// A12, H2: the same mutations from a user and from their agent produce the same journey;
/// only the events' stamp differs.
async fn a_users_patch_and_their_agents_land_alike_but_for_the_stamp(
    service: &Service,
    _: &InProcessNotifier,
) {
    let mutations = "\
- op: create_journey\n  name: Same\n\
- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n\
- op: transition\n  node: n_a\n  transition: start\n";
    let by_user = patch("p_user", "{journey: j_user}", 0, mutations);
    let by_agent = patch("p_agent", "{journey: j_agent}", 0, mutations);
    applied(service.patch(&call("u_lead", AT), &domain(by_user)).await);
    let agent = support::agent_call("u_lead", "ag_helper", AT);
    applied(service.patch(&agent, &domain(by_agent)).await);
    let user_journey = service
        .journey(&journey_id("j_user"))
        .await
        .unwrap()
        .unwrap();
    let agent_journey = service
        .journey(&journey_id("j_agent"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user_journey.graph, agent_journey.graph);
    assert_eq!(user_journey.revision, agent_journey.revision);
    for (journey, expected) in [
        ("j_user", call("u_lead", AT).actor),
        ("j_agent", agent.actor),
    ] {
        let query = EventQuery {
            log: Some(Domain::Journey(journey_id(journey))),
            ..EventQuery::default()
        };
        let events = service.events(&query).await.unwrap().items;
        assert_eq!(events.len(), 3);
        assert!(
            events.iter().all(|logged| logged.event.actor == expected),
            "{journey}"
        );
    }
}

/// H6: every commit is announced: the journey a patch moved, and the deployment when an
/// identity create riding in it moved that too; a subscriber hears only what it watches.
async fn every_commit_is_announced_with_each_revision_it_moved(
    service: &Service,
    _: &InProcessNotifier,
) {
    let watching: BTreeSet<Watch> = [
        Watch::Journeys,
        Watch::One(RevisionOf::Domain(Domain::Deployment)),
    ]
    .into_iter()
    .collect();
    let subscription = service.subscribe(watching).await.unwrap();
    let initial = subscription.take(std::time::Duration::ZERO);
    assert!(matches!(initial, Take::Current(_)));
    let riding = patch(
        "p_riding",
        "{journey: j_new}",
        0,
        "- op: create_journey\n  name: New\n- op: create_identity\n  identity: {key: who_new, name: New}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(riding)).await);
    let route = patch(
        "p_route",
        "{route: elsewhere}",
        0,
        "- op: create_route\n  name: Elsewhere\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(route)).await);
    let one = cairn_schema::Revision::NONE.next();
    assert_eq!(
        subscription.take(std::time::Duration::from_secs(1)),
        Take::Ticks(vec![
            Tick {
                of: RevisionOf::Domain(Domain::Journey(journey_id("j_new"))),
                revision: one,
            },
            Tick {
                of: RevisionOf::Domain(Domain::Deployment),
                revision: one,
            },
        ])
    );
}

/// D7: an accepted patch reports what it newly caused (a completion it made stale), and both
/// sides are derived at the request's today, so work that became overdue at midnight is not
/// blamed on a patch made the next day, while work the patch adds already overdue is.
async fn consequences_report_what_the_patch_caused_and_not_what_midnight_did(
    service: &Service,
    _: &InProcessNotifier,
) {
    let id = journey_id("j_d7");
    create(
        service,
        "j_d7",
        &[
            "{key: n_work, id: work, kind: action, title: Work}",
            "{key: n_review, id: review, kind: milestone, title: Review}",
            "{key: n_draft, id: draft, kind: action, title: Draft, due_by: {before: n_review}}",
        ],
    )
    .await;
    let target = "{journey: j_d7}";
    let prepare = patch(
        "p_prepare",
        target,
        1,
        "- op: transition\n  node: n_work\n  transition: complete\n- op: set_pin\n  node: n_review\n  date: \"2026-10-07\"\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(prepare)).await);
    let insert = patch(
        "p_insert",
        target,
        2,
        "- op: add_node\n  node: {key: n_new, id: new, kind: action, title: New}\n- op: add_edge\n  edge: {node: n_work, requires: n_new}\n",
    );
    let Written::Applied { consequences, .. } = applied(
        service
            .patch(&call("u_lead", AT), &domain(insert.clone()))
            .await,
    ) else {
        unreachable!()
    };
    let stale: Vec<_> = consequences[&id]
        .stale
        .iter()
        .map(|found| found.node.clone())
        .collect();
    assert_eq!(stale, vec!["n_work".parse().unwrap()]);
    let again = service
        .patch(&call("u_lead", AT), &domain(insert))
        .await
        .unwrap();
    assert!(
        matches!(again, Written::AlreadyApplied { .. }),
        "none the second time"
    );

    // Two days on, n_draft is overdue by the calendar alone; a patch adding n_late, due
    // before the same review, is blamed only for n_late.
    let late = patch(
        "p_late",
        target,
        3,
        "- op: add_node\n  node: {key: n_late, id: late, kind: action, title: Late, due_by: {before: n_review}}\n",
    );
    let Written::Applied { consequences, .. } = applied(
        service
            .patch(&call("u_lead", "2026-10-09T12:00:00Z"), &domain(late))
            .await,
    ) else {
        unreachable!()
    };
    assert_eq!(consequences[&id].overdue, vec!["n_late".parse().unwrap()]);
}

#[test]
fn a_patch_to_a_proposal_is_not_a_domain_patch() {
    let proposal = patch(
        "p_proposal",
        "{proposal: {id: pr_one, destination: {journey: j_one}}}",
        0,
        "- op: discard_proposal\n",
    );
    assert!(DomainPatch::new(proposal, None).is_err());
}

/// The futures a multi-threaded host awaits are `Send` over either store
/// (2026-10-18-the-store-trait-returns-boxed-futures-and-the-stack-holds-a-trait-object.md).
#[test]
fn every_operation_can_be_awaited_on_another_thread() {
    fn send<T: Send>(_: &T) {}
    let (memory, _) = support::memory();
    let created = domain(patch(
        "p_create",
        "{journey: j_one}",
        0,
        "- op: create_journey\n  name: One\n",
    ));
    let call = call("u_lead", AT);
    send(&memory.patch(&call, &created));
    send(&memory.caller(&call));
    send(&memory.arrive(&call, &mut cairn_schema::SequentialKeys::default()));
    send(&memory.subscribe(BTreeSet::new()));
    send(&memory.journey(&journey_id("j_one")));
    support::run(async {
        let (sqlite, _) = support::sqlite(TMP.as_ref());
        send(&sqlite.patch(&call, &created));
        send(&sqlite.subscribe(BTreeSet::new()));
    });
}

/// H6, A19: a hard delete moves the deployment and announces it, and announces each journey it
/// deleted past the revision a view holds, so an open view learns it is gone; resubmitted, it is
/// answered from its receipt.
async fn a_hard_delete_announces_the_deployment_and_each_journey_it_deleted(
    service: &Service,
    _: &InProcessNotifier,
) {
    create(service, "j_one", &[]).await;
    create(service, "j_two", &[]).await;
    let of = |id: &str| RevisionOf::Domain(Domain::Journey(journey_id(id)));
    let deployment = RevisionOf::Domain(Domain::Deployment);
    let watching: BTreeSet<Watch> = [deployment.clone(), of("j_one"), of("j_two")]
        .into_iter()
        .map(Watch::One)
        .collect();
    let subscription = service.subscribe(watching).await.unwrap();
    let mut clock = std::time::Duration::ZERO;
    let mut take = || {
        clock += std::time::Duration::from_secs(1);
        subscription.take(clock)
    };
    let initial = take();
    assert!(matches!(initial, Take::Current(_)), "{initial:?}");
    let base = service.deployment().await.unwrap().revision;
    let delete = deletion(service, "p_delete", &[("j_one", 1), ("j_two", 1)], &[]).await;
    applied(service.patch(&call("u_lead", AT), &delete).await);
    let Take::Ticks(mut ticks) = take() else {
        panic!("the delete is announced")
    };
    ticks.sort();
    let two = cairn_schema::Revision::NONE.next().next();
    let mut expected = vec![
        Tick {
            of: deployment,
            revision: base.next(),
        },
        Tick {
            of: of("j_one"),
            revision: two,
        },
        Tick {
            of: of("j_two"),
            revision: two,
        },
    ];
    expected.sort();
    assert_eq!(ticks, expected);
    let again = service.patch(&call("u_lead", AT), &delete).await.unwrap();
    assert!(matches!(again, Written::AlreadyApplied { .. }), "{again:?}");
}
