//! The write path (A15 to A18, B4, B5, B10, B11, D4, E3, E6, G2): what apply accepts, what it
//! rejects, and what an accepted patch records.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::apply;
use cairn_schema::{
    ChangeClass, EventType, Guard, GuardFailure, LocalEdit, NodeField, Rejection, RevisionOf,
    ViolationCode,
};
use support::{codes, key, vendor_after, vendor_graph, vendor_patch};

#[test]
fn a_stale_base_revision_is_a_rejection_naming_the_revision() {
    let records = vendor_after(2);
    let mut patch = support::patch_to(
        &records,
        "{journey: j_vendor_eval}",
        "- op: transition\n  node: n_kickoff\n  transition: reach\n",
    );
    patch.base_revision = cairn_schema::Revision::NONE.next();
    let Err(Rejection::Stale { conflicts, .. }) = apply(&records, &patch, &support::fixed_inputs())
    else {
        panic!("a base revision behind the journey's is stale (A17, H5)")
    };
    assert_eq!(conflicts.len(), 1);
    assert!(matches!(conflicts[0].of, RevisionOf::Domain(_)));
    assert_eq!(
        conflicts[0].current,
        records.revision(&patch.target.domain())
    );
}

#[test]
fn a_stale_deployment_revision_is_a_rejection() {
    let records = vendor_after(1);
    let mut patch = support::patch_to(
        &records,
        "{journey: j_vendor_eval}",
        "- op: answer\n  decision: n_who_owns\n  value: {entity: e_lead}\n",
    );
    patch.deployment_revision = Some(cairn_schema::Revision::NONE);
    assert!(matches!(
        apply(&records, &patch, &support::fixed_inputs()),
        Err(Rejection::Stale { .. })
    ));
}

#[test]
fn a_patch_applies_all_or_nothing_and_lists_every_violation() {
    let records = vendor_after(2);
    let before = records.clone();
    let mutations = "\
- op: transition\n  node: n_kickoff\n  transition: reach\n\
- op: transition\n  node: n_purpose\n  transition: start\n\
- op: transition\n  node: n_nowhere\n  transition: start\n\
- op: answer\n  decision: n_partner_runs\n  value: {text: maybe}\n";
    let found = codes(vendor_patch(&records, mutations));
    assert_eq!(
        records, before,
        "a rejected patch leaves the records as they were"
    );
    let mut found = found;
    found.sort();
    let mut expected = vec![
        ViolationCode::IllegalTransition,
        ViolationCode::UnresolvedReference,
        ViolationCode::AnswerTypeMismatch,
    ];
    expected.sort();
    assert_eq!(
        found, expected,
        "three independent violations report three (A15)"
    );
}

#[test]
fn an_accepted_patch_advances_the_revision_by_one_with_one_event_per_mutation() {
    let records = vendor_after(2);
    let mutations = "- op: transition\n  node: n_kickoff\n  transition: reach\n- op: transition\n  node: n_access\n  transition: start\n";
    let applied = vendor_patch(&records, mutations).unwrap();
    let domain = cairn_schema::Domain::Journey("j_vendor_eval".parse().unwrap());
    assert_eq!(applied.revision(), records.revision(&domain).next());
    assert_eq!(applied.records().revision(&domain), applied.revision());
    assert_eq!(applied.events().len(), 2);
    let types: Vec<EventType> = applied
        .events()
        .iter()
        .map(|event| event.event_type)
        .collect();
    assert_eq!(
        types,
        [EventType::MilestoneReached, EventType::NodeTransitioned]
    );
}

/// The subtree of the partner-led group as an author sees it after the journey starts.
const REMOVE_PARTNER_LED: &str = "\
- op: remove_node\n  removal:\n    node: n_partner_led\n    descendants: [n_criteria, n_partner_results]\n    edges: [{node: n_partner_results, requires: n_criteria}]\n";

