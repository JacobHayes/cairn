//! The checks that need derived state, on the graph a patch produces (D4, A17): relevance on
//! the transitions that need scope (start, complete or a first answer), never order; the
//! evidence bypass that records the failures it accepted; and where a snooze may sit (B6),
//! with its wait cycles rejected.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::Records;
use cairn_schema::{GuardFailure, Rejection, Relevance, State, Violation, ViolationCode};
use support::{add_nodes as add, key};

fn complete(node: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: complete\n")
}

fn bypass(node: &str, guards: &str) -> String {
    format!(
        "- op: apply_override\n  node: {node}\n  override: {{guard_bypass: {{guards: [{guards}], reason: Done elsewhere.}}}}\n"
    )
}

fn snooze(node: &str, until: &str) -> String {
    format!("- op: snooze\n  node: {node}\n  until: {{node: {until}}}\n")
}

/// The rejection's violations, or a panic.
fn violations(records: &Records, mutations: &str) -> Vec<Violation> {
    match support::journey_patch(records, mutations) {
        Err(Rejection::Invalid { violations }) => violations.as_slice().to_vec(),
        other => panic!("expected a rejection: {other:#?}"),
    }
}

const FIRST: &str = "{key: n_first, id: first, kind: action, title: First}";
const SECOND: &str =
    "{key: n_second, id: second, kind: action, title: Second, requires: [n_first]}";
const THIRD: &str =
    "{key: n_third, id: third, kind: milestone, title: Third, requires: [n_second]}";

fn chain() -> Records {
    support::journey(&add(&[FIRST, SECOND, THIRD]))
}

/// A17, D4: order is never a guard: a chain completes in one patch in any order, and its
/// later nodes complete or start, and a decision is answered, with what they wait on open.
#[test]
fn order_is_never_a_guard() {
    let complete_third = "- op: transition\n  node: n_third\n  transition: complete\n";
    let orders = [
        format!(
            "{}{}{complete_third}",
            complete("n_first"),
            complete("n_second")
        ),
        format!(
            "{complete_third}{}{}",
            complete("n_second"),
            complete("n_first")
        ),
        format!("{}{complete_third}", complete("n_second")),
        "- op: transition\n  node: n_second\n  transition: start\n".to_owned(),
    ];
    for order in orders {
        assert!(support::journey_patch(&chain(), &order).is_ok(), "{order}");
    }
    let gated = "{key: n_gated, id: gated, kind: decision, title: Gated, prompt: Go?, answer_type: boolean, requires: [n_first]}";
    let records = support::journey(&add(&[FIRST, gated]));
    let answered = support::accepted(
        &records,
        "- op: answer\n  decision: n_gated\n  value: {boolean: true}\n",
    );
    assert_eq!(
        support::graph(&answered).state.nodes[&key("n_gated")].state,
        State::Decided
    );
}

const FLAG: &str =
    "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";
const BRANCH: &str = "{key: n_branch, id: branch, kind: action, title: Branch, relevant_when: {equals: {decision: n_flag, value: true}}}";

/// D4: a completing transition is refused on a node the graph the patch produces leaves not
/// relevant, naming the answer in the same patch that did it; force include is the escape
/// hatch.
#[test]
fn relevance_guards_completion() {
    let records = support::journey(&add(&[FLAG, BRANCH]));
    let answered_away = format!(
        "{}- op: answer\n  decision: n_flag\n  value: {{boolean: false}}\n",
        complete("n_branch")
    );
    let [found] = &violations(&records, &answered_away)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.code, ViolationCode::NotRelevant);
    assert_eq!(found.bypassable, None, "relevance has no bypass");
    assert_eq!(found.caused_by, BTreeSet::from([1]), "the answer");
    assert_eq!(found.at.mutation, Some(0), "the completion");
    let forced = format!(
        "- op: apply_override\n  node: n_branch\n  override: {{force_include: {{reason: Needed anyway.}}}}\n{answered_away}"
    );
    let forced = support::accepted(&records, &forced);
    assert_eq!(
        support::graph(&forced).state.nodes[&key("n_branch")].state,
        State::Done
    );
}

/// Gating, D4: completing, or answering a node whose relevance waits on an
/// unanswered decision is accepted, its condition gate holding nothing; the node stays
/// undecided, gains no override, and is not stale.
#[test]
fn undecided_work_finishes_and_stays_undecided() {
    let cases = [
        (BRANCH, complete("n_branch")),
        (
            "{key: n_branch, id: branch, kind: decision, title: Branch, prompt: Go?, answer_type: boolean, relevant_when: {equals: {decision: n_flag, value: true}}}",
            "- op: answer\n  decision: n_branch\n  value: {boolean: true}\n".to_owned(),
        ),
        (
            "{key: n_branch, id: branch, kind: milestone, title: Branch, relevant_when: {equals: {decision: n_flag, value: true}}}",
            "- op: transition\n  node: n_branch\n  transition: complete\n".to_owned(),
        ),
    ];
    for (node, finish) in cases {
        let records = support::journey(&add(&[FLAG, node]));
        let finished = support::accepted(&records, &finish);
        let derived = support::derived(&finished, support::JOURNEY);
        let graph = support::journey_graph(&finished, support::JOURNEY);
        assert_eq!(
            derived.relevance().value(&key("n_branch")),
            Relevance::Undecided,
            "{finish}"
        );
        assert_eq!(
            derived.unanswered(&graph, &key("n_branch")),
            BTreeSet::from([key("n_flag")])
        );
        assert!(!derived.is_stale(&key("n_branch")), "{finish}");
        assert!(
            !support::graph(&finished)
                .state
                .overrides
                .contains_key(&key("n_branch")),
            "not forced into scope: {finish}"
        );
    }
}

