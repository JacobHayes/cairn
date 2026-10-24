//! Scheduled (D8, D5, D4, A8, F1): what a date ahead holds off the acting frontier. A milestone
//! that marks itself done waits for its effective date; any other milestone for its pinned
//! date (a pin, or a fed date answer), never for a derived due; work and decisions for their
//! own not-before date. A date holds a node off the acting frontier, never stops it: work
//! started before its date stays active, and finished before it is accepted plainly; what
//! waits on a scheduled node is blocked and names its date; and a journey held only by dates
//! is stalled and names each. Hand-built journeys are created on 2026-10-06 and derived at that day.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Derived, Records};
use cairn_schema::{Date, DisplayState, NodeKey, StallCause};
use support::{add_nodes as add, key};

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn derived(records: &Records) -> Derived {
    support::derived(records, support::JOURNEY)
}

fn shown(derived: &Derived, records: &Records, node: &str) -> DisplayState {
    let graph = support::journey_graph(records, support::JOURNEY);
    derived.display_state(&graph, &key(node))
}

fn acting(derived: &Derived) -> BTreeSet<&str> {
    derived
        .blocking()
        .acting_frontier()
        .iter()
        .map(NodeKey::as_str)
        .collect()
}

/// D8, F1: each kind of date that holds a node: the effective date of a milestone that marks
/// itself done, a milestone's pin, a milestone's fed date answer, and the own not-before date
/// of an action, a deliverable, and a decision. Each is on the frontier, off the acting
/// frontier, and scheduled with that date; a derived due is a deadline, not the event's date,
/// so the milestone it alone dates stays ready.
#[test]
fn a_date_ahead_holds_each_kind_off_the_acting_frontier() {
    let records = [
        pin("n_auto", "2026-10-09"),
        pin("n_pinned", "2026-10-10"),
        pin("n_final", "2026-10-30"),
        "- op: answer\n  decision: n_when\n  value: {date: \"2026-10-12\"}\n".to_owned(),
    ]
    .iter()
    .fold(
        support::journey(&add(&[
            "{key: n_auto, id: auto, kind: milestone, title: Auto, auto_done: true}",
            "{key: n_pinned, id: pinned, kind: milestone, title: Pinned}",
            "{key: n_fed, id: fed, kind: milestone, title: Fed}",
            "{key: n_when, id: when, kind: decision, title: When, prompt: When?, answer_type: date, feeds_milestone: n_fed}",
            "{key: n_final, id: final, kind: milestone, title: Final}",
            "{key: n_dated, id: dated, kind: milestone, title: Dated, due_by: {before: n_final, offset: 3}}",
            "{key: n_action, id: action, kind: action, title: Action, not_before: {after: n_pinned, offset: 1}}",
            "{key: n_report, id: report, kind: deliverable, title: Report, not_before: {after: journey.created_at, offset: 3}}",
            "{key: n_choose, id: choose, kind: decision, title: Choose, prompt: Which?, answer_type: boolean, not_before: {before: n_fed, offset: 1}}",
        ])),
        |records, patch| support::accepted(&records, patch),
    );
    let derived = derived(&records);
    let cases = [
        ("n_auto", "2026-10-09", true),
        ("n_pinned", "2026-10-10", false),
        ("n_fed", "2026-10-12", false),
        ("n_final", "2026-10-30", false),
        ("n_action", "2026-10-11", false),
        ("n_report", "2026-10-09", false),
        ("n_choose", "2026-10-11", false),
    ];
    let blocking = derived.blocking();
    for (node, on, marks_itself_done) in cases {
        let held = blocking.scheduled(&key(node)).expect(node);
        assert_eq!(
            (held.date, held.marks_itself_done),
            (date(on), marks_itself_done),
            "{node}"
        );
        assert!(blocking.frontier().contains(&key(node)), "{node}");
        assert_eq!(
            shown(&derived, &records, node),
            DisplayState::Scheduled,
            "{node}"
        );
    }
    assert_eq!(
        derived.dates().due(&key("n_dated")),
        Some(date("2026-10-27")),
        "a deadline from the final milestone"
    );
    assert_eq!(acting(&derived), BTreeSet::from(["n_dated"]));
    assert_eq!(shown(&derived, &records, "n_dated"), DisplayState::Ready);
    // On its day the date no longer holds.
    let on_the_day = support::derived_on(&records, support::JOURNEY, date("2026-10-11"));
    assert!(acting(&on_the_day).is_superset(&BTreeSet::from(["n_action", "n_choose", "n_report"])));
}