#[test]
fn removing_a_route_copied_subtree_retires_its_keys_and_tombstones_it() {
    let records = vendor_after(1);
    let applied = vendor_patch(&records, REMOVE_PARTNER_LED).unwrap();
    let graph = vendor_graph(applied.records());
    for removed in ["n_partner_led", "n_criteria", "n_partner_results"] {
        assert!(graph.nodes.get(&key(removed)).is_none(), "{removed}");
        assert!(
            graph.retired_keys.nodes.contains(&key(removed)),
            "{removed}"
        );
        assert!(
            graph.state.tombstones.contains(&key(removed)),
            "{removed} has a tombstone (B4)"
        );
        assert!(!graph.state.nodes.contains_key(&key(removed)), "{removed}");
    }
    assert_eq!(applied.events().len(), 1);
}

#[test]
fn removing_a_journey_local_node_leaves_no_tombstone() {
    let records = vendor_after(5);
    let removal = "- op: remove_node\n  removal: {node: n_workload_ingest}\n";
    let applied = vendor_patch(&records, removal).unwrap();
    let graph = vendor_graph(applied.records());
    assert!(graph.retired_keys.nodes.contains(&key("n_workload_ingest")));
    assert!(!graph.state.tombstones.contains(&key("n_workload_ingest")));
}

#[test]
fn a_removal_that_misses_what_the_subtree_holds_is_rejected() {
    let records = vendor_after(1);
    let cases = [
        (
            "a descendant left out",
            "- op: remove_node\n  removal: {node: n_partner_led, descendants: [n_criteria]}\n",
            None,
        ),
        (
            "a child moved in",
            REMOVE_PARTNER_LED,
            Some("- op: set_node_field\n  node: n_kickoff\n  value: {parent: n_partner_led}\n"),
        ),
        (
            "a note added",
            REMOVE_PARTNER_LED,
            Some(
                "- op: add_annotation\n  annotation: {key: a_note, node: n_criteria, note: Started.}\n",
            ),
        ),
        (
            "a participation added",
            REMOVE_PARTNER_LED,
            Some(
                "- op: set_participation\n  node: n_criteria\n  kind: k_owner\n  source: [e_lead]\n",
            ),
        ),
        (
            "an edge added",
            REMOVE_PARTNER_LED,
            Some("- op: add_edge\n  edge: {node: n_findings, requires: n_criteria}\n"),
        ),
    ];
    for (name, removal, earlier) in cases {
        let current = match earlier {
            Some(change) => vendor_patch(&records, change).unwrap().records().clone(),
            None => records.clone(),
        };
        let found = codes(vendor_patch(&current, removal));
        assert_eq!(found, [ViolationCode::RemovalWidened], "{name}");
    }
}

#[test]
fn a_removal_leaving_a_dangling_reference_is_rejected_unless_the_patch_fixes_it() {
    let records = vendor_after(1);
    let removal = "- op: remove_node\n  removal: {node: n_partner_runs}\n";
    let found = codes(vendor_patch(&records, removal));
    assert_eq!(found, [ViolationCode::DanglingReference]);
    let fixed = format!(
        "{removal}- op: set_node_field\n  node: n_partner_led\n  value: {{relevant_when: null}}\n"
    );
    assert!(vendor_patch(&records, &fixed).is_ok());
}

#[test]
fn a_removed_key_never_comes_back() {
    let records = vendor_after(1);
    let readd = "- op: add_node\n  node: {key: n_criteria, id: criteria-again, kind: action, title: Again}\n";
    let same_patch = format!("{REMOVE_PARTNER_LED}{readd}");
    assert_eq!(
        codes(vendor_patch(&records, &same_patch)),
        [ViolationCode::RetiredKeyReused]
    );
    let removed = vendor_patch(&records, REMOVE_PARTNER_LED).unwrap();
    assert_eq!(
        codes(vendor_patch(removed.records(), readd)),
        [ViolationCode::RetiredKeyReused]
    );
}

