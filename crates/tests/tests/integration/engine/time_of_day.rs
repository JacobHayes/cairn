//! Time of day (F5, F1, B6, D8, F6): a pin, a date answer, and a snooze may carry a time, and
//! a node's own time acts at its minute. Dates elsewhere stay days: a time on another day
//! behaves as that day, and what is derived from a time (due dates, slack) is in days. The
//! deployment is in UTC here, so a time is written with `+00:00`.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Derived, Records};
use cairn_schema::{DisplayState, ViolationCode, When};
use support::{add_nodes as add, key};

const GATE: &str = "{key: n_gate, id: gate, kind: milestone, title: Gate}";
const AUTO_GATE: &str = "{key: n_gate, id: gate, kind: milestone, title: Gate, auto_done: true}";
const AFTER: &str = "{key: n_after, id: after, kind: action, title: After, requires: [n_gate]}";
const WORK: &str = "{key: n_work, id: work, kind: action, title: Work}";

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

fn snooze(node: &str, until: &str) -> String {
    format!("- op: snooze\n  node: {node}\n  until: {{date: \"{until}\"}}\n")
}

fn derived_at(records: &Records, now: &str) -> Derived {
    support::derived_at(records, support::JOURNEY, now)
}

fn shown(records: &Records, now: &str, node: &str) -> DisplayState {
    let graph = support::journey_graph(records, support::JOURNEY);
    derived_at(records, now).display_state(&graph, &key(node))
}

fn when(text: &str) -> When {
    text.parse().unwrap()
}

/// A milestone pinned to a time is scheduled until that minute, then ready; what waits on it
/// stays blocked until it is done.
#[test]
fn a_pinned_milestone_turns_ready_at_its_minute() {
    let records = support::accepted(
        &support::journey(&add(&[GATE, AFTER])),
        &pin("n_gate", "2026-10-10T08:15+00:00"),
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:14:59Z", "n_gate"),
        DisplayState::Scheduled
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:15:00Z", "n_gate"),
        DisplayState::Ready
    );
    for now in ["2026-10-10T08:14:00Z", "2026-10-10T08:15:00Z"] {
        assert_eq!(shown(&records, now, "n_after"), DisplayState::Blocked);
    }
    let held = derived_at(&records, "2026-10-10T08:14:00Z");
    let scheduled = held.blocking().scheduled(&key("n_gate")).unwrap();
    assert_eq!(scheduled.date, when("2026-10-10T08:15+00:00"));
}

/// A milestone that marks itself done does so at its minute, and releases what waits on it.
#[test]
fn an_automatic_milestone_is_done_at_its_minute_and_releases_its_dependents() {
    let records = support::accepted(
        &support::journey(&add(&[AUTO_GATE, AFTER])),
        &pin("n_gate", "2026-10-10T08:15+00:00"),
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:14:00Z", "n_gate"),
        DisplayState::Scheduled
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:14:00Z", "n_after"),
        DisplayState::Blocked
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:15:00Z", "n_gate"),
        DisplayState::Done
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:15:00Z", "n_after"),
        DisplayState::Ready
    );
}

/// Work with its own time is late from the minute after it, by no days; from the next day the
/// count of days runs as it does for a day.
#[test]
fn work_is_late_from_the_minute_after_its_own_time() {
    let records = support::accepted(
        &support::journey(&add(&[WORK])),
        &pin("n_work", "2026-10-10T08:15+00:00"),
    );
    let overdue = |now: &str| {
        derived_at(&records, now)
            .dates()
            .overdue_days(&key("n_work"))
    };
    assert_eq!(overdue("2026-10-10T08:15:00Z"), None);
    assert_eq!(overdue("2026-10-10T08:15:59Z"), None);
    assert_eq!(overdue("2026-10-10T08:16:00Z"), Some(0));
    assert_eq!(overdue("2026-10-11T00:00:00Z"), Some(1));
    assert!(
        derived_at(&records, "2026-10-10T08:16:00Z")
            .dates()
            .overdue(&key("n_work"))
    );
}

/// A snooze until a time holds to that minute.
#[test]
fn a_snooze_lifts_at_its_time() {
    let records = support::accepted(
        &support::journey(&add(&[WORK])),
        &snooze("n_work", "2026-10-10T14:00+00:00"),
    );
    let held = |now: &str| {
        derived_at(&records, now)
            .blocking()
            .snoozed(&key("n_work"))
            .is_some()
    };
    assert!(held("2026-10-10T13:59:00Z"));
    assert!(!held("2026-10-10T14:00:00Z"));
    assert_eq!(
        shown(&records, "2026-10-10T13:59:00Z", "n_work"),
        DisplayState::Snoozed
    );
}

/// A node held by its own snooze and a container's lifts when the later does; a day lifts
/// before any time on it.
#[test]
fn a_held_node_lifts_with_the_later_snooze() {
    let lifts = |records: &Records| {
        let graph = support::journey_graph(records, support::JOURNEY);
        derived_at(records, "2026-10-09T12:00:00Z").snooze_lifts(&graph, &key("n_item"))
    };
    let base = support::journey(&add(&[
        "{key: n_box, id: box, kind: group, title: Box}",
        "{key: n_item, id: item, parent: n_box, kind: action, title: Item}",
    ]));
    let both = |own: &str, container: &str| {
        let records = support::accepted(&base, &snooze("n_item", own));
        support::accepted(&records, &snooze("n_box", container))
    };
    let (early, late) = ("2026-10-10T14:00+00:00", "2026-10-10T16:00+00:00");
    assert_eq!(lifts(&both(early, late)), Some(when(late)));
    assert_eq!(lifts(&both(late, early)), Some(when(late)));
    assert_eq!(
        lifts(&both("2026-10-10", "2026-10-10T00:30+00:00")),
        Some(when("2026-10-10T00:30+00:00"))
    );
}