/// D8, A8: work started before its own date stays active, on the acting frontier; a
/// container's date holds only its own work, never its children; what waits
/// on a scheduled node is blocked and its blocker carries the date.
#[test]
fn started_work_children_and_dependents_of_a_dated_node() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_launch, id: launch, kind: milestone, title: Launch}",
            "{key: n_early, id: early, kind: action, title: Early, not_before: {after: n_launch}}",
            "{key: n_kit, id: kit, kind: deliverable, title: Kit, not_before: {after: n_launch}}",
            "{key: n_part, id: part, parent: n_kit, kind: action, title: Part}",
            "{key: n_after, id: after, kind: action, title: After, requires: [n_launch]}",
        ])),
        &format!(
            "{}{}",
            pin("n_launch", "2026-11-01"),
            transition("n_early", "start")
        ),
    );
    let derived = derived(&records);
    let graph = support::journey_graph(&records, support::JOURNEY);
    assert_eq!(shown(&derived, &records, "n_early"), DisplayState::Active);
    assert_eq!(
        acting(&derived),
        BTreeSet::from(["n_early", "n_part"]),
        "the kit's date holds the kit, not its part"
    );
    assert!(
        derived.blocking().scheduled(&key("n_kit")).is_none(),
        "it waits on its part first"
    );
    assert_eq!(shown(&derived, &records, "n_after"), DisplayState::Blocked);
    let waits = derived.node_derived(&graph, &key("n_after")).waits_on;
    let dates: Vec<(&str, Option<Date>)> = waits
        .entries
        .as_slice()
        .iter()
        .map(|blocker| (blocker.node.as_str(), blocker.date))
        .collect();
    assert_eq!(dates, [("n_launch", Some(date("2026-11-01")))]);
}

/// D4, A8: work finished before its own not-before date is accepted plainly, falls short of
/// nothing, and is not stale, before or after the date moves later; a milestone marked done
/// before its pinned date is accepted the same way.
#[test]
fn completing_before_the_own_date_is_accepted() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_opening, id: opening, kind: milestone, title: Opening}",
            "{key: n_setup, id: setup, kind: action, title: Setup, not_before: {after: n_opening}}",
        ])),
        &pin("n_opening", "2026-10-10"),
    );
    let graph = support::journey_graph(&records, support::JOURNEY);
    assert!(
        derived(&records)
            .node_derived(&graph, &key("n_setup"))
            .failing_guards
            .is_empty()
    );
    let done = support::accepted(&records, &transition("n_setup", "complete"));
    let moved = support::accepted(&done, &pin("n_opening", "2026-10-12"));
    for records in [&done, &moved] {
        let derived = derived(records);
        assert!(!derived.is_stale(&key("n_setup")));
        assert_eq!(derived.dates().shortfall_days(&key("n_setup")), None);
        assert_eq!(derived.dates().shortfall_days(&key("n_opening")), None);
    }
    assert!(
        support::journey_patch(&records, &transition("n_opening", "complete")).is_ok(),
        "a milestone done before its pinned date"
    );
}

/// D5: a journey whose only frontier node waits for its own date is stalled, and the
/// diagnostic names that node with its date (the diagnostic asserts it names every node held
/// off the acting frontier).
#[test]
fn a_journey_held_only_by_a_not_before_date_names_it() {
    let records = support::journey(&add(&[
        "{key: n_start, id: start, kind: action, title: Start, not_before: {after: journey.created_at, offset: 3}}",
        "{key: n_next, id: next, kind: action, title: Next, requires: [n_start]}",
    ]));
    let derived = derived(&records);
    let stalled = derived.blocking().stalled().expect("stalled");
    assert_eq!(
        stalled.waiting_on,
        [StallCause::Scheduled {
            node: key("n_start"),
            date: date("2026-10-09"),
            marks_itself_done: false,
        }]
    );
    assert!(!stalled.all_blocked);
}