#[test]
fn edits_to_route_copied_nodes_set_markers_and_reset_clears_them() {
    let records = vendor_after(5);
    let edits = "\
- op: set_node_field\n  node: n_access\n  value: {title: \"Environment access, granted\"}\n\
- op: add_edge\n  edge: {node: n_findings, requires: n_access}\n\
- op: set_participation\n  node: n_access\n  kind: k_owner\n  source: [e_lead]\n\
- op: add_resource\n  node: n_access\n  resource: {key: a_tip, tip: Ask early.}\n\
- op: set_node_field\n  node: n_workload_ingest\n  value: {title: Ingest the workload}\n";
    let applied = vendor_patch(&records, edits).unwrap();
    let markers = &vendor_graph(applied.records()).state.local_edits;
    let on_access = &markers[&key("n_access")];
    assert!(on_access.contains(&LocalEdit::Field(NodeField::Title)));
    assert!(on_access.contains(&LocalEdit::Participation("k_owner".parse().unwrap())));
    assert!(on_access.contains(&LocalEdit::Resource("a_tip".parse().unwrap())));
    assert!(markers[&key("n_findings")].contains(&LocalEdit::Requires(key("n_access"))));
    assert!(
        !markers.contains_key(&key("n_workload_ingest")),
        "a journey-local node has no markers"
    );
    let reset =
        "- op: mark_local_edit\n  node: n_access\n  edit: {field: title}\n  marked: false\n";
    let reset = vendor_patch(applied.records(), reset).unwrap();
    let on_access = &vendor_graph(reset.records()).state.local_edits[&key("n_access")];
    assert!(!on_access.contains(&LocalEdit::Field(NodeField::Title)));
}

#[test]
fn a_journey_weight_edit_is_state() {
    let records = vendor_after(1);
    let applied = vendor_patch(
        &records,
        "- op: set_node_field\n  node: n_access\n  value: {weight: 3}\n",
    )
    .unwrap();
    assert_eq!(applied.events()[0].event_type, EventType::WeightChanged);
    let patch = support::patch_to(
        &records,
        "{journey: j_vendor_eval}",
        "- op: set_node_field\n  node: n_access\n  value: {weight: 3}\n",
    );
    assert_eq!(patch.change_class(), ChangeClass::State);
    let markers = &vendor_graph(applied.records()).state.local_edits[&key("n_access")];
    assert!(
        markers.contains(&LocalEdit::Field(NodeField::Weight)),
        "B5: tracked by the marker"
    );
}

/// The vendor evaluation after its scenario, with the final review opened: every dependency
/// of the final report is satisfied, so its other guards are what a completion meets.
fn report_ready() -> cairn_engine::Records {
    support::accepted_on(
        &vendor_after(8),
        "j_vendor_eval",
        "- op: transition\n  node: n_review_opens\n  transition: reach\n",
    )
}

/// [`report_ready`] without the final report's artifact link.
fn report_ready_without_artifact() -> cairn_engine::Records {
    support::accepted_on(
        &report_ready(),
        "j_vendor_eval",
        "- op: remove_annotation\n  annotation: a_report_link\n",
    )
}

#[test]
fn an_artifact_guard_fails_on_the_graph_the_patch_produces() {
    let records = report_ready();
    let complete = "- op: transition\n  node: n_final_report\n  transition: complete\n";
    // The artifact link was added in step 8 and the final review is open, so completing
    // passes.
    assert!(vendor_patch(&records, complete).is_ok());
    let without = format!("{complete}- op: remove_annotation\n  annotation: a_report_link\n");
    let Err(Rejection::Invalid { violations }) = vendor_patch(&records, &without) else {
        panic!("removing the artifact in the same patch fails the guard (D4)")
    };
    let [found] = violations.as_slice() else {
        panic!("{violations:#?}")
    };
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert_eq!(found.bypassable, Some(Guard::HasArtifact));
    assert!(found.failures.contains(&GuardFailure::MissingArtifact));
}

#[test]
fn an_artifact_on_another_node_does_not_satisfy_the_guard() {
    let records = report_ready_without_artifact();
    let mutations = "\
- op: add_annotation\n  annotation: {key: a_elsewhere, node: n_final_review, artifact: \"https://example.org/report\"}\n\
- op: transition\n  node: n_final_report\n  transition: complete\n";
    assert_eq!(
        codes(vendor_patch(&records, mutations)),
        [ViolationCode::GuardFailed]
    );
}

