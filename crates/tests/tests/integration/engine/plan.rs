//! The plan layer (F5; Invariants: the date line): contradictory chains, pinned or not, are
//! rejected with their chains, sources, pins, shortfall, and resolution moves; every move
//! resolves its chain; a negative or zero cycle is legal; actuals and today never reject.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Applied, Records};
use cairn_schema::{
    ChainList, ConstraintSource, FixedBy, Mutation, Rejection, ShortChain, ViolationCode,
};
use support::key;

/// `add_node` mutations, one per node written as a YAML flow mapping.
fn add(nodes: &[&str]) -> String {
    let mut mutations = String::new();
    for node in nodes {
        mutations.push_str("- op: add_node\n  node: ");
        mutations.push_str(node);
        mutations.push('\n');
    }
    mutations
}

/// The contradictory chains a rejection lists, or a panic if there are none.
fn chains(result: Result<Applied, Rejection>) -> ChainList {
    let Err(Rejection::Invalid { violations }) = result else {
        panic!("expected a rejection, got {result:#?}");
    };
    let found = violations
        .as_slice()
        .iter()
        .find(|violation| violation.code == ViolationCode::ContradictoryChain);
    found
        .and_then(|violation| violation.chains.clone())
        .unwrap_or_else(|| panic!("no contradictory chain in {violations:#?}"))
}

fn only(list: &ChainList) -> &ShortChain {
    assert_eq!(list.chains.len(), 1, "{list:#?}");
    assert!(!list.more);
    &list.chains.as_slice()[0]
}

fn rule_nodes(chain: &ShortChain) -> Vec<String> {
    chain
        .chain
        .constraints
        .iter()
        .filter_map(|constraint| match &constraint.source {
            ConstraintSource::DueBy { node } | ConstraintSource::NotBefore { node } => {
                Some(node.to_string())
            }
            _ => None,
        })
        .collect()
}

/// Every resolution move, applied with the patch it resolves, is accepted (F5: resolution is
/// an ordinary patch).
fn each_move_resolves(records: &Records, mutations: &str, short: &ShortChain) {
    assert!(!short.resolutions.is_empty(), "a short chain offers moves");
    for resolution in &short.resolutions {
        let resolution = cairn_schema::to_json(resolution).unwrap();
        let both = format!("{mutations}- {resolution}\n");
        if let Err(rejection) = support::journey_patch(records, &both) {
            panic!("{resolution} does not resolve the chain: {rejection:#?}");
        }
    }
}

const MILESTONES: [&str; 2] = [
    "{key: n_review, id: review, kind: milestone, title: Review}",
    "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}",
];

/// F5: two rules that each place a milestone before the other contradict with no pin.
#[test]
fn two_rules_placing_a_milestone_before_each_other_are_rejected_with_no_pin() {
    let records = support::journey(&add(&MILESTONES));
    let mutations = "- op: set_node_field\n  node: n_review\n  value: {due_by: {before: n_meeting, offset: 1}}\n\
- op: set_node_field\n  node: n_meeting\n  value: {due_by: {before: n_review}}\n";
    let list = chains(support::journey_patch(&records, mutations));
    let short = only(&list);
    assert_eq!(short.shortfall_days, 1);
    assert!(short.chain.fixed.is_empty(), "no pin on it");
    let mut sources = rule_nodes(short);
    sources.sort();
    assert_eq!(sources, ["n_meeting", "n_review"]);
    each_move_resolves(&records, mutations, short);
}

/// Cycles: a negative cycle is legal, the same loop with offsets that add up to more time
/// than it allows is not.
#[test]
fn a_cycle_is_a_contradiction_only_when_it_gains_days() {
    let records = support::journey(&add(&MILESTONES));
    let rules = |direction: &str| {
        format!(
            "- op: set_node_field\n  node: n_review\n  value: {{not_before: {{{direction}: n_meeting, offset: 1}}}}\n\
- op: set_node_field\n  node: n_meeting\n  value: {{not_before: {{{direction}: n_review, offset: 1}}}}\n"
        )
    };
    let within_a_day = support::journey_patch(&records, &rules("before"));
    assert!(within_a_day.is_ok(), "{within_a_day:#?}");
    let list = chains(support::journey_patch(&records, &rules("after")));
    assert_eq!(only(&list).shortfall_days, 2);
}

