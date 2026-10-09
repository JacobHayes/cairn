//! Domain patches through the service, over the memory and Turso stores alike (A11, A12,
//! A17, A19, B1, B11, D7, E6, H2, H3, H5, H6, Journey durability).
#![cfg(test)]

use crate::service::support;

use std::collections::BTreeSet;

use cairn_engine::Records;
use cairn_schema::{Domain, EventType, JourneyId, Rejection, RevisionOf, Subject, ViolationCode};
use cairn_service::{DomainPatch, Service, Written};
use cairn_store::{EventQuery, InProcessNotifier, Store, Take, Tick, Watch};

use support::{
    applied, call, domain, engine_step, patch, publish_fixture_route, rejected, scenario,
    step_call, step_patch, vendor_after,
};

/// Runs each case once over a fresh memory store and once over a fresh Turso store.
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
        mod turso {
            $(
                #[test]
                fn $case() {
                    crate::service::support::run(async {
                        let (service, notifier) = crate::service::support::turso(crate::service::writes::TMP.as_ref()).await;
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
    a_removal_retried_after_an_insertion_beneath_it_is_not_widened,
    a_merge_that_breaks_a_referencing_journey_is_rejected_naming_it,
    a_merge_moves_emails_and_an_email_stays_on_one_entity,
    a_merge_is_stale_once_a_journey_it_checked_moves_or_a_new_one_refers,
    publishing_a_route_never_touches_its_journeys,
    an_archived_journey_takes_only_unarchiving_or_deletion_and_its_id_never_returns,
    a_users_patch_and_their_agents_land_alike_but_for_the_stamp,
    every_commit_is_announced_with_each_revision_it_moved,
    consequences_report_what_the_patch_caused_and_not_what_midnight_did,
    a_hard_delete_announces_no_deployment_revision_it_did_not_commit,
);

const AT: &str = "2026-10-06T12:00:00Z";

/// Where the Turso cases keep their databases: cargo's per-target temporary directory.
const TMP: &str = env!("CARGO_TARGET_TMPDIR");

/// The events a scenario's patches emit: one per mutation (J2).
fn scenario_event_count(steps: &[cairn_schema::ScenarioStep]) -> usize {
    steps.iter().map(|step| step.patch.mutations.len()).sum()
}

fn journey_id(id: &str) -> JourneyId {
    id.parse().unwrap()
}

/// Creates an empty journey `id` with the nodes given (as YAML flow mappings), at revision 1.
async fn create<S: Store>(service: &Service<S>, id: &str, nodes: &[&str]) {
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
async fn a_fixture_scenario_lands_as_the_engine_alone_would<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
    let seed = publish_fixture_route("vendor-evaluation");
    let seed_call = call("u_author", "2026-09-01T12:00:00Z");
    applied(service.patch(&seed_call, &domain(seed.clone())).await);
    let mut reference = engine_step(&Records::default(), &seed, &seed_call, None);
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
async fn a_resubmission_after_another_patch_is_answered_from_its_receipt<S: Store>(
    service: &Service<S>,
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
/// lands; the rejection carries what the edit touched, which overlaps the removal, so the
/// client may not retry it on its own.
async fn a_subtree_removal_conflicts_with_an_edit_inside_it<S: Store>(
    service: &Service<S>,
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
    } = rejected(
        service
            .patch(&call("u_lead", AT), &domain(removal.clone()))
            .await,
    )
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
    assert!(
        intervening.overlaps(&removal.touched()),
        "not safe to retry"
    );
}

/// H5, A18: when what intervened is an insertion beneath the subtree, the touched sets do
/// not overlap and the client retries at the new revision; the retry names only what its
/// author saw, so it is rejected rather than widened to the new node.
async fn a_removal_retried_after_an_insertion_beneath_it_is_not_widened<S: Store>(
    service: &Service<S>,
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
    let Rejection::Stale { intervening, .. } = rejected(
        service
            .patch(&call("u_lead", AT), &domain(removal.clone()))
            .await,
    ) else {
        panic!("the removal is stale");
    };
    assert!(!intervening.overlaps(&removal.touched()), "safe to retry");
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

/// E6: a merge whose aliases make a contradictory chain apply in a referencing journey is
/// rejected, naming the journey.
async fn a_merge_that_breaks_a_referencing_journey_is_rejected_naming_it<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let prepare = patch(
        "p_prepare",
        "{journey: j_vendor_eval}",
        2,
        "\
- op: create_entity\n  entity: {key: e_other, name: Other}\n\
- op: add_node\n  node: {key: n_start, id: start, kind: milestone, title: Start}\n\
- op: set_pin\n  node: n_start\n  date: \"2026-11-10\"\n\
- op: add_node\n  node: {key: n_long, id: long, kind: action, title: Long, estimate: 30, relevant_when: {equals: {decision: n_who_owns, value: e_other}}, not_before: {after: n_start}, due_by: {before: n_decision_meeting}}\n",
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
        "- op: merge_entities\n  survivor: e_other\n  merged: e_lead\n  journeys: {j_vendor_eval: 3}\n",
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

/// H3, E6: a merge moves the merged entity's emails to the survivor and leaves its key as an
/// alias; an email already on another entity cannot be given to a second one.
async fn a_merge_moves_emails_and_an_email_stays_on_one_entity<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let taken = patch(
        "p_taken",
        "deployment",
        1,
        "- op: edit_entity\n  entity: {key: e_stakeholder_b, name: Stakeholder B, emails: [Lead@Example.org]}\n",
    );
    let refused = rejected(service.patch(&call("u_lead", AT), &domain(taken)).await);
    assert!(matches!(refused, Rejection::Invalid { .. }), "{refused:#?}");

    let give = patch(
        "p_give",
        "deployment",
        1,
        "- op: edit_entity\n  entity: {key: e_stakeholder_b, name: Stakeholder B, emails: [b@example.org]}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(give)).await);
    let merge = patch(
        "p_merge",
        "deployment",
        2,
        "- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {j_vendor_eval: 2}\n",
    );
    applied(service.patch(&call("u_lead", AT), &domain(merge)).await);
    let survivor = service
        .entity(&"e_stakeholder_b".parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        survivor.key,
        "e_stakeholder_a".parse().unwrap(),
        "the alias resolves"
    );
    assert!(survivor.emails.contains(&"b@example.org".parse().unwrap()));
}

/// E6, H5: a merge names the journeys it was checked against at their revisions; one of them
/// edited since, or a journey newly referring to either entity, makes it stale.
async fn a_merge_is_stale_once_a_journey_it_checked_moves_or_a_new_one_refers<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
    vendor_after(service, 2).await;
    let merge = |id: &str| {
        domain(patch(
            id,
            "deployment",
            1,
            "- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {j_vendor_eval: 2}\n",
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
- op: fill_role\n  role: r_lead\n  entities: [e_stakeholder_b]\n",
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
        "- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {j_vendor_eval: 3}\n",
    ));
    let Rejection::Stale { conflicts, .. } =
        rejected(service.patch(&call("u_lead", AT), &fresh).await)
    else {
        panic!("a journey newly refers to a merged entity");
    };
    let newcomer = RevisionOf::Domain(Domain::Journey(journey_id("j_new")));
    assert!(
        conflicts.iter().any(|conflict| conflict.of == newcomer),
        "{conflicts:#?}"
    );
}

/// A11, Journey durability: a second draft is refused while one is open, and publishing a
/// new version leaves a journey on the old one exactly as it was.
async fn publishing_a_route_never_touches_its_journeys<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
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

/// B11, A19: an archived journey takes only un-archiving or a hard delete; the delete removes
/// it, the deployment log keeps who deleted it, and its id is never reused.
async fn an_archived_journey_takes_only_unarchiving_or_deletion_and_its_id_never_returns<
    S: Store,
>(
    service: &Service<S>,
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
    applied(
        service
            .patch(&call("u_lead", AT), &status("p_unarchive", 3, "completed"))
            .await,
    );
    applied(
        service
            .patch(&call("u_lead", AT), &status("p_rearchive", 4, "archived"))
            .await,
    );
    let delete = domain(patch("p_delete", target, 5, "- op: delete_journey\n"));
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
    assert_eq!(codes(&refused), vec![ViolationCode::DeletedJourneyId]);
}

/// A12, H2: the same mutations from a user and from their agent produce the same journey;
/// only the events' stamp differs.
async fn a_users_patch_and_their_agents_land_alike_but_for_the_stamp<S: Store>(
    service: &Service<S>,
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
/// entity create riding in it moved that too; a subscriber hears only what it watches.
async fn every_commit_is_announced_with_each_revision_it_moved<S: Store>(
    service: &Service<S>,
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
        "- op: create_journey\n  name: New\n- op: create_entity\n  entity: {key: e_new, name: New}\n",
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
async fn consequences_report_what_the_patch_caused_and_not_what_midnight_did<S: Store>(
    service: &Service<S>,
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
/// (decisions/2026-10-06-the-store-trait-is-generic-with-send-futures-not-object-safe.md).
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
    send(&memory.viewer(&call));
    send(&memory.subscribe(BTreeSet::new()));
    send(&memory.journey(&journey_id("j_one")));
    support::run(async {
        let (turso, _) = support::turso(TMP.as_ref()).await;
        send(&turso.patch(&call, &created));
        send(&turso.subscribe(BTreeSet::new()));
    });
}

/// H6: a hard delete moves no deployment revision in the store, so none is announced; the
/// entity create that later moves it to 1 is heard.
async fn a_hard_delete_announces_no_deployment_revision_it_did_not_commit<S: Store>(
    service: &Service<S>,
    _: &InProcessNotifier,
) {
    create(service, "j_gone", &[]).await;
    let deployment = RevisionOf::Domain(Domain::Deployment);
    let watching: BTreeSet<Watch> = [Watch::One(deployment.clone())].into_iter().collect();
    let subscription = service.subscribe(watching).await.unwrap();
    let mut clock = std::time::Duration::ZERO;
    let mut take = || {
        clock += std::time::Duration::from_secs(1);
        subscription.take(clock)
    };
    let initial = take();
    assert!(matches!(initial, Take::Current(_)), "{initial:?}");
    let delete = domain(patch(
        "p_delete",
        "{journey: j_gone}",
        1,
        "- op: delete_journey\n",
    ));
    applied(service.patch(&call("u_lead", AT), &delete).await);
    assert_eq!(take(), Take::Empty, "the deployment did not move");
    let entity = domain(patch(
        "p_entity",
        "deployment",
        0,
        "- op: create_entity\n  entity: {key: e_new, name: New}\n",
    ));
    applied(service.patch(&call("u_lead", AT), &entity).await);
    assert_eq!(
        take(),
        Take::Ticks(vec![Tick {
            of: deployment,
            revision: cairn_schema::Revision::NONE.next(),
        }])
    );
}
