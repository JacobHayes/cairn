//! D7: what a patch newly caused in derived state, from the two sides' derivations at the
//! same today: completions it made stale, new or larger shortfalls, newly overdue nodes, and
//! the journey becoming stalled; never what the date alone caused.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Records, consequences};
use cairn_schema::{Consequences, StaleConsequence, StaleReason, UndecidedConsequence};
use support::{add_nodes as add, key};

/// D7 over the test journey: `before` against `before` with `patch` applied, both sides
/// derived at `today`.
pub(crate) fn caused(before: &Records, patch: &str, today: &str) -> Consequences {
    let after = support::accepted(before, patch);
    let side = |records: &Records| {
        let graph = support::journey_graph(records, support::JOURNEY);
        let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
        inputs.today = today.parse().unwrap();
        let created_on = records.journeys[&support::JOURNEY.parse().unwrap()]
            .header
            .created_on;
        let derived = cairn_engine::derive(&graph, Some(created_on), &inputs);
        (graph, derived)
    };
    let (before_graph, before) = side(before);
    let (after_graph, after) = side(&after);
    consequences(&before_graph, &before, &after_graph, &after)
}

const WORK: &str = "{key: n_work, id: work, kind: action, title: Work}";

/// D4, D7: reopening a dependency reports the completion that relied on it as stale, with
/// the reason, and a node already stale reports only the reason it gained.
#[test]
fn a_reopen_reports_the_completion_it_made_stale() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_one, id: one, kind: action, title: One}",
            "{key: n_two, id: two, kind: action, title: Two}",
            "{key: n_work, id: work, kind: action, title: Work, requires: [n_one, n_two]}",
        ])),
        "- op: transition\n  node: n_one\n  transition: complete\n\
- op: transition\n  node: n_two\n  transition: complete\n\
- op: transition\n  node: n_work\n  transition: complete\n",
    );
    let reopen = |node: &str| format!("- op: transition\n  node: {node}\n  transition: reopen\n");
    let found = caused(&records, &reopen("n_one"), "2026-10-06");
    assert_eq!(
        found.stale,
        [StaleConsequence {
            node: key("n_work"),
            reasons: BTreeSet::from([StaleReason::Reopened(key("n_one"))]),
        }]
    );
    let again = support::accepted(&records, &reopen("n_one"));
    let gained = caused(&again, &reopen("n_two"), "2026-10-06");
    assert_eq!(
        gained.stale[0].reasons,
        BTreeSet::from([StaleReason::Reopened(key("n_two"))]),
        "a stale node reports only the reason it gained"
    );
}

/// F6, D7: correcting an actual date to one later than the remaining plan allows reports the
/// shortfall it caused on the remaining work, with its chain; never on the fact itself.
#[test]
fn a_late_actual_reports_the_shortfall_of_the_work_it_holds() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_kickoff, id: kickoff, kind: milestone, title: Kickoff}",
            "{key: n_review, id: review, kind: milestone, title: Review}",
            "{key: n_draft, id: draft, kind: action, title: Draft, estimate: 1, not_before: {after: n_kickoff, offset: 5}, due_by: {before: n_review}}",
        ])),
        "- op: set_pin\n  node: n_review\n  date: \"2026-10-10\"\n\
- op: transition\n  node: n_kickoff\n  transition: complete\n\
- op: set_recorded_date\n  node: n_kickoff\n  end: finish\n  date: \"2026-10-02\"\n",
    );
    let late = |on: &str| {
        format!("- op: set_recorded_date\n  node: n_kickoff\n  end: finish\n  date: \"{on}\"\n")
    };
    let found = caused(&records, &late("2026-10-05"), "2026-10-06");
    let draft = found
        .shortfalls
        .iter()
        .find(|shortfall| shortfall.node == key("n_draft"))
        .expect("the draft's shortfall");
    assert_eq!(
        draft.shortfall.shortfall_days, 1,
        "not before the 10th, and due to finish by the 10th with a day's work"
    );
    assert!(
        !draft.shortfall.chain.constraints.is_empty(),
        "with its chain"
    );
    assert!(
        found
            .shortfalls
            .iter()
            .all(|shortfall| shortfall.node != key("n_kickoff")),
        "the fact itself is never short"
    );
    assert!(
        caused(&records, &late("2026-10-03"), "2026-10-06")
            .shortfalls
            .is_empty(),
        "an actual the plan allows causes none"
    );
}