#[test]
fn a_bypass_records_the_failures_it_bypassed() {
    let records = report_ready_without_artifact();
    let mutations = "\
- op: transition\n  node: n_final_report\n  transition: complete\n\
- op: apply_override\n  node: n_final_report\n  override: {guard_bypass: {guards: [has_artifact], reason: Linked in the meeting notes.}}\n";
    let applied = vendor_patch(&records, mutations).unwrap();
    assert_eq!(applied.events()[1].event_type, EventType::GuardBypassed);
    let overrides = &vendor_graph(applied.records()).state.overrides[&key("n_final_report")];
    let bypass = overrides.bypass.as_ref().unwrap();
    assert_eq!(
        bypass.failures.iter().collect::<Vec<_>>(),
        [&GuardFailure::MissingArtifact]
    );
    let alone = "- op: apply_override\n  node: n_final_report\n  override: {guard_bypass: {guards: [has_artifact], reason: No transition.}}\n";
    assert_eq!(
        codes(vendor_patch(&records, alone)),
        [ViolationCode::IllegalTransition]
    );
}

#[test]
fn a_placeholder_completes_only_once_broken_down_or_atomic() {
    let records = vendor_after(4);
    let complete = "- op: transition\n  node: n_workload\n  transition: complete\n";
    let Err(Rejection::Invalid { violations }) = vendor_patch(&records, complete) else {
        panic!("an unexpanded placeholder fails broken_down (A16, B10)")
    };
    assert_eq!(violations.as_slice()[0].bypassable, Some(Guard::BrokenDown));
    let atomic = format!("- op: set_atomic\n  node: n_workload\n  atomic: true\n{complete}");
    assert!(vendor_patch(&records, &atomic).is_ok());
    assert_eq!(
        codes(vendor_patch(
            &records,
            "- op: set_atomic\n  node: n_access\n  atomic: true\n"
        )),
        [ViolationCode::FieldNotOnKind]
    );
}

#[test]
fn a_role_with_a_filling_decision_and_a_fed_milestone_change_only_through_the_decision() {
    let records = vendor_after(2);
    // The client routes both through the decision (decisions/2026-10-07-a-direct-fill-of-a-decision-filled-role-is-routed.md).
    for direct in [
        "- op: fill_role\n  role: r_eval_owner\n  entities: [e_lead]\n",
        "- op: clear_role_fill\n  role: r_eval_owner\n",
    ] {
        assert_eq!(
            codes(vendor_patch(&records, direct)),
            [ViolationCode::FilledThroughDecision],
            "{direct}"
        );
    }
    let pin = "- op: set_pin\n  node: n_decision_meeting\n  date: \"2026-11-30\"\n";
    assert_eq!(
        codes(vendor_patch(&records, pin)),
        [ViolationCode::PinnedThroughDecision]
    );
    let reopen = "- op: transition\n  node: n_who_owns\n  transition: reopen\n";
    let reopened = vendor_patch(&records, reopen).unwrap();
    assert!(
        !vendor_graph(reopened.records())
            .state
            .answers
            .contains_key(&key("n_who_owns")),
        "E3: reopening empties the role it filled"
    );
}

#[test]
fn an_entity_created_in_a_journey_patch_advances_the_deployment_once() {
    let (initial, applied) = support::run("vendor-evaluation");
    assert_eq!(initial.deployment.revision, cairn_schema::Revision::NONE);
    // Step 1 creates three entities in one journey patch.
    assert_eq!(
        applied[0].records().deployment.revision,
        cairn_schema::Revision::NONE.next()
    );
    assert_eq!(applied[0].records().deployment.entities.len(), 3);
    let again = "- op: create_entity\n  entity: {key: e_lead, name: Someone Else}\n";
    let records = applied[0].records();
    assert_eq!(
        codes(vendor_patch(records, again)),
        [ViolationCode::EntityKeyTaken]
    );
    let unknown = "- op: answer\n  decision: n_who_owns\n  value: {entity: e_nobody}\n";
    assert_eq!(
        codes(vendor_patch(records, unknown)),
        [ViolationCode::EntityUnresolved]
    );
}

