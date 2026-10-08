//! The guards that need derived state, on the graph a patch produces (D4, A17): relevance
//! and `deps_done` on completing transitions (complete, a first answer, reach), the bypass
//! that records the failures it accepted, and where a snooze may sit (B6), with its wait
//! cycles rejected.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_engine::Records;
use cairn_schema::{Guard, GuardFailure, Rejection, Relevance, Violation, ViolationCode};
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

/// A17, D4: guards see the graph the whole patch produces, so a chain completes in one patch
/// in any order.
#[test]
fn a_chain_completes_in_one_patch_in_any_order() {
    let reach = "- op: transition\n  node: n_third\n  transition: reach\n";
    let orders = [
        format!("{}{}{reach}", complete("n_first"), complete("n_second")),
        format!("{reach}{}{}", complete("n_second"), complete("n_first")),
    ];
    for order in orders {
        assert!(support::journey_patch(&chain(), &order).is_ok(), "{order}");
    }
    let [found] = &violations(&chain(), &format!("{}{reach}", complete("n_second")))[..] else {
        panic!("one violation per guarded transition")
    };
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert_eq!(found.bypassable, Some(Guard::DepsDone));
    assert_eq!(found.at.mutation, Some(0), "the completion of the second");
    assert_eq!(
        found.failures,
        BTreeSet::from([GuardFailure::OpenDependency(key("n_first"))])
    );
}

/// D4: completing a node and giving it an unfinished dependency in the same patch is
/// rejected, naming both mutations; as two patches it is accepted and the node is stale.
#[test]
fn a_completion_and_a_new_dependency_in_one_patch_are_rejected_together() {
    let records = support::journey(&add(&[FIRST]));
    let insert = format!(
        "{}- op: add_edge\n  edge: {{node: n_first, requires: n_new}}\n",
        add(&["{key: n_new, id: new, kind: action, title: New}"])
    );
    let together = format!("{}{insert}", complete("n_first"));
    let [found] = &violations(&records, &together)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.at.mutation, Some(0));
    assert_eq!(
        found.caused_by,
        BTreeSet::from([1, 2]),
        "the new node and its edge"
    );
    let completed = support::accepted(&records, &complete("n_first"));
    let inserted = support::accepted(&completed, &insert);
    let derived = support::derived(&inserted, support::JOURNEY);
    let graph = support::journey_graph(&inserted, support::JOURNEY);
    assert_eq!(
        derived.stale(&graph, &key("n_first")),
        BTreeSet::from([GuardFailure::OpenDependency(key("n_new"))])
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
    let forced = format!(
        "- op: apply_override\n  node: n_branch\n  override: {{force_include: {{reason: Needed anyway.}}}}\n{answered_away}"
    );
    assert!(support::journey_patch(&records, &forced).is_ok());
}

/// Gating, D4: completing, answering, or reaching a node whose relevance waits on an
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
            "- op: transition\n  node: n_branch\n  transition: reach\n".to_owned(),
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

/// D4: an undecided node's other open dependencies still hold its completion: the violation
/// names them, not the decision its relevance waits on, and a bypass accepts only them.
#[test]
fn undecided_work_still_waits_on_its_other_dependencies() {
    let gated = "{key: n_branch, id: branch, kind: action, title: Branch, requires: [n_first], relevant_when: {equals: {decision: n_flag, value: true}}}";
    let records = support::journey(&add(&[FLAG, FIRST, gated]));
    let [found] = &violations(&records, &complete("n_branch"))[..] else {
        panic!("one violation")
    };
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert_eq!(
        found.failures,
        BTreeSet::from([GuardFailure::OpenDependency(key("n_first"))])
    );
    let bypassed = support::accepted(
        &records,
        &format!(
            "{}{}",
            complete("n_branch"),
            bypass("n_branch", "deps_done")
        ),
    );
    assert!(!support::derived(&bypassed, support::JOURNEY).is_stale(&key("n_branch")));
}

/// Gating 2, D4: a gated decision answered before what it requires is done is rejected; in
/// order, or with a `deps_done` bypass, it is accepted, and the bypass records the open
/// dependency it accepted.
#[test]
fn a_gated_decision_answered_out_of_order() {
    let gated = "{key: n_gated, id: gated, kind: decision, title: Gated, prompt: Go?, answer_type: boolean, requires: [n_first]}";
    let records = support::journey(&add(&[FIRST, gated]));
    let answer = "- op: answer\n  decision: n_gated\n  value: {boolean: true}\n";
    let [found] = &violations(&records, answer)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert!(support::journey_patch(&records, &format!("{answer}{}", complete("n_first"))).is_ok());
    let bypassed = support::accepted(
        &records,
        &format!("{answer}{}", bypass("n_gated", "deps_done")),
    );
    let recorded = &support::graph(&bypassed).state.overrides[&key("n_gated")]
        .bypass
        .as_ref()
        .unwrap()
        .failures;
    assert_eq!(
        recorded,
        &BTreeSet::from([GuardFailure::OpenDependency(key("n_first"))])
    );
    let revised = "- op: answer\n  decision: n_gated\n  value: {boolean: false}\n";
    assert!(
        support::journey_patch(&bypassed, revised).is_ok(),
        "a revision is not guarded"
    );
}