/// F5: two pins with a chain between them that needs more days than they allow.
#[test]
fn pins_too_close_for_the_work_between_them_are_rejected() {
    let records = support::journey(&add(&[
        MILESTONES[0],
        MILESTONES[1],
        "{key: n_report, id: report, kind: deliverable, title: Report, estimate: 5, not_before: {after: n_review}, due_by: {before: n_meeting}}",
    ]));
    let mutations = "- op: set_pin\n  node: n_review\n  date: \"2026-10-10\"\n\
- op: set_pin\n  node: n_meeting\n  date: \"2026-10-12\"\n";
    let list = chains(support::journey_patch(&records, mutations));
    let short = only(&list);
    assert_eq!(short.shortfall_days, 3);
    let pins: Vec<_> = short
        .chain
        .fixed
        .iter()
        .map(|fixed| fixed.fixed_by)
        .collect();
    assert_eq!(pins, [FixedBy::Pin, FixedBy::Pin]);
    let shifts: Vec<&Mutation> = short
        .resolutions
        .iter()
        .filter(|found| matches!(found, Mutation::ShiftPin { .. }))
        .collect();
    assert_eq!(shifts.len(), 2, "either pin moves");
    each_move_resolves(&records, mutations, short);
}

/// F5, A15: past the chain limit the rejection lists the limit and says there were more.
#[test]
fn a_rejection_past_the_chain_limit_says_so() {
    let limit = cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let mut nodes = Vec::new();
    for at in 0..=limit {
        nodes.push(format!(
            "{{key: n_first_{at}, id: first-{at}, kind: milestone, title: First, due_by: {{before: n_second_{at}, offset: 2}}}}"
        ));
        nodes.push(format!(
            "{{key: n_second_{at}, id: second-{at}, kind: milestone, title: Second, due_by: {{before: n_first_{at}}}}}"
        ));
    }
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let created = support::journey(&add(&MILESTONES));
    let list = chains(support::journey_patch(&created, &add(&nodes)));
    assert_eq!(list.chains.len(), limit);
    assert!(list.more);
}

/// F5: the illustrative example's report pinned later than its decision meeting allows is
/// rejected with its chain, and every move it lists resolves it: moving or dropping the
/// report's pin, answering the meeting date later, or letting the stage not close.
#[test]
fn the_examples_later_report_pin_is_rejected_with_its_chain() {
    let records = support::vendor_after(2);
    let mutations = "- op: set_pin\n  node: n_final_report\n  date: \"2026-11-25\"\n";
    let list = chains(support::vendor_patch(&records, mutations));
    let short = only(&list);
    assert_eq!(short.shortfall_days, 5);
    let sources: Vec<&ConstraintSource> =
        short.chain.constraints.iter().map(|c| &c.source).collect();
    assert!(sources.contains(&&ConstraintSource::StageClose {
        group: key("n_final_review")
    }));
    assert!(sources.contains(&&ConstraintSource::Containment {
        parent: key("n_final_review"),
        child: key("n_final_report")
    }));
    let reanswer = short.resolutions.iter().any(|found| {
        matches!(found, Mutation::Answer { decision, .. } if *decision == key("n_meeting_date"))
    });
    assert!(
        reanswer,
        "the meeting's pin moves through its decision (E3)"
    );
    for resolution in &short.resolutions {
        let resolution = cairn_schema::to_json(resolution).unwrap();
        let both = format!("{mutations}- {resolution}\n");
        let resolved = support::vendor_patch(&records, &both);
        assert!(resolved.is_ok(), "{resolution}: {resolved:#?}");
    }
}

/// F5: answering a date decision that feeds a milestone is a pin, rejected the same way.
#[test]
fn a_feeding_answer_is_checked_as_a_pin() {
    let records = support::vendor_after(2);
    let pinned = support::accepted_on(
        &records,
        "j_vendor_eval",
        "- op: set_pin\n  node: n_final_report\n  date: \"2026-11-02\"\n",
    );
    let earlier = "- op: answer\n  decision: n_meeting_date\n  value: {date: \"2026-10-30\"}\n";
    let list = chains(support::vendor_patch(&pinned, earlier));
    assert_eq!(only(&list).shortfall_days, 3);
}

/// Construction validation: a route file whose rules contradict does not load.
#[test]
fn a_route_with_a_contradictory_chain_does_not_load() {
    let yaml = "format: 1\nroute: test\nname: Test\nnodes:\n\
- {key: n_a, id: a, kind: milestone, title: A, not_before: {after: b, offset: 3}}\n\
- {key: n_b, id: b, kind: milestone, title: B, not_before: {after: a}}\n";
    let file = cairn_schema::from_yaml(yaml).unwrap();
    let violations =
        cairn_engine::from_file(&file, &mut cairn_schema::SequentialKeys::default()).unwrap_err();
    let codes: Vec<_> = violations
        .as_slice()
        .iter()
        .map(|found| found.code)
        .collect();
    assert_eq!(codes, [ViolationCode::ContradictoryChain]);
}