#[test]
fn an_archived_journey_accepts_only_unarchiving_or_deletion() {
    let records = vendor_after(2);
    let archived =
        vendor_patch(&records, "- op: set_journey_status\n  status: archived\n").unwrap();
    let archived = archived.records();
    let answer = "- op: transition\n  node: n_kickoff\n  transition: reach\n";
    assert_eq!(
        codes(vendor_patch(archived, answer)),
        [ViolationCode::ArchivedJourney]
    );
    let active = "- op: set_journey_status\n  status: active\n";
    assert_eq!(
        codes(vendor_patch(archived, active)),
        [ViolationCode::ArchivedJourney]
    );
    assert!(vendor_patch(archived, "- op: set_journey_status\n  status: completed\n").is_ok());
    let deleted = vendor_patch(archived, "- op: delete_journey\n").unwrap();
    let deleted = deleted.records();
    assert!(deleted.journeys.is_empty());
    assert!(
        deleted
            .deleted_journeys
            .contains_key(&"j_vendor_eval".parse().unwrap())
    );
    let recreate = "- op: create_journey\n  name: Again\n";
    assert_eq!(
        codes(vendor_patch(deleted, recreate)),
        [ViolationCode::DeletedJourneyId]
    );
}

// Review round 1 regressions.

#[test]
fn clearing_a_participation_on_a_missing_node_is_a_violation() {
    let records = vendor_after(1);
    let clear = "- op: clear_participation\n  node: n_nowhere\n  kind: k_owner\n";
    assert_eq!(
        codes(vendor_patch(&records, clear)),
        [ViolationCode::UnresolvedReference]
    );
}

#[test]
fn a_completion_reopened_in_the_same_patch_still_meets_its_guards() {
    let records = report_ready_without_artifact();
    let mutations = "\
- op: transition\n  node: n_final_report\n  transition: complete\n\
- op: transition\n  node: n_final_report\n  transition: reopen\n";
    assert_eq!(
        codes(vendor_patch(&records, mutations)),
        [ViolationCode::GuardFailed]
    );
}

/// Entities within their own limits can still fill the deployment record past its cap: the
/// cap is the backstop behind the per-entity limits.
#[test]
fn a_deployment_past_its_size_cap_is_rejected() {
    use cairn_schema::limits::{EMAIL_BYTES_MAX, EMAIL_COUNT_PER_ENTITY_MAX};
    let records = vendor_after(1);
    let domain = "@example.org";
    let padding = "a".repeat(usize::try_from(EMAIL_BYTES_MAX).unwrap() - domain.len() - 8);
    let entity = |index: u32| cairn_schema::Entity {
        key: format!("e_large_{index}").parse().unwrap(),
        name: "Large".parse().unwrap(),
        emails: (0..EMAIL_COUNT_PER_ENTITY_MAX)
            .map(|email| {
                format!("{index:06}{email:02}{padding}{domain}")
                    .parse()
                    .unwrap()
            })
            .collect(),
    };
    // Enough entities at their email limit to pass 16 MiB of emails alone.
    let per_entity = EMAIL_COUNT_PER_ENTITY_MAX * EMAIL_BYTES_MAX;
    let count = cairn_schema::limits::GRAPH_BYTES_MAX / per_entity + 1;
    let mut patch = support::patch_to(
        &records,
        "{journey: j_vendor_eval}",
        "- op: create_entity\n  entity: {key: e_placeholder, name: Placeholder}\n",
    );
    let creates = (0..count).map(|index| cairn_schema::Mutation::CreateEntity {
        entity: entity(index),
    });
    patch.mutations = cairn_schema::Mutations::new(creates.collect()).unwrap();
    let Err(Rejection::Invalid { violations }) = apply(&records, &patch, &support::fixed_inputs())
    else {
        panic!("a deployment past graph_bytes_max is rejected")
    };
    let found: Vec<_> = violations
        .as_slice()
        .iter()
        .map(|found| (found.code, found.limit))
        .collect();
    assert_eq!(
        found,
        [(
            ViolationCode::LimitExceeded,
            Some(cairn_schema::Limit::GraphBytes)
        )]
    );
}

