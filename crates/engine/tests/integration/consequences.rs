//! D7: what a patch newly caused in derived state, from the two sides' derivations at the
//! same today: completions it made stale, new or larger shortfalls, newly overdue nodes, and
//! the journey becoming stalled; never what the date alone caused.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_engine::{Records, consequences};
use cairn_schema::{Consequences, GuardFailure, StaleConsequence, UndecidedConsequence};
use support::{add_nodes as add, key};

/// D7 over the test journey: `before` against `before` with `patch` applied, both sides
/// derived at `today`.
fn caused(before: &Records, patch: &str, today: &str) -> Consequences {
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

/// D4, D7: an inserted dependency reports the completion it made stale, with the reason.
#[test]
fn an_inserted_dependency_reports_the_completion_it_made_stale() {
    let records = support::accepted(
        &support::journey(&add(&[WORK])),
        "- op: transition\n  node: n_work\n  transition: complete\n",
    );
    let insert = format!(
        "{}- op: add_edge\n  edge: {{node: n_work, requires: n_new}}\n",
        add(&["{key: n_new, id: new, kind: action, title: New}"])
    );
    let found = caused(&records, &insert, "2026-10-06");
    assert_eq!(
        found.stale,
        [StaleConsequence {
            node: key("n_work"),
            reasons: BTreeSet::from([GuardFailure::OpenDependency(key("n_new"))]),
        }]
    );
    let again = support::accepted(&records, &insert);
    let more = format!(
        "{}- op: add_edge\n  edge: {{node: n_work, requires: n_more}}\n",
        add(&["{key: n_more, id: more, kind: action, title: More}"])
    );
    let gained = caused(&again, &more, "2026-10-06");
    assert_eq!(
        gained.stale[0].reasons,
        BTreeSet::from([GuardFailure::OpenDependency(key("n_more"))]),
        "a stale node reports only the reason it gained"
    );
}

/// F6, D7: correcting an actual date to one later than the chain allows reports the
/// shortfall it caused, with its chain.
#[test]
fn a_late_actual_reports_its_shortfall() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_review, id: review, kind: milestone, title: Review}",
            "{key: n_draft, id: draft, kind: action, title: Draft, due_by: {before: n_review}}",
        ])),
        "- op: set_pin\n  node: n_review\n  date: \"2026-10-03\"\n\
- op: transition\n  node: n_draft\n  transition: complete\n\
- op: set_recorded_date\n  node: n_draft\n  end: finish\n  date: \"2026-10-01\"\n",
    );
    let late = "- op: set_recorded_date\n  node: n_draft\n  end: finish\n  date: \"2026-10-05\"\n";
    let found = caused(&records, late, "2026-10-06");
    let draft = found
        .shortfalls
        .iter()
        .find(|shortfall| shortfall.node == key("n_draft"))
        .expect("the draft's shortfall");
    assert_eq!(
        draft.shortfall.shortfall_days, 2,
        "finished the 5th, due the 3rd"
    );
    assert!(
        !draft.shortfall.chain.constraints.is_empty(),
        "with its chain"
    );
    assert!(
        caused(
            &records,
            "- op: set_recorded_date\n  node: n_draft\n  end: finish\n  date: \"2026-10-02\"\n",
            "2026-10-06"
        )
        .shortfalls
        .is_empty(),
        "an actual the chain allows causes none"
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
