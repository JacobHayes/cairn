//! Work moved out of order (D4, D8, F6; ARCHITECTURE, Date network > Execution layer): a
//! node is started, completed or answered whatever it waits on and whatever its own date,
//! with no guard and no bypass. Recorded dates never warn: a finished node answers to
//! nothing, so it holds no prerequisite short and misses no deadline of its own, while work
//! still to do keeps every deadline, those naming a finished milestone among them. Reopening
//! is the one signal: finished work that relied on what was reopened is stale while it is
//! open. Hand-built journeys are created on 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::Records;
use cairn_schema::{Bypass, Date, DisplayState, GuardFailure, StaleReason, State};
use support::{add_nodes as add, key};

fn date(text: &str) -> Date {
    text.parse().unwrap()
}

fn short_on(records: &Records, today: &str, node: &str) -> Option<u32> {
    support::derived_on(records, support::JOURNEY, date(today))
        .dates()
        .shortfall_days(&key(node))
}

fn short(records: &Records, node: &str) -> Option<u32> {
    short_on(records, "2026-10-06", node)
}

fn stale_on(records: &Records, today: &str, node: &str) -> BTreeSet<StaleReason> {
    let graph = support::journey_graph(records, support::JOURNEY);
    support::derived_on(records, support::JOURNEY, date(today)).stale(&graph, &key(node))
}

fn pin(node: &str, on: &str) -> String {
    format!("- op: set_pin\n  node: {node}\n  date: \"{on}\"\n")
}

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn answer(node: &str) -> String {
    format!("- op: answer\n  decision: {node}\n  value: {{text: Soon.}}\n")
}

/// Applies mutations to the test journey on `today`, panicking on a rejection.
fn accepted_at(records: &Records, today: &str, mutations: &str) -> Records {
    let patch = support::patch_to(
        records,
        &format!("{{journey: {}}}", support::JOURNEY),
        mutations,
    );
    let mut inputs = support::fixed_inputs();
    inputs.today = date(today);
    inputs.at = format!("{today}T12:00:00Z").parse().unwrap();
    match support::apply(records, &patch, &inputs) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{rejection:#?}\n{mutations}"),
    }
}

/// Applies each `(day, mutations)` in turn.
fn days(records: &Records, steps: &[(&str, String)]) -> Records {
    steps
        .iter()
        .fold(records.clone(), |records, (today, mutations)| {
            accepted_at(&records, today, mutations)
        })
}

/// A shape of waiting: its name, its nodes, the setup written first, and the moves that take
/// a node ahead of what it waits on, in order.
type Shape = (&'static str, Vec<&'static str>, String, Vec<String>);

fn work(node: &str) -> Vec<String> {
    vec![transition(node, "start"), transition(node, "complete")]
}

fn shapes() -> Vec<Shape> {
    [waiting_shapes(), other_shapes()].concat()
}

/// Waiting on a dependency, an inherited requirement or a stage opening.
fn waiting_shapes() -> Vec<Shape> {
    vec![
        (
            "explicit dependency",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_x, id: x, kind: action, title: X, requires: [n_r]}",
            ],
            String::new(),
            work("n_x"),
        ),
        (
            "inherited requirement",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_p, id: p, kind: group, title: P, requires: [n_r]}",
                "{key: n_x, id: x, parent: n_p, kind: action, title: X}",
            ],
            String::new(),
            work("n_x"),
        ),
        (
            "stage requiring a milestone",
            vec![
                "{key: n_o, id: o, kind: milestone, title: O}",
                "{key: n_s, id: s, kind: group, title: S, requires: [n_o]}",
                "{key: n_x, id: x, parent: n_s, kind: action, title: X}",
            ],
            pin("n_o", "2026-10-20"),
            work("n_x"),
        ),
    ]
}