#[test]
fn a_proposal_is_edited_only_through_its_own_destination() {
    let records = vendor_after(1);
    let create = "- op: create_proposal\n  proposal: {title: A note, destination_revision: 1}\n";
    let target = "{proposal: {id: pr_note, destination: {journey: j_vendor_eval}}}";
    let patch = support::patch_to(&records, target, create);
    let created = apply(&records, &patch, &support::fixed_inputs()).unwrap();
    let elsewhere = "{proposal: {id: pr_note, destination: deployment}}";
    let edit = "- op: edit_proposal\n  proposal: {title: Another note, destination_revision: 1}\n";
    let patch = support::patch_to(created.records(), elsewhere, edit);
    let mut patch = patch;
    patch.base_revision = cairn_schema::Revision::NONE.next();
    let found = codes(apply(created.records(), &patch, &support::fixed_inputs()));
    assert_eq!(found, [ViolationCode::MutationNotForTarget]);
}

#[test]
fn an_archived_journey_takes_no_riding_entity_create() {
    let records = vendor_after(2);
    let archived =
        vendor_patch(&records, "- op: set_journey_status\n  status: archived\n").unwrap();
    let create = "- op: create_entity\n  entity: {key: e_new, name: New Person}\n";
    assert_eq!(
        codes(vendor_patch(archived.records(), create)),
        [ViolationCode::ArchivedJourney]
    );
}

#[test]
fn a_skip_keeps_its_reason_until_reopened() {
    let records = vendor_after(2);
    let skip = "- op: transition\n  node: n_partner_led\n  transition: {skip: {reason: No partner this time.}}\n";
    let skipped = vendor_patch(&records, skip).unwrap();
    let stored = &vendor_graph(skipped.records()).state.nodes[&key("n_partner_led")];
    assert_eq!(
        stored
            .skip_reason
            .as_ref()
            .map(cairn_schema::Reason::as_str),
        Some("No partner this time.")
    );
    let reopen = "- op: transition\n  node: n_partner_led\n  transition: reopen\n";
    let reopened = vendor_patch(skipped.records(), reopen).unwrap();
    assert_eq!(
        vendor_graph(reopened.records()).state.nodes[&key("n_partner_led")].skip_reason,
        None
    );
}

#[test]
fn a_removal_marks_the_route_copied_nodes_that_lose_an_edge() {
    let records = vendor_after(1);
    let removal = "- op: remove_node\n  removal:\n    node: n_access\n    edges: [{node: n_plan, requires: n_access}]\n    resources: [a_access_request]\n";
    let applied = vendor_patch(&records, removal).unwrap();
    let markers = &vendor_graph(applied.records()).state.local_edits;
    assert!(
        markers[&key("n_plan")].contains(&LocalEdit::Requires(key("n_access"))),
        "B4"
    );
}

#[test]
fn an_entity_create_rides_only_in_a_patch_to_something_that_exists() {
    let records = support::seeded("vendor-evaluation");
    let create = "- op: create_entity\n  entity: {key: e_new, name: New Person}\n";
    assert_eq!(
        codes(vendor_patch(&records, create)),
        [ViolationCode::TargetMissing]
    );
}

#[test]
fn applying_a_proposal_that_does_not_create_its_new_destination_is_rejected() {
    let records = support::seeded("vendor-evaluation");
    let target = "{proposal: {id: pr_empty, destination: {journey: j_new}}}";
    let create =
        "- op: create_proposal\n  proposal: {title: Nothing yet, destination_revision: 0}\n";
    let patch = support::patch_to(&records, target, create);
    let created = apply(&records, &patch, &support::fixed_inputs()).unwrap();
    let apply_it = "- op: apply_proposal\n  proposal: pr_empty\n  reviewed_revision: 1\n";
    let patch = support::patch_to(created.records(), "{journey: j_new}", apply_it);
    let found = codes(apply(created.records(), &patch, &support::fixed_inputs()));
    assert_eq!(found, [ViolationCode::TargetMissing]);
}