/// D4: a bypass covers the evidence a deliverable's or action's completion lacks, so a
/// bypass with no completion in its patch, or on a decision's answer, is refused; the
/// completion it covers is not stale for what it recorded, and reopening clears it.
#[test]
fn a_bypass_covers_a_completion_and_reopening_clears_it() {
    let noted = "{key: n_noted, id: noted, kind: action, title: Noted, requires_note: true}";
    let gated = "{key: n_gated, id: gated, kind: decision, title: Gated, prompt: Go?, answer_type: boolean}";
    let records = support::journey(&add(&[noted, gated]));
    let start = "- op: transition\n  node: n_noted\n  transition: start\n";
    let answer = "- op: answer\n  decision: n_gated\n  value: {boolean: true}\n";
    for alone in [
        format!("{start}{}", bypass("n_noted", "has_note")),
        format!("{answer}{}", bypass("n_gated", "has_note")),
    ] {
        let [found] = &violations(&records, &alone)[..] else {
            panic!("one violation: {alone}")
        };
        assert_eq!(found.code, ViolationCode::IllegalTransition, "{alone}");
    }
    let bypassed = support::accepted(
        &records,
        &format!("{}{}", complete("n_noted"), bypass("n_noted", "has_note")),
    );
    assert!(!support::derived(&bypassed, support::JOURNEY).is_stale(&key("n_noted")));
    let reopened = support::accepted(
        &bypassed,
        "- op: transition\n  node: n_noted\n  transition: reopen\n",
    );
    let overrides = support::graph(&reopened)
        .state
        .overrides
        .get(&key("n_noted"));
    assert!(overrides.is_none_or(|overrides| overrides.bypass.is_none()));
    let [found] = &violations(&reopened, &complete("n_noted"))[..] else {
        panic!("one violation")
    };
    assert_eq!(found.failures, BTreeSet::from([GuardFailure::MissingNote]));
}

/// B6, Gating: a blocked node may be snoozed; a node out of scope or effectively skipped may
/// not, nor a group with no open work left beneath it.
#[test]
fn a_snooze_sits_on_a_node_in_scope() {
    let records = support::journey(&add(&[FIRST, SECOND, FLAG, BRANCH]));
    assert!(support::journey_patch(&records, &snooze("n_second", "n_flag")).is_ok());
    let away = support::accepted(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    assert_eq!(
        support::codes(support::journey_patch(
            &away,
            &snooze("n_branch", "n_first")
        )),
        [ViolationCode::SnoozeNotActionable]
    );
    let parent = support::journey(&add(&[
        "{key: n_box, id: box, kind: deliverable, title: Box}",
        "{key: n_inside, id: inside, parent: n_box, kind: action, title: Inside}",
        FIRST,
    ]));
    let skipped = support::accepted(
        &parent,
        "- op: transition\n  node: n_box\n  transition: {skip: {reason: Not needed.}}\n",
    );
    assert_eq!(
        support::codes(support::journey_patch(
            &skipped,
            &snooze("n_inside", "n_first")
        )),
        [ViolationCode::SnoozeNotActionable]
    );
    let group = support::accepted(
        &support::journey(&add(&[
            "{key: n_pack, id: pack, kind: group, title: Pack}",
            "{key: n_item, id: item, parent: n_pack, kind: action, title: Item}",
            FIRST,
        ])),
        &complete("n_item"),
    );
    assert_eq!(
        support::codes(support::journey_patch(&group, &snooze("n_pack", "n_first"))),
        [ViolationCode::SnoozeNotActionable]
    );
}

/// B6: a node snooze whose target transitively depends on the snoozed node is rejected: a
/// dependent, an ancestor, or another snooze waiting the other way.
#[test]
fn snooze_wait_cycles_are_rejected() {
    let records = support::journey(&add(&[
        FIRST,
        SECOND,
        "{key: n_box, id: box, kind: deliverable, title: Box}",
        "{key: n_inside, id: inside, parent: n_box, kind: action, title: Inside}",
    ]));
    let cases = [
        snooze("n_first", "n_second"),
        snooze("n_inside", "n_box"),
        format!(
            "{}{}",
            snooze("n_first", "n_inside"),
            snooze("n_inside", "n_first")
        ),
    ];
    for case in cases {
        let codes = support::codes(support::journey_patch(&records, &case));
        assert!(
            !codes.is_empty() && codes.iter().all(|code| *code == ViolationCode::SnoozeCycle),
            "{case}: {codes:?}"
        );
    }
    // B6: a container's snooze holds over its subtree, so its own child is no target.
    let inside = support::codes(support::journey_patch(
        &records,
        &snooze("n_box", "n_inside"),
    ));
    assert_eq!(inside, [ViolationCode::SnoozeCycle]);
}

/// Review round 1: an `auto_done` milestone that reads as done has nothing left to do, so
/// it is not snoozed (B6, F1).
#[test]
fn a_milestone_that_reads_as_done_is_not_snoozed() {
    let records = support::accepted(
        &support::journey(&add(&[
            FIRST,
            "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_done: true}",
        ])),
        "- op: set_pin\n  node: n_gate\n  date: \"2026-10-05\"\n",
    );
    assert_eq!(
        support::codes(support::journey_patch(
            &records,
            &snooze("n_gate", "n_first")
        )),
        [ViolationCode::SnoozeNotActionable]
    );
}
