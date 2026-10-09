//! Routes, the deployment, and proposals (A11, A19, B7, B9, E6, H3, I6), and J2: every
//! mutation type the patch format declares applies and emits its event.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_engine::{Records, apply};
use cairn_schema::{
    EventType, Lineage, Mutation, Patch, ProposalStatus, Rejection, RevisionOf, VersionNumber,
    ViolationCode,
};
use serde_json::Value;

/// Applies mutations to a target, written as YAML; panics on a rejection.
fn accept(records: &Records, target: &str, mutations: &str, ops: &mut BTreeSet<String>) -> Records {
    let patch = support::patch_to(records, target, mutations);
    let applied = apply(records, &patch, &support::fixed_inputs())
        .unwrap_or_else(|rejection| panic!("{rejection:#?}\n{mutations}"));
    assert_eq!(applied.events().len(), patch.mutations.len(), "J2");
    collect(&patch, ops);
    applied.records().clone()
}

fn collect(patch: &Patch, ops: &mut BTreeSet<String>) {
    for mutation in patch.mutations.as_slice() {
        ops.insert(op_name(mutation));
        if let Mutation::CreateProposal { proposal } | Mutation::EditProposal { proposal } =
            mutation
        {
            ops.extend(proposal.mutations.as_slice().iter().map(op_name));
        }
    }
}