// Review round 2 regressions.

#[test]
fn replacing_a_decision_with_another_answer_type_marks_its_shape() {
    let records = vendor_after(1);
    let replace = "- op: replace_node\n  node: {key: n_purpose, id: purpose, kind: decision, title: Purchase or research, prompt: Is this evaluation for a purchase or research only?, answer_type: text}\n";
    let applied = vendor_patch(&records, replace).unwrap();
    let markers = &vendor_graph(applied.records()).state.local_edits[&key("n_purpose")];
    assert!(markers.contains(&LocalEdit::Shape), "B4, B7: {markers:?}");
}

#[test]
fn a_proposal_may_delete_its_existing_destination() {
    let records = vendor_after(2);
    let target = "{proposal: {id: pr_delete, destination: {journey: j_vendor_eval}}}";
    let create = "- op: create_proposal\n  proposal:\n    title: Delete the journey\n    destination_revision: 2\n    mutations:\n    - op: delete_journey\n";
    let patch = support::patch_to(&records, target, create);
    let created = apply(&records, &patch, &support::fixed_inputs()).unwrap();
    let apply_it = "- op: apply_proposal\n  proposal: pr_delete\n  reviewed_revision: 1\n";
    let patch = support::patch_to(created.records(), "{journey: j_vendor_eval}", apply_it);
    let applied = apply(created.records(), &patch, &support::fixed_inputs()).unwrap();
    assert!(applied.records().journeys.is_empty());
    let event = &applied.events()[0];
    assert_eq!(
        event.log,
        cairn_schema::Domain::Deployment,
        "A19: the deployment log keeps the deletion"
    );
}

#[test]
fn bypassed_failures_are_recorded_on_the_bypass_event() {
    let records = report_ready_without_artifact();
    let mutations = "\
- op: apply_override\n  node: n_final_report\n  override: {guard_bypass: {guards: [has_artifact], reason: Shared in the meeting.}}\n\
- op: transition\n  node: n_final_report\n  transition: complete\n\
- op: apply_override\n  node: n_final_report\n  override: {force_include: {reason: Always in scope.}}\n";
    let applied = vendor_patch(&records, mutations).unwrap();
    let failures_in = |event: &cairn_schema::Event| {
        event.delta.iter().find_map(|write| match write {
            cairn_schema::Write::Put(cairn_schema::Record::Graph {
                record: cairn_schema::GraphRecord::Overrides { overrides, .. },
                ..
            }) => overrides
                .bypass
                .as_ref()
                .map(|bypass| bypass.failures.clone()),
            _ => None,
        })
    };
    let bypass_event = &applied.events()[0];
    assert_eq!(bypass_event.event_type, EventType::GuardBypassed);
    let recorded = failures_in(bypass_event).unwrap();
    assert!(
        recorded.contains(&GuardFailure::MissingArtifact),
        "D4: the guard-bypassed event says what it bypassed"
    );
    let stored = &vendor_graph(applied.records()).state.overrides[&key("n_final_report")];
    assert_eq!(stored.bypass.as_ref().unwrap().failures, recorded);
}

// Review round 3 regressions.

#[test]
fn a_merge_counts_a_journey_whose_conditions_compare_the_entities() {
    let records = vendor_after(2);
    let prepare = "\
- op: create_entity\n  entity: {key: e_one, name: One}\n\
- op: create_entity\n  entity: {key: e_two, name: Two}\n\
- op: add_node\n  node:\n    key: n_follow_up\n    id: follow-up\n    kind: action\n    title: Follow up\n    relevant_when: {equals: {decision: n_who_owns, value: e_one}}\n";
    let prepared = vendor_patch(&records, prepare).unwrap();
    let records = prepared.records();
    let revision = records.journeys.values().next().unwrap().revision.get();
    let merge = format!(
        "- op: merge_entities\n  survivor: e_two\n  merged: e_one\n  journeys: {{j_vendor_eval: {revision}}}\n"
    );
    let patch = support::patch_to(records, "deployment", &merge);
    assert!(
        apply(records, &patch, &support::fixed_inputs()).is_ok(),
        "E6: the condition makes the journey a referencing one"
    );
    let unnamed = "- op: merge_entities\n  survivor: e_two\n  merged: e_one\n  journeys: {}\n";
    let patch = support::patch_to(records, "deployment", unnamed);
    assert!(
        matches!(
            apply(records, &patch, &support::fixed_inputs()),
            Err(Rejection::Stale { .. })
        ),
        "a merge that leaves it out is stale"
    );
}