/// D7: both sides derive with the same today, so a patch applied after midnight is never
/// blamed for what the new day made overdue.
#[test]
fn a_patch_across_midnight_reports_nothing_the_date_alone_caused() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}",
            "{key: n_prep, id: prep, kind: action, title: Prep, due_by: {before: n_meeting}}",
        ])),
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-10-06\"\n",
    );
    let note = "- op: add_annotation\n  annotation: {key: a_note, node: n_prep, note: Started thinking.}\n";
    let unrelated = caused(&records, note, "2026-10-07");
    assert_eq!(unrelated, Consequences::default());
    // On the 7th the prep is overdue on both sides; only a before side derived on the 6th
    // would have blamed the note for it.
    let overdue_on = |today: &str| {
        let graph = support::journey_graph(&records, support::JOURNEY);
        let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
        inputs.today = today.parse().unwrap();
        let derived = cairn_engine::derive(&graph, None, &inputs);
        derived.dates().overdue(&key("n_prep"))
    };
    assert!(!overdue_on("2026-10-06") && overdue_on("2026-10-07"));
}

/// D5, D7: a patch that leaves nothing to act on reports the journey becoming stalled.
#[test]
fn a_patch_that_stalls_the_journey_reports_it() {
    let records = support::journey(&add(&[WORK]));
    let snooze = "- op: snooze\n  node: n_work\n  until: {date: \"2026-10-09\"}\n";
    let found = caused(&records, snooze, "2026-10-06");
    let stalled = found.stalled.expect("stalled");
    assert_eq!(stalled.waiting_on.len(), 1);
    let again = support::accepted(&records, snooze);
    let unchanged = caused(
        &again,
        "- op: add_annotation\n  annotation: {key: a_note, node: n_work, note: Later.}\n",
        "2026-10-06",
    );
    assert_eq!(unchanged.stalled, None, "already stalled before");
}

/// D7: a patch that makes work newly overdue reports it.
#[test]
fn a_patch_that_makes_work_overdue_reports_it() {
    let records = support::journey(&add(&[
        "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}",
        "{key: n_prep, id: prep, kind: action, title: Prep, due_by: {before: n_meeting}}",
    ]));
    let found = caused(
        &records,
        "- op: set_pin\n  node: n_meeting\n  date: \"2026-10-05\"\n",
        "2026-10-06",
    );
    assert_eq!(found.overdue, [key("n_meeting"), key("n_prep")]);
}

const FLAG: &str =
    "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";
const BRANCH: &str = "{key: n_branch, id: branch, kind: action, title: Branch, relevant_when: {equals: {decision: n_flag, value: true}}}";

/// D4, D7: finishing undecided work reports that it may not apply, naming the decision its
/// relevance waits on, and nothing stale; the answer that settles it reports nothing more;
/// reopening that decision under the finished work reports the same warning, not `stale`.
#[test]
fn finished_undecided_work_is_reported_as_it_may_not_apply() {
    let records = support::journey(&add(&[FLAG, BRANCH]));
    let complete = "- op: transition\n  node: n_branch\n  transition: complete\n";
    let warned = [UndecidedConsequence {
        node: key("n_branch"),
        unanswered: BTreeSet::from([key("n_flag")]),
    }];
    let finished = caused(&records, complete, "2026-10-06");
    assert_eq!(
        (&finished.undecided[..], &finished.stale[..]),
        (&warned[..], &[][..])
    );
    let done = support::accepted(&records, complete);
    let answer = "- op: answer\n  decision: n_flag\n  value: {boolean: true}\n";
    let settled = caused(&done, answer, "2026-10-06");
    assert_eq!(
        (&settled.undecided[..], &settled.stale[..]),
        (&[][..], &[][..])
    );
    let answered = support::accepted(&done, answer);
    let reopen = "- op: transition\n  node: n_flag\n  transition: reopen\n";
    let reopened = caused(&answered, reopen, "2026-10-06");
    assert_eq!(
        (&reopened.undecided[..], &reopened.stale[..]),
        (&warned[..], &[][..])
    );
}