/// An answer to a date decision that feeds a milestone gives the milestone its time.
#[test]
fn a_date_answer_carries_its_time_to_the_milestone_it_feeds() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_fed, id: fed, kind: milestone, title: Fed}",
            "{key: n_when, id: when, kind: decision, title: When, prompt: When?, answer_type: date, feeds_milestone: n_fed}",
        ])),
        "- op: answer\n  decision: n_when\n  value: {date: \"2026-10-10T08:15+00:00\"}\n",
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:14:00Z", "n_fed"),
        DisplayState::Scheduled
    );
    assert_eq!(
        shown(&records, "2026-10-10T08:15:00Z", "n_fed"),
        DisplayState::Ready
    );
}

/// A condition on a date answer compares the day, so a timed answer equals its day.
#[test]
fn a_condition_on_a_date_reads_the_day_of_a_timed_answer() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_when, id: when, kind: decision, title: When, prompt: When?, answer_type: date}",
            "{key: n_prep, id: prep, kind: action, title: Prep, relevant_when: {equals: {decision: n_when, value: \"2026-10-10\"}}}",
        ])),
        "- op: answer\n  decision: n_when\n  value: {date: \"2026-10-10T08:15+00:00\"}\n",
    );
    let derived = derived_at(&records, "2026-10-10T07:00:00Z");
    assert_eq!(
        derived.relevance().value(&key("n_prep")),
        cairn_schema::Relevance::Relevant
    );
}

/// A time on another day acts as that day at every minute today.
#[test]
fn a_time_on_tomorrow_acts_as_tomorrow() {
    let timed = support::accepted(
        &support::journey(&add(&[GATE])),
        &pin("n_gate", "2026-10-11T00:30+00:00"),
    );
    let day = support::accepted(
        &support::journey(&add(&[GATE])),
        &pin("n_gate", "2026-10-11"),
    );
    for now in [
        "2026-10-10T00:00:00Z",
        "2026-10-10T12:00:00Z",
        "2026-10-10T23:59:00Z",
    ] {
        assert_eq!(shown(&timed, now, "n_gate"), shown(&day, now, "n_gate"));
        assert_eq!(derived_at(&timed, now).holds_until(), None);
    }
}

/// A derivation holds until the next time today that would change it: a time arriving, and a
/// pin's lateness a minute after.
#[test]
fn a_derivation_holds_until_the_next_time_today() {
    let timed = support::accepted(
        &support::journey(&add(&[GATE, WORK])),
        &(pin("n_gate", "2026-10-10T08:15+00:00") + &pin("n_work", "2026-10-10T09:00+00:00")),
    );
    let until = |now: &str| {
        derived_at(&timed, now)
            .holds_until()
            .map(|at| at.to_string())
    };
    assert_eq!(
        until("2026-10-10T07:00:00Z").as_deref(),
        Some("2026-10-10T08:15:00Z")
    );
    assert_eq!(
        until("2026-10-10T08:15:00Z").as_deref(),
        Some("2026-10-10T08:16:00Z")
    );
    assert_eq!(
        until("2026-10-10T08:16:00Z").as_deref(),
        Some("2026-10-10T09:00:00Z")
    );
    assert_eq!(
        until("2026-10-10T09:00:00Z").as_deref(),
        Some("2026-10-10T09:01:00Z")
    );
    assert_eq!(until("2026-10-10T09:01:00Z"), None);
    assert_eq!(until("2026-10-09T09:01:00Z"), None, "no time today yet");

    let days = support::accepted(
        &support::journey(&add(&[GATE])),
        &pin("n_gate", "2026-10-10"),
    );
    assert_eq!(
        derived_at(&days, "2026-10-10T07:00:00Z").holds_until(),
        None
    );
}

/// A pin with a time is set again, not shifted by days.
#[test]
fn a_pin_with_a_time_is_not_shifted() {
    let records = support::accepted(
        &support::journey(&add(&[GATE])),
        &pin("n_gate", "2026-10-10T08:15+00:00"),
    );
    let shift = "- op: shift_pin\n  node: n_gate\n  offset_days: 3\n";
    assert_eq!(
        support::codes(support::journey_patch(&records, shift)),
        [ViolationCode::InvalidTime]
    );
    let day = support::accepted(
        &support::journey(&add(&[GATE])),
        &pin("n_gate", "2026-10-10"),
    );
    assert!(support::journey_patch(&day, shift).is_ok());
}

/// A shortfall that a timed pin holds still offers to unpin it, though not to shift it by days.
#[test]
fn a_shortfall_held_by_a_timed_pin_offers_the_unpin_and_no_shift() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_meeting, id: meeting, kind: milestone, title: Meeting}",
            "{key: n_work, id: work, kind: action, title: Work, estimate: 3, due_by: {before: n_meeting}}",
        ])),
        &pin("n_meeting", "2026-10-05T08:15+00:00"),
    );
    let derived = support::derived(&records, support::JOURNEY);
    let graph = support::journey_graph(&records, support::JOURNEY);
    let shortfall = derived
        .dates()
        .node(&graph, &key("n_work"))
        .shortfall
        .unwrap();
    assert!(
        shortfall
            .resolutions
            .iter()
            .any(|found| matches!(found, cairn_schema::Mutation::ClearPin { .. }))
    );
    assert!(
        !shortfall
            .resolutions
            .iter()
            .any(|found| matches!(found, cairn_schema::Mutation::ShiftPin { .. }))
    );
}