#[test]
fn a_proposal_may_unarchive_or_delete_an_archived_journey() {
    let records = vendor_after(2);
    let archived =
        vendor_patch(&records, "- op: set_journey_status\n  status: archived\n").unwrap();
    let records = archived.records();
    let revision = records.journeys.values().next().unwrap().revision.get();
    for (id, mutation) in [
        (
            "pr_restore",
            "- op: set_journey_status\n      status: completed\n",
        ),
        ("pr_delete", "- op: delete_journey\n"),
    ] {
        let target = format!("{{proposal: {{id: {id}, destination: {{journey: j_vendor_eval}}}}}}");
        let create = format!(
            "- op: create_proposal\n  proposal:\n    title: Tidy up\n    destination_revision: {revision}\n    mutations:\n    {mutation}"
        );
        let patch = support::patch_to(records, &target, &create);
        let created = apply(records, &patch, &support::fixed_inputs()).unwrap();
        let apply_it = format!("- op: apply_proposal\n  proposal: {id}\n  reviewed_revision: 1\n");
        let patch = support::patch_to(created.records(), "{journey: j_vendor_eval}", &apply_it);
        assert!(
            apply(created.records(), &patch, &support::fixed_inputs()).is_ok(),
            "{id}"
        );
    }
}

/// A18: a removal sees what earlier mutations in its patch changed, even after another
/// removal in the patch has indexed the graph: a child added in between goes with its
/// parent, an edge added in between widens the removal unless it is named, and a child
/// moved out in between stays.
#[test]
fn a_removal_sees_what_its_patch_added_after_an_earlier_removal() {
    let records = support::journey(&support::add_nodes(&[
        "{key: n_parent, id: parent, kind: group, title: Parent}",
        "{key: n_gone, id: gone, kind: action, title: Gone}",
        "{key: n_other, id: other, kind: action, title: Other}",
        "{key: n_home, id: home, kind: group, title: Home}",
        "{key: n_mover, id: mover, kind: action, title: Mover, parent: n_parent}",
    ]));
    let first = "- op: remove_node\n  removal: {node: n_gone}\n";
    let child = "- op: add_node\n  node: {key: n_child, id: child, kind: action, title: Child, parent: n_parent}\n";
    let edge = "- op: add_edge\n  edge: {node: n_other, requires: n_parent}\n";
    let moved = "- op: set_node_field\n  node: n_mover\n  value: {parent: n_home}\n";
    let parent = |names: &str| {
        format!("- op: remove_node\n  removal: {{node: n_parent, descendants: [n_mover{names}}}\n")
    };
    let holds = |records: &cairn_engine::Records, node: &str| {
        support::graph(records).nodes.get(&key(node)).is_some()
    };
    let with_child =
        support::accepted(&records, &format!("{first}{child}{}", parent(", n_child]")));
    assert!(!holds(&with_child, "n_child"));
    let without_mover = support::accepted(&records, &format!("{first}{moved}{}", parent("]")));
    assert!(holds(&without_mover, "n_mover"));
    assert!(!holds(&without_mover, "n_parent"));
    assert_eq!(
        codes(support::journey_patch(
            &records,
            &format!("{first}{edge}{}", parent("]"))
        )),
        [ViolationCode::RemovalWidened]
    );
    let named = parent("], edges: [{node: n_other, requires: n_parent}]");
    let with_edge = support::accepted(&records, &format!("{first}{edge}{named}"));
    assert!(!holds(&with_edge, "n_parent"));
}