fn op_name(mutation: &Mutation) -> String {
    serde_json::to_value(mutation).unwrap()["op"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The `op` names the patch format's JSON Schema declares, so this follows the type.
fn declared_ops() -> BTreeSet<String> {
    let schema = serde_json::to_value(schemars::schema_for!(Mutation)).unwrap();
    let mut names = BTreeSet::new();
    let mut stack = vec![&schema];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(map) => {
                let constant = map
                    .get("properties")
                    .and_then(|p| p.get("op"))
                    .and_then(|p| p.get("const"));
                if let Some(constant) = constant {
                    names.insert(constant.as_str().unwrap().to_owned());
                }
                stack.extend(map.values());
            }
            Value::Array(items) => stack.extend(items),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    names
}

const ROUTE: &str = "{route: vendor-evaluation}";
const JOURNEY: &str = "{journey: j_vendor_eval}";

/// Draft edits touching every structural mutation.
const DRAFT_EDITS: &str = "\
- op: edit_route\n  name: Vendor evaluation revised\n\
- op: set_node_field\n  node: n_access\n  value: {estimate: 3}\n\
- op: add_node\n  node: {key: n_signoff, id: signoff, kind: milestone, title: Sign-off}\n\
- op: add_edge\n  edge: {node: n_signoff, requires: n_findings}\n\
- op: remove_edge\n  edge: {node: n_signoff, requires: n_findings}\n\
- op: add_role\n  role: {key: r_approver, id: approver}\n\
- op: edit_role\n  role: {key: r_approver, id: approver, title: Approver}\n\
- op: add_participation_kind\n  kind: {key: k_approver, id: approver}\n\
- op: edit_participation_kind\n  kind: {key: k_approver, id: approver, title: Approver}\n\
- op: set_participation\n  node: n_signoff\n  kind: k_approver\n  source: r_approver\n\
- op: clear_participation\n  node: n_signoff\n  kind: k_approver\n\
- op: remove_role\n  role: r_approver\n\
- op: remove_participation_kind\n  kind: k_approver\n\
- op: add_resource\n  node: n_signoff\n  resource: {key: a_signoff_tip, tip: Get it in writing.}\n\
- op: edit_resource\n  node: n_signoff\n  resource: {key: a_signoff_tip, tip: Get it in writing and signed.}\n\
- op: remove_resource\n  node: n_signoff\n  resource: a_signoff_tip\n\
- op: replace_node\n  node: {key: n_signoff, id: signoff, kind: action, title: Sign off}\n\
- op: set_default_owner\n\
- op: set_default_owner\n  role: r_eval_owner\n\
- op: add_node\n  node: {key: n_temporary, id: temporary, kind: action, title: Temporary}\n\
- op: remove_node\n  removal: {node: n_temporary}\n";

/// A11: a draft opened from the latest version, edited, and published as the next version;
/// an imported draft discarded; a route created, retired, and brought back.
fn walk_route(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let records = accept(records, ROUTE, "- op: open_draft\n  source: edit\n", ops);
    let draft = &records.routes.values().next().unwrap().draft;
    let first = records.versions.values().next().unwrap();
    assert_eq!(
        draft.as_ref().unwrap().graph,
        first.graph,
        "the draft copies version 1"
    );
    let records = accept(&records, ROUTE, DRAFT_EDITS, ops);
    let records = accept(&records, ROUTE, "- op: publish_draft\n", ops);
    let route = records.routes.values().next().unwrap();
    assert!(route.draft.is_none(), "publishing clears the draft");
    assert_eq!(route.versions.len(), 2);
    let version = Lineage {
        route: route.header.id.clone(),
        version: VersionNumber::FIRST.next(),
    };
    assert!(
        records.versions[&version]
            .graph
            .nodes
            .get(&support::key("n_signoff"))
            .is_some()
    );
    let records = accept(&records, ROUTE, "- op: open_draft\n  source: import\n", ops);
    let again = support::patch_to(&records, ROUTE, "- op: open_draft\n  source: edit\n");
    assert_eq!(
        support::codes(apply(&records, &again, &support::fixed_inputs())),
        [ViolationCode::DraftExists]
    );
    let records = accept(&records, ROUTE, "- op: discard_draft\n", ops);
    let records = accept(
        &records,
        ROUTE,
        "- op: set_route_retired\n  retired: true\n",
        ops,
    );
    let records = accept(
        &records,
        ROUTE,
        "- op: set_route_retired\n  retired: false\n",
        ops,
    );
    accept(
        &records,
        "{route: scratch}",
        "- op: create_route\n  name: Scratch\n",
        ops,
    )
}

/// A21: a segment refuses a second root, a default owner, a final milestone, and an empty
/// publish, and is never started from nor saved to. Answers the records with it published.
fn segment_rules(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    const SEGMENT: &str = "{route: reuse}";
    let reject = |records: &Records, target: &str, mutations: &str| {
        let patch = support::patch_to(records, target, mutations);
        support::codes(apply(records, &patch, &support::fixed_inputs()))
    };
    // A root id at the length limit has no suffix that fits, so a second insertion under the
    // same parent is refused rather than searching for one.
    let id = format!("reuse-{}", "x".repeat(58));
    let root =
        format!("- op: add_node\n  node: {{key: n_reuse, id: {id}, kind: group, title: Reuse}}\n");
    let records = accept(
        records,
        SEGMENT,
        "- op: create_route\n  name: Reuse\n  kind: segment\n- op: open_draft\n  source: import\n",
        ops,
    );
    let rules = [
        "- op: add_node\n  node: {key: n_two, id: two, kind: group, title: Two}\n",
        "- op: add_role\n  role: {key: r_lead, id: lead}\n- op: set_default_owner\n  role: r_lead\n",
        "- op: add_node\n  node: {key: n_end, id: end, parent: n_reuse, kind: milestone, title: End, final: true}\n",
    ];
    for rule in rules {
        assert_eq!(
            reject(&records, SEGMENT, &format!("{root}{rule}")),
            [ViolationCode::SegmentRule]
        );
    }
    assert_eq!(
        reject(&records, SEGMENT, "- op: publish_draft\n"),
        [ViolationCode::SegmentRule]
    );
    let child = "- op: add_node\n  node: {key: n_check, id: check, parent: n_reuse, kind: action, title: Check}\n";
    let records = accept(
        &records,
        SEGMENT,
        &format!("{root}{child}- op: publish_draft\n"),
        ops,
    );
    let start = "- op: create_journey\n  name: Started\n  from: {route: reuse, version: 1}\n";
    let saved = "- op: open_draft\n  source: {save_as_route: {journey: j_vendor_eval}}\n";
    for (target, mutations) in [("{journey: j_reuse}", start), (SEGMENT, saved)] {
        assert_eq!(
            reject(&records, target, mutations),
            [ViolationCode::NotAProcessRoute]
        );
    }
    records
}

/// A21, B13: a segment refuses a second root, a default owner, a final milestone, and an
/// empty publish, and is never started from; inserted into a route draft it survives the
/// publish, and a journey started from that version holds its nodes as the route's own.
fn walk_segment(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let reject = |records: &Records, target: &str, mutations: &str| {
        let patch = support::patch_to(records, target, mutations);
        support::codes(apply(records, &patch, &support::fixed_inputs()))
    };
    let records = segment_rules(records, ops);
    let insert = "- op: open_draft\n  source: edit\n- op: insert_segment\n  insertion: i_reuse\n  segment: {route: reuse, version: 1}\n  parent: n_reporting\n";
    let records = accept(&records, ROUTE, insert, ops);
    let again = insert
        .replace("open_draft\n  source: edit\n- op: ", "")
        .replace("i_reuse", "i_again");
    assert_eq!(
        reject(&records, ROUTE, &again),
        [ViolationCode::InsertionInvalid]
    );
    let records = accept(&records, ROUTE, "- op: publish_draft\n", ops);
    assert!(
        records
            .versions
            .values()
            .any(|v| v.graph.insertions.len() == 1)
    );
    let start =
        "- op: create_journey\n  name: Started\n  from: {route: vendor-evaluation, version: 3}\n";
    let started = accept(&records, "{journey: j_reuse}", start, ops);
    let graph = &started.journeys[&"j_reuse".parse().unwrap()].graph;
    let from_route = cairn_schema::Provenance::FromRoute;
    assert!(graph.insertions.is_empty());
    assert!(
        graph
            .state
            .nodes
            .values()
            .all(|state| state.provenance == from_route)
    );
    records
}

/// B7, B9, B4, F2, E3, B6, B5, B10, G1, B11 on the finished vendor journey.
fn walk_journey(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let changes = "\
- op: upgrade\n  to: 2\n\
- op: relink\n  lineage: {route: vendor-evaluation, version: 2}\n\
- op: set_provenance\n  node: n_access\n  provenance: orphaned\n\
- op: set_provenance\n  node: n_access\n  provenance: from_route\n\
- op: mark_local_edit\n  node: n_plan\n  edit: {field: title}\n  marked: true\n\
- op: mark_local_edit\n  node: n_plan\n  edit: {field: title}\n  marked: false\n\
- op: edit_journey\n  name: Evaluate the analytics vendor again\n\
- op: set_recorded_date\n  node: n_access\n  end: start\n  date: \"2026-10-05\"\n\
- op: add_role\n  role: {key: r_helper, id: helper}\n\
- op: fill_role\n  role: r_helper\n  entities: [e_lead]\n\
- op: clear_role_fill\n  role: r_helper\n\
- op: set_pin\n  node: n_review_opens\n  date: \"2026-11-01\"\n\
- op: shift_pin\n  node: n_review_opens\n  offset_days: -2\n\
- op: clear_pin\n  node: n_review_opens\n\
- op: snooze\n  node: n_decision_meeting\n  until: {date: \"2026-11-10\"}\n\
- op: unsnooze\n  node: n_decision_meeting\n\
- op: apply_override\n  node: n_partner_led\n  override: {force_include: {reason: The partner may join.}}\n\
- op: remove_override\n  node: n_partner_led\n  kind: force_include\n\
- op: set_atomic\n  node: n_workload\n  atomic: true\n\
- op: add_annotation\n  annotation: {key: a_link, node: n_findings, reference: \"https://example.org/notes\"}\n\
- op: edit_annotation\n  annotation: {key: a_link, node: n_findings, reference: \"https://example.org/notes-v2\"}\n\
- op: remove_annotation\n  annotation: a_link\n\
- op: set_journey_status\n  status: completed\n\
- op: set_journey_status\n  status: active\n";
    let records = accept(records, JOURNEY, changes, ops);
    let journey = records.journeys.values().next().unwrap();
    assert_eq!(
        journey.header.lineage.as_ref().unwrap().version,
        VersionNumber::FIRST.next()
    );
    assert_eq!(
        journey.graph.state.nodes[&support::key("n_access")].started_on,
        Some("2026-10-05".parse().unwrap())
    );
    records
}

/// E6, H3: an entity's emails edited, and two entities merged with the journeys that
/// reference them named at their revisions.
fn walk_deployment(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let edit = "- op: edit_entity\n  entity: {key: e_stakeholder_a, name: Stakeholder A, emails: [a@example.org]}\n";
    let records = accept(records, "deployment", edit, ops);
    let journey = records.journeys.values().next().unwrap().revision;
    let merge = format!(
        "- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {{j_vendor_eval: {}}}\n",
        journey.get()
    );
    let stale = format!(
        "- op: merge_entities\n  survivor: e_stakeholder_a\n  merged: e_stakeholder_b\n  journeys: {{j_vendor_eval: {}}}\n",
        journey.get() - 1
    );
    let patch = support::patch_to(&records, "deployment", &stale);
    let Err(Rejection::Stale { conflicts, .. }) = apply(&records, &patch, &support::fixed_inputs())
    else {
        panic!("a merge naming a journey's old revision is stale (E6)")
    };
    assert!(matches!(conflicts[0].of, RevisionOf::Domain(_)));
    let records = accept(&records, "deployment", &merge, ops);
    let deployment = &records.deployment;
    assert!(
        deployment
            .entities
            .get(&"e_stakeholder_b".parse().unwrap())
            .is_none()
    );
    assert_eq!(
        deployment.aliases[&"e_stakeholder_b".parse().unwrap()],
        "e_stakeholder_a".parse().unwrap()
    );
    records
}

/// A proposal create or edit to the vendor journey adding one note.
fn draft(op: &str, title: &str, destination_revision: u32) -> String {
    format!(
        "- op: {op}\n  proposal:\n    title: {title}\n    destination_revision: {destination_revision}\n    mutations:\n    - op: add_annotation\n      annotation: {{key: a_proposed, note: Proposed.}}\n"
    )
}

fn journey_revision(records: &Records) -> u32 {
    records.journeys.values().next().unwrap().revision.get()
}

/// I6, H5: a proposal drafted and edited without touching its destination, then applied;
/// a second apply finds it applied; another is discarded.
fn walk_proposals(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let proposal = "{proposal: {id: pr_note, destination: {journey: j_vendor_eval}}}";
    let before = journey_revision(records);
    let records = accept(
        records,
        proposal,
        &draft("create_proposal", "Add a note", before),
        ops,
    );
    let records = accept(
        &records,
        proposal,
        &draft("edit_proposal", "Add a short note", before),
        ops,
    );
    assert_eq!(
        journey_revision(&records),
        before,
        "drafting does not advance the destination (H5)"
    );
    let stale = "- op: apply_proposal\n  proposal: pr_note\n  reviewed_revision: 1\n";
    let patch = support::patch_to(&records, JOURNEY, stale);
    let result = apply(&records, &patch, &support::fixed_inputs());
    assert!(
        matches!(result, Err(Rejection::Stale { .. })),
        "the reviewer saw revision 1; it is at 2"
    );
    let records = apply_note_proposal(&records, ops);
    let other = "{proposal: {id: pr_other, destination: {journey: j_vendor_eval}}}";
    let created = draft(
        "create_proposal",
        "Something else",
        journey_revision(&records),
    );
    let records = accept(&records, other, &created, ops);
    accept(&records, other, "- op: discard_proposal\n", ops)
}

/// I6, H2: applying commits the proposal's mutations with its applied status, attributed to
/// its author and confirmed by the user applying it; applying again finds it applied.
fn apply_note_proposal(records: &Records, ops: &mut BTreeSet<String>) -> Records {
    let apply_it = "- op: apply_proposal\n  proposal: pr_note\n  reviewed_revision: 2\n";
    let patch = support::patch_to(records, JOURNEY, apply_it);
    let applied = apply(records, &patch, &support::fixed_inputs()).unwrap();
    let event = &applied.events()[0];
    assert_eq!(event.event_type, EventType::ProposalApplied);
    assert_eq!(
        event.confirming_user,
        Some(support::fixed_inputs().actor.user)
    );
    collect(&patch, ops);
    let records = applied.records().clone();
    assert_eq!(
        records.proposals.values().next().unwrap().status,
        ProposalStatus::Applied
    );
    let annotations = &records
        .journeys
        .values()
        .next()
        .unwrap()
        .graph
        .state
        .annotations;
    assert!(annotations.get(&"a_proposed".parse().unwrap()).is_some());
    let again = "- op: apply_proposal\n  proposal: pr_note\n  reviewed_revision: 3\n";
    let twice = support::patch_to(&records, JOURNEY, again);
    let found = support::codes(apply(&records, &twice, &support::fixed_inputs()));
    assert_eq!(found, [ViolationCode::ProposalNotOpen]);
    records
}

#[test]
fn every_mutation_type_applies_and_emits_its_event() {
    let mut ops = BTreeSet::new();
    for name in support::fixture_names() {
        for step in support::scenario(&name).steps.as_slice() {
            collect(&step.patch, &mut ops);
        }
    }
    let records = support::finished("vendor-evaluation");
    let records = walk_route(&records, &mut ops);
    let records = walk_segment(&records, &mut ops);
    let records = walk_journey(&records, &mut ops);
    let records = walk_deployment(&records, &mut ops);
    let records = walk_proposals(&records, &mut ops);
    let records = accept(&records, JOURNEY, "- op: delete_journey\n", &mut ops);
    assert!(records.journeys.is_empty());
    assert_eq!(ops, declared_ops());
}

#[test]
fn a_patch_to_the_wrong_kind_of_target_is_rejected() {
    let records = support::finished("vendor-evaluation");
    let cases = [
        (
            ROUTE,
            "- op: transition\n  node: n_access\n  transition: start\n",
        ),
        (
            "deployment",
            "- op: add_node\n  node: {key: n_x, id: x, kind: action, title: X}\n",
        ),
        (JOURNEY, "- op: publish_draft\n"),
    ];
    for (target, mutations) in cases {
        let patch = support::patch_to(&records, target, mutations);
        assert_eq!(
            support::codes(apply(&records, &patch, &support::fixed_inputs())),
            [ViolationCode::MutationNotForTarget],
            "{target}"
        );
    }
    let no_draft = support::patch_to(
        &records,
        ROUTE,
        "- op: add_node\n  node: {key: n_x, id: x, kind: action, title: X}\n",
    );
    assert_eq!(
        support::codes(apply(&records, &no_draft, &support::fixed_inputs())),
        [ViolationCode::NoDraft]
    );
}