/// A container ahead of its children, a decision, a milestone, and an own date, for work and
/// for a decision.
fn other_shapes() -> Vec<Shape> {
    vec![
        (
            "container ahead of its children",
            vec![
                "{key: n_x, id: x, kind: deliverable, title: X}",
                "{key: n_c, id: c, parent: n_x, kind: action, title: C, estimate: 3}",
            ],
            String::new(),
            work("n_x"),
        ),
        (
            "decision answered ahead",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_x, id: x, kind: decision, title: X, prompt: Which?, answer_type: text, requires: [n_r]}",
            ],
            String::new(),
            vec![answer("n_x")],
        ),
        (
            "milestone done ahead",
            vec![
                "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
                "{key: n_x, id: x, kind: milestone, title: X, requires: [n_r]}",
            ],
            String::new(),
            vec![transition("n_x", "complete")],
        ),
        (
            "before its own date",
            vec![
                "{key: n_k, id: k, kind: milestone, title: K}",
                "{key: n_x, id: x, kind: action, title: X, not_before: {after: n_k, offset: 2}}",
            ],
            pin("n_k", "2026-10-10"),
            work("n_x"),
        ),
        (
            "decision before its own date",
            vec![
                "{key: n_k, id: k, kind: milestone, title: K}",
                "{key: n_x, id: x, kind: decision, title: X, prompt: Which?, answer_type: text, not_before: {after: n_k}}",
            ],
            pin("n_k", "2026-10-10"),
            vec![answer("n_x")],
        ),
    ]
}

/// D4, F6: in every shape of waiting, each move ahead is a plain transition that stores no
/// override; started, the node is active; finished, nothing in the journey falls short (on
/// main a finish ahead of a prerequisite with an estimate capped it, and it read short).
#[test]
fn moving_ahead_is_a_plain_transition_and_nothing_falls_short() {
    for (name, nodes, setup, moves) in shapes() {
        let records = support::journey(&format!("{}{setup}", add(&nodes)));
        let mut done = records.clone();
        for step in &moves {
            done = support::accepted(&done, step);
            assert!(
                support::graph(&done).state.overrides.is_empty(),
                "{name}: no override"
            );
            let state = support::graph(&done).state.nodes[&key("n_x")].state;
            if state == State::Active {
                let graph = support::journey_graph(&done, support::JOURNEY);
                let shown = support::derived(&done, support::JOURNEY)
                    .node_derived(&graph, &key("n_x"))
                    .display_state;
                assert_eq!(shown, DisplayState::Active, "{name}");
            }
        }
        assert!(
            support::graph(&done).state.nodes[&key("n_x")]
                .state
                .is_terminal(),
            "{name}: finished"
        );
        for node in support::graph(&done).nodes.as_map().keys() {
            assert_eq!(short(&done, node.as_str()), None, "{name}: {node}");
        }
    }
}

/// F6: work still to do keeps its deadline when what it waits on is late: the prerequisite
/// finished work went ahead of holds nothing short, and a remaining node due before it can be
/// done reads short.
#[test]
fn remaining_work_still_falls_short_of_its_deadline() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_r, id: r, kind: action, title: R, estimate: 3}",
            "{key: n_x, id: x, kind: action, title: X, requires: [n_r]}",
            "{key: n_due, id: due, kind: milestone, title: Due}",
        ])),
        &format!(
            "{}{}",
            pin("n_due", "2026-10-08"),
            transition("n_x", "complete")
        ),
    );
    assert_eq!(short(&records, "n_r"), None);
    let remaining = support::accepted(
        &records,
        &add(&[
            "{key: n_y, id: y, kind: action, title: Y, estimate: 1, requires: [n_r], due_by: {before: n_due}}",
        ]),
    );
    assert_eq!(
        short(&remaining, "n_y"),
        Some(2),
        "R finishes on the 9th at the earliest, Y on the 10th, due by the 8th"
    );
    assert_eq!(short(&remaining, "n_x"), None);
}