/// F6: reality is never rejected. The launch's late code freeze is accepted, and an unrelated
/// patch after it is too.
#[test]
fn a_late_actual_never_rejects_a_patch() {
    let finished = support::finished("product-launch");
    let note = "- op: add_annotation\n  annotation: {key: a_note, node: n_docs, note: Drafting.}\n";
    let patch = support::patch_to(&finished, "{journey: j_launch}", note);
    let applied = cairn_engine::apply(&finished, &patch, &support::fixed_inputs());
    assert!(applied.is_ok(), "{applied:#?}");
}

// Review round 1 regressions.

/// F5: when one answer fixes both ends of a chain (a date decision and the milestone it
/// feeds), moving it moves both, so the rejection offers the moves that close the gap
/// instead.
#[test]
fn an_answer_at_both_ends_is_not_offered_as_a_move() {
    let records = support::journey(&add(&[
        "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}",
        "{key: n_date, id: date, kind: decision, title: Date, prompt: When?, answer_type: date, feeds_milestone: n_meeting}",
        "{key: n_prep, id: prep, kind: action, title: Prep, estimate: 3, not_before: {after: n_date}, due_by: {before: n_meeting}}",
    ]));
    let mutations = "- op: answer\n  decision: n_date\n  value: {date: \"2026-10-20\"}\n";
    let list = chains(support::journey_patch(&records, mutations));
    let short = only(&list);
    assert_eq!(short.shortfall_days, 3);
    assert!(
        !short
            .resolutions
            .iter()
            .any(|found| matches!(found, Mutation::Answer { .. })),
        "{:#?}",
        short.resolutions
    );
    each_move_resolves(&records, mutations, short);
}

/// E6, F5: a merge whose aliases make contradictory work apply is rejected with the chains,
/// whether it is alone in its patch or shares it with an unrelated merge before or after it
/// (each merge adds its journeys to the check; none replaces another's).
#[test]
fn a_merge_that_breaks_the_plan_carries_its_chains() {
    let records = support::vendor_after(2);
    let prepare = "\
- op: create_entity\n  entity: {key: e_other, name: Other}\n\
- op: create_entity\n  entity: {key: e_x, name: X}\n\
- op: create_entity\n  entity: {key: e_y, name: Y}\n\
- op: add_node\n  node: {key: n_start, id: start, kind: milestone, title: Start}\n\
- op: set_pin\n  node: n_start\n  date: \"2026-11-10\"\n\
- op: add_node\n  node: {key: n_long, id: long, kind: action, title: Long, estimate: 30, relevant_when: {equals: {decision: n_who_owns, value: e_other}}, not_before: {after: n_start}, due_by: {before: n_decision_meeting}}\n";
    let prepared = support::vendor_patch(&records, prepare).unwrap();
    let records = prepared.records();
    let revision = records.journeys.values().next().unwrap().revision.get();
    let breaking = format!(
        "- op: merge_entities\n  survivor: e_other\n  merged: e_lead\n  journeys: {{j_vendor_eval: {revision}}}\n"
    );
    let unrelated = "- op: merge_entities\n  survivor: e_x\n  merged: e_y\n  journeys: {}\n";
    for mutations in [
        breaking.clone(),
        format!("{breaking}{unrelated}"),
        format!("{unrelated}{breaking}"),
    ] {
        let patch = support::patch_to(records, "deployment", &mutations);
        let Err(Rejection::Invalid { violations }) =
            cairn_engine::apply(records, &patch, &support::fixed_inputs())
        else {
            panic!("the merge makes the long action apply:\n{mutations}");
        };
        let found = &violations.as_slice()[0];
        assert_eq!(found.code, ViolationCode::MergeBreaksJourney, "{mutations}");
        let chains = found.chains.as_ref().expect("the chains ride along");
        assert_eq!(chains.chains.as_slice()[0].shortfall_days, 20);
    }
}

// Review round 3 regression.

/// F5: a move past the last date there is resolves nothing, so it is not offered.
#[test]
fn no_move_leaves_the_dates_there_are() {
    let records = support::journey(&add(&[
        "{key: n_start, id: start, kind: milestone, title: Start}",
        "{key: n_end, id: end, kind: decision, title: End, prompt: When?, answer_type: date}",
        "{key: n_work, id: work, kind: action, title: Work, estimate: 1, not_before: {after: n_start}, due_by: {before: n_end}}",
    ]));
    let mutations = "- op: set_pin\n  node: n_start\n  date: \"9999-12-31\"\n\
- op: answer\n  decision: n_end\n  value: {date: \"9999-12-31\"}\n";
    let list = chains(support::journey_patch(&records, mutations));
    each_move_resolves(&records, mutations, only(&list));
}