/// D4, D7: the warning names every decision leaving the work undecided, its ancestors' too,
/// so reopening an ancestor's decision under finished work warns again.
#[test]
fn the_warning_names_the_decisions_of_undecided_ancestors() {
    let records = support::journey(&add(&[
        FLAG,
        "{key: n_other, id: other, kind: decision, title: Other, prompt: Other?, answer_type: boolean}",
        "{key: n_group, id: group, kind: group, title: Group, relevant_when: {equals: {decision: n_other, value: true}}}",
        "{key: n_inner, id: inner, kind: action, title: Inner, parent: n_group, relevant_when: {equals: {decision: n_flag, value: true}}}",
    ]));
    let complete = "- op: transition\n  node: n_inner\n  transition: complete\n";
    let finished = caused(&records, complete, "2026-10-06");
    assert_eq!(
        finished.undecided,
        [UndecidedConsequence {
            node: key("n_inner"),
            unanswered: BTreeSet::from([key("n_flag"), key("n_other")]),
        }]
    );
    let answered = support::accepted(
        &support::accepted(&records, complete),
        "- op: answer\n  decision: n_other\n  value: {boolean: true}\n",
    );
    let reopen = "- op: transition\n  node: n_other\n  transition: reopen\n";
    let reopened = caused(&answered, reopen, "2026-10-06");
    assert_eq!(
        reopened.undecided,
        [UndecidedConsequence {
            node: key("n_inner"),
            unanswered: BTreeSet::from([key("n_other")]),
        }]
    );
}

/// Priority, D7: a decision's answer reports the informational half, never as a warning:
/// the nodes it takes out of scope and the work it newly puts on the acting frontier, the
/// engine's own two sides at the same today. The hiring loop's offer decision.
#[test]
fn answering_a_decision_reports_what_left_scope_and_what_was_unlocked() {
    let name = "hiring-loop";
    let journey = support::scenario(name).journey.to_string();
    let side = |steps: usize| {
        let records = support::after(name, steps);
        let graph = support::journey_graph(&records, &journey);
        let derived = support::derived(&records, &journey);
        (graph, derived)
    };
    let (before_graph, before) = side(5);
    let (after_graph, after) = side(6);
    let found = consequences(&before_graph, &before, &after_graph, &after);
    assert_eq!(found.out_of_scope, [key("n_close_out")]);
    assert_eq!(found.unlocked, [key("n_offer")]);
    assert!(!found.has_warnings(), "informational, never a warning");
}

/// Priority, D7: deciding a branch back in reports it into scope and, once nothing holds it,
/// on the acting frontier; a node the patch adds is neither (the patch names it already).
#[test]
fn deciding_a_branch_back_in_reports_it_into_scope_and_unlocked() {
    let records = support::journey(&add(&[FLAG, BRANCH]));
    let declined = support::accepted(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    let out = caused(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
        "2026-10-06",
    );
    assert_eq!(out.out_of_scope, [key("n_branch")]);
    assert_eq!(out.unlocked, [] as [cairn_schema::NodeKey; 0]);
    let back = caused(
        &declined,
        "- op: answer\n  decision: n_flag\n  value: {boolean: true}\n",
        "2026-10-06",
    );
    assert_eq!(back.into_scope, [key("n_branch")]);
    assert_eq!(back.unlocked, [key("n_branch")]);
    assert_eq!(back.out_of_scope, [] as [cairn_schema::NodeKey; 0]);
    let added = caused(
        &records,
        &add(&["{key: n_new, id: new, kind: action, title: New}"]),
        "2026-10-06",
    );
    assert_eq!(
        added,
        Consequences::default(),
        "an added node is not unlocked"
    );
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    assert_eq!(
        consequences(&graph, &derived, &graph, &derived),
        Consequences::default(),
        "a patch that changes nothing reports nothing"
    );
}