/// F6: a node finished after its own deadline is a fact, not a warning; open, it reads short.
#[test]
fn a_finished_nodes_own_missed_deadline_is_not_short() {
    let records = support::journey(&add(&[
        "{key: n_x, id: x, kind: action, title: X, estimate: 1, due_by: {after: journey.created_at, offset: 1}}",
    ]));
    assert_eq!(short_on(&records, "2026-10-09", "n_x"), Some(3));
    let late = accepted_at(&records, "2026-10-09", &transition("n_x", "complete"));
    assert_eq!(
        support::graph(&late).state.nodes[&key("n_x")].finished_on,
        Some(date("2026-10-09"))
    );
    assert_eq!(short_on(&late, "2026-10-09", "n_x"), None);
}

/// F6: a deadline of remaining work that names a milestone keeps binding once the milestone
/// is done, now from the day it happened.
#[test]
fn a_remaining_deadline_naming_a_done_milestone_still_binds() {
    let records = support::accepted(
        &support::journey(&add(&[
            "{key: n_m, id: m, kind: milestone, title: M}",
            "{key: n_y, id: y, kind: action, title: Y, estimate: 3, due_by: {before: n_m}}",
        ])),
        &pin("n_m", "2026-10-08"),
    );
    assert_eq!(short(&records, "n_y"), Some(1));
    let happened = support::accepted(&records, &transition("n_m", "complete"));
    assert_eq!(short(&happened, "n_y"), Some(3), "due by the 6th now");
    assert_eq!(short(&happened, "n_m"), None);
}

/// D4: finished work that relied on a reopened dependency is stale while it is open, started
/// again or not, and clear once it is finished again; through a group, or a milestone that
/// marked itself done, that node is named.
#[test]
fn a_reopen_makes_what_relied_on_it_stale() {
    let records = support::journey(&add(&[
        "{key: n_g, id: g, kind: group, title: G}",
        "{key: n_a, id: a, parent: n_g, kind: action, title: A}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_a]}",
        "{key: n_c, id: c, kind: action, title: C, requires: [n_g]}",
        "{key: n_m, id: m, kind: milestone, title: M, auto_done: true, requires: [n_a]}",
        "{key: n_d, id: d, kind: action, title: D, requires: [n_m]}",
    ]));
    let finished = days(
        &records,
        &[
            ("2026-10-06", pin("n_m", "2026-10-07")),
            ("2026-10-07", transition("n_a", "complete")),
            (
                "2026-10-08",
                format!(
                    "{}{}{}",
                    transition("n_b", "complete"),
                    transition("n_c", "complete"),
                    transition("n_d", "complete")
                ),
            ),
        ],
    );
    assert!(
        support::derived_on(&finished, support::JOURNEY, date("2026-10-08"))
            .blocking()
            .done_by_date(&key("n_m"))
    );
    assert!(stale_on(&finished, "2026-10-08", "n_b").is_empty());
    let reopened = accepted_at(&finished, "2026-10-09", &transition("n_a", "reopen"));
    // The record names the revision the reopen produced and the earlier one of the finish it
    // undid.
    let record = support::graph(&reopened).state.nodes[&key("n_a")]
        .reopened
        .unwrap();
    let revision = reopened.journeys.values().next().unwrap().revision;
    assert_eq!(record.revision, revision);
    assert!(record.finished_revision < revision);
    assert_eq!(
        stale_on(&reopened, "2026-10-09", "n_b"),
        BTreeSet::from([StaleReason::Reopened(key("n_a"))])
    );
    assert_eq!(
        stale_on(&reopened, "2026-10-09", "n_c"),
        BTreeSet::from([StaleReason::Reopened(key("n_g"))])
    );
    assert_eq!(
        stale_on(&reopened, "2026-10-09", "n_d"),
        BTreeSet::from([StaleReason::Reopened(key("n_m"))])
    );
    let started = accepted_at(&reopened, "2026-10-09", &transition("n_a", "start"));
    assert!(
        !stale_on(&started, "2026-10-09", "n_b").is_empty(),
        "still open"
    );
    let again = accepted_at(&started, "2026-10-10", &transition("n_a", "complete"));
    assert_eq!(
        support::graph(&again).state.nodes[&key("n_a")].reopened,
        None
    );
    for node in ["n_b", "n_c", "n_d"] {
        assert!(stale_on(&again, "2026-10-10", node).is_empty(), "{node}");
    }
}