/// D4: a bypass accepts the failures present when it was applied, so the node is not stale
/// for them, while a later, distinct failure makes it stale; reopening clears the bypass.
#[test]
fn a_bypass_accepts_only_the_failures_it_recorded() {
    let records = support::journey(&add(&[FIRST, SECOND]));
    let bypassed = support::accepted(
        &records,
        &format!(
            "{}{}",
            complete("n_second"),
            bypass("n_second", "deps_done")
        ),
    );
    assert!(!support::derived(&bypassed, support::JOURNEY).is_stale(&key("n_second")));
    let later = support::accepted(
        &bypassed,
        &format!(
            "{}- op: add_edge\n  edge: {{node: n_second, requires: n_later}}\n",
            add(&["{key: n_later, id: later, kind: action, title: Later}"])
        ),
    );
    let derived = support::derived(&later, support::JOURNEY);
    let graph = support::journey_graph(&later, support::JOURNEY);
    assert_eq!(
        derived.stale(&graph, &key("n_second")),
        BTreeSet::from([GuardFailure::OpenDependency(key("n_later"))])
    );
    let reopened = support::accepted(
        &later,
        "- op: transition\n  node: n_second\n  transition: reopen\n",
    );
    let overrides = support::graph(&reopened)
        .state
        .overrides
        .get(&key("n_second"));
    assert!(overrides.is_none_or(|overrides| overrides.bypass.is_none()));
}

/// B6, Gating: a blocked node may be snoozed; a node out of scope or effectively skipped may
/// not.
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
    assert!(
        support::journey_patch(&records, &snooze("n_box", "n_inside")).is_ok(),
        "a container may wait for its own child"
    );
}

/// Review round 1: an `auto_reach` milestone that reads as reached has nothing left to do, so
/// it is not snoozed (B6, F1).
#[test]
fn a_milestone_that_reads_as_reached_is_not_snoozed() {
    let records = support::accepted(
        &support::journey(&add(&[
            FIRST,
            "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true}",
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

/// Review round 1: an answer that brings a completed node's dependency back into scope is
/// named with the completion it fails.
#[test]
fn an_answer_that_reopens_a_dependency_is_named() {
    let user = "{key: n_user, id: user, kind: action, title: User, requires: [n_branch]}";
    let records = support::accepted(
        &support::journey(&add(&[FLAG, BRANCH, user])),
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    let patch = format!(
        "{}- op: answer\n  decision: n_flag\n  value: {{boolean: true}}\n",
        complete("n_user")
    );
    let [found] = &violations(&records, &patch)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.code, ViolationCode::GuardFailed);
    assert_eq!(found.caused_by, BTreeSet::from([1]), "the answer");
}

/// Review round 2: a failed completion names the mutation that gave its ancestor an unfinished
/// opening, and the pin edit that put an `auto_reach` dependency's date back ahead.
#[test]
fn inherited_openings_and_moved_pins_are_named() {
    let staged = support::journey(&add(&[
        "{key: n_opens, id: opens, kind: milestone, title: Opens}",
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_inside, id: inside, parent: n_stage, kind: action, title: Inside}",
    ]));
    let opening = format!(
        "{}- op: set_node_field\n  node: n_stage\n  value: {{opens_at: n_opens}}\n",
        complete("n_inside")
    );
    let [found] = &violations(&staged, &opening)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.caused_by, BTreeSet::from([1]), "the opening");
    let gated = support::accepted(
        &support::journey(&add(&[
            "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_reach: true}",
            "{key: n_after, id: after, kind: action, title: After, requires: [n_gate]}",
        ])),
        "- op: set_pin\n  node: n_gate\n  date: \"2026-10-05\"\n",
    );
    let moved = format!(
        "{}- op: set_pin\n  node: n_gate\n  date: \"2026-10-10\"\n",
        complete("n_after")
    );
    let [found] = &violations(&gated, &moved)[..] else {
        panic!("one violation")
    };
    assert_eq!(found.caused_by, BTreeSet::from([1]), "the pin");
}