/// D4: on one day the order of the writes decides: work finished after its dependency was
/// reopened did not rely on it, and work finished before the reopen did, whether the
/// dependency is required or is a child of the work.
#[test]
fn on_one_day_a_reopen_counts_only_against_what_finished_before_it() {
    let records = support::journey(&add(&[
        "{key: n_a, id: a, kind: action, title: A}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_a]}",
        "{key: n_parent, id: parent, kind: action, title: Parent}",
        "{key: n_child, id: child, parent: n_parent, kind: deliverable, title: Child}",
    ]));
    let day = "2026-10-07";
    let after = days(
        &records,
        &[
            (day, transition("n_a", "complete")),
            (day, transition("n_a", "reopen")),
            (day, transition("n_b", "complete")),
            (day, transition("n_child", "complete")),
            (day, transition("n_child", "reopen")),
            (day, transition("n_parent", "complete")),
        ],
    );
    assert!(stale_on(&after, day, "n_b").is_empty());
    assert!(stale_on(&after, day, "n_parent").is_empty());
    let before = days(
        &records,
        &[
            (day, transition("n_a", "complete")),
            (day, transition("n_b", "complete")),
            (day, transition("n_a", "reopen")),
            (day, transition("n_child", "complete")),
            (day, transition("n_parent", "complete")),
            (day, transition("n_child", "reopen")),
        ],
    );
    assert_eq!(
        stale_on(&before, day, "n_b"),
        BTreeSet::from([StaleReason::Reopened(key("n_a"))])
    );
    assert_eq!(
        stale_on(&before, day, "n_parent"),
        BTreeSet::from([StaleReason::Reopened(key("n_child"))])
    );
}

/// D4: a milestone that marks itself done on a later date, marked done by hand and reopened,
/// was relied on by what finished in between.
#[test]
fn a_milestone_marked_done_ahead_of_its_date_and_reopened_makes_what_relied_on_it_stale() {
    let records = support::journey(&add(&[
        "{key: n_m, id: m, kind: milestone, title: M, auto_done: true}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_m]}",
    ]));
    let reopened = days(
        &records,
        &[
            ("2026-10-06", pin("n_m", "2026-10-20")),
            ("2026-10-07", transition("n_m", "complete")),
            ("2026-10-08", transition("n_b", "complete")),
            ("2026-10-09", transition("n_m", "reopen")),
        ],
    );
    assert_eq!(
        stale_on(&reopened, "2026-10-09", "n_b"),
        BTreeSet::from([StaleReason::Reopened(key("n_m"))])
    );
}

/// D4: what was not done when the work finished was not relied on: work finished ahead of
/// a dependency later finished and reopened, work finished while the dependency was already
/// reopened, work finished after a skip that is reopened, and work finished while a group it
/// requires still had other work open are not stale.
#[test]
fn a_reopen_of_what_was_not_relied_on_is_not_stale() {
    let records = support::journey(&add(&[
        "{key: n_a, id: a, kind: action, title: A}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_a]}",
    ]));
    let ahead = days(
        &records,
        &[
            ("2026-10-07", transition("n_b", "complete")),
            ("2026-10-08", transition("n_a", "complete")),
            ("2026-10-09", transition("n_a", "reopen")),
        ],
    );
    let while_open = days(
        &records,
        &[
            ("2026-10-07", transition("n_a", "complete")),
            ("2026-10-08", transition("n_a", "reopen")),
            ("2026-10-09", transition("n_b", "complete")),
        ],
    );
    let after_skip = days(
        &records,
        &[
            (
                "2026-10-07",
                transition("n_a", "{skip: {reason: Not needed.}}"),
            ),
            ("2026-10-08", transition("n_b", "complete")),
            ("2026-10-09", transition("n_a", "reopen")),
        ],
    );
    assert_eq!(
        support::graph(&after_skip).state.nodes[&key("n_a")].reopened,
        None,
        "a skip undid no finish"
    );
    let grouped = support::journey(&add(&[
        "{key: n_g, id: g, kind: group, title: G}",
        "{key: n_a, id: a, parent: n_g, kind: action, title: A}",
        "{key: n_open, id: open, parent: n_g, kind: action, title: Open}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_g]}",
    ]));
    let group_never_done = days(
        &grouped,
        &[
            ("2026-10-07", transition("n_a", "complete")),
            ("2026-10-08", transition("n_b", "complete")),
            ("2026-10-09", transition("n_a", "reopen")),
        ],
    );
    for (name, records) in [
        ("ahead", ahead),
        ("while open", while_open),
        ("after a skip", after_skip),
        ("a group with other work open", group_never_done),
    ] {
        assert!(stale_on(&records, "2026-10-09", "n_b").is_empty(), "{name}");
    }
}

/// D4: a decision reopened is the same signal, and answering it again clears it; reopened
/// again, the answer that was taken back came after the work, so the work is not stale.
#[test]
fn a_reopened_decision_makes_what_relied_on_it_stale_until_answered() {
    let records = support::journey(&add(&[
        "{key: n_d, id: d, kind: decision, title: D, prompt: Which?, answer_type: text}",
        "{key: n_b, id: b, kind: action, title: B, requires: [n_d]}",
    ]));
    let reopened = days(
        &records,
        &[
            ("2026-10-07", answer("n_d")),
            ("2026-10-08", transition("n_b", "complete")),
            ("2026-10-09", transition("n_d", "reopen")),
        ],
    );
    assert_eq!(
        stale_on(&reopened, "2026-10-09", "n_b"),
        BTreeSet::from([StaleReason::Reopened(key("n_d"))])
    );
    let answered = accepted_at(&reopened, "2026-10-10", &answer("n_d"));
    assert!(stale_on(&answered, "2026-10-10", "n_b").is_empty());
    let again = accepted_at(&answered, "2026-10-11", &transition("n_d", "reopen"));
    assert!(stale_on(&again, "2026-10-11", "n_b").is_empty());
}

fn stored_bypass(records: &Records, node: &str) -> Option<Bypass> {
    support::journey_graph(records, support::JOURNEY)
        .document()
        .state
        .overrides
        .get(&key(node))
        .and_then(|overrides| overrides.bypass.clone())
}

/// A completion bypassing a missing artifact, a note, an unticked item or a missing
/// breakdown needs no reason, and records the failure.
#[test]
fn evidence_bypasses_need_no_reason() {
    let records = support::journey(&add(&[
        "{key: n_art, id: art, kind: deliverable, title: Art, requires_artifact: true}",
        "{key: n_note, id: note, kind: action, title: Note, requires_note: true}",
        "{key: n_list, id: list, kind: action, title: List, checklist: [{id: one, title: One}], requires_checklist: true}",
        "{key: n_hold, id: hold, kind: action, title: Hold, placeholder: true}",
    ]));
    for (node, failure) in [
        ("n_art", GuardFailure::MissingArtifact),
        ("n_note", GuardFailure::MissingNote),
        ("n_list", GuardFailure::Unticked("one".parse().unwrap())),
        ("n_hold", GuardFailure::NotBrokenDown),
    ] {
        let done = support::accepted(&records, &transition(node, "{complete_anyway: {}}"));
        let stored = stored_bypass(&done, node).unwrap_or_else(|| panic!("{node}"));
        assert!(stored.reason.is_none(), "{node}");
        assert!(stored.failures.contains(&failure), "{node}");
    }
}
