//! The per-kind state machines (D1, D1a, B6, F1, F2): every row of every table is allowed and
//! nothing else is, through apply as well as the table itself.
#![cfg(test)]

use crate::support;

use cairn_engine::transition::{self, Move};
use cairn_schema::{AnswerValue, GraphKey, NodeKind, Record, State, Write};
use support::{accepted, graph, journey, journey_patch, key};

use Move::{Answer, Complete, Reach, Reopen, Skip, Start, Stop};
use NodeKind::{Action, Decision, Deliverable, Group, Milestone};
use State::{Active, Decided, Derived, Done, Open, Pending, Reached, Skipped, Todo};

/// PRD D1, row by row.
const TABLE: &[(NodeKind, Move, State, State)] = &[
    (Deliverable, Start, Todo, Active),
    (Deliverable, Stop, Active, Todo),
    (Deliverable, Complete, Todo, Done),
    (Deliverable, Complete, Active, Done),
    (Deliverable, Skip, Todo, Skipped),
    (Deliverable, Skip, Active, Skipped),
    (Deliverable, Reopen, Done, Todo),
    (Deliverable, Reopen, Skipped, Todo),
    (Action, Start, Todo, Active),
    (Action, Stop, Active, Todo),
    (Action, Complete, Todo, Done),
    (Action, Complete, Active, Done),
    (Action, Skip, Todo, Skipped),
    (Action, Skip, Active, Skipped),
    (Action, Reopen, Done, Todo),
    (Action, Reopen, Skipped, Todo),
    (Decision, Answer, Open, Decided),
    (Decision, Answer, Decided, Decided),
    (Decision, Skip, Open, Skipped),
    (Decision, Reopen, Decided, Open),
    (Decision, Reopen, Skipped, Open),
    (Milestone, Reach, Pending, Reached),
    (Milestone, Skip, Pending, Skipped),
    (Milestone, Reopen, Reached, Pending),
    (Milestone, Reopen, Skipped, Pending),
    (Group, Skip, Derived, Skipped),
    (Group, Reopen, Skipped, Derived),
];

fn expected(kind: NodeKind, step: Move, from: State) -> Option<State> {
    TABLE
        .iter()
        .find(|(k, s, f, _)| *k == kind && *s == step && *f == from)
        .map(|row| row.3)
}

#[test]
fn the_tables_have_every_row_and_no_other() {
    for kind in NodeKind::ALL {
        for step in Move::ALL {
            for from in State::ALL {
                let found = transition::next(kind, from, step);
                assert_eq!(
                    found,
                    expected(kind, step, from),
                    "{kind:?} {from:?} {step:?}"
                );
            }
        }
    }
}

fn node_yaml(kind: NodeKind) -> String {
    let extra = match kind {
        Decision => "\n    prompt: Yes?\n    answer_type: boolean",
        Deliverable | Action | Milestone | Group => "",
    };
    format!(
        "- op: add_node\n  node:\n    key: n_x\n    id: x\n    kind: {}\n    title: X{extra}\n",
        kind.name()
    )
}

/// A journey holding one node of `kind`, stored in `state` (a decided decision holds an
/// answer).
fn one_node(kind: NodeKind, state: State) -> cairn_engine::Records {
    let mut records = journey(&node_yaml(kind));
    let journey = records.journeys.values_mut().next().unwrap();
    journey
        .graph
        .state
        .nodes
        .get_mut(&key("n_x"))
        .unwrap()
        .state = state;
    if state == Decided {
        journey
            .graph
            .state
            .answers
            .insert(key("n_x"), AnswerValue::Boolean(true));
    }
    if state == Skipped {
        let stored = journey.graph.state.nodes.get_mut(&key("n_x")).unwrap();
        stored.skip_reason = Some("Skipped earlier.".parse().unwrap());
    }
    records
}

fn mutation(step: Move) -> &'static str {
    match step {
        Start => "- op: transition\n  node: n_x\n  transition: start\n",
        Stop => "- op: transition\n  node: n_x\n  transition: stop\n",
        Complete => "- op: transition\n  node: n_x\n  transition: complete\n",
        Skip => "- op: transition\n  node: n_x\n  transition: {skip: {reason: Not needed here.}}\n",
        Reopen => "- op: transition\n  node: n_x\n  transition: reopen\n",
        Reach => "- op: transition\n  node: n_x\n  transition: reach\n",
        Answer => "- op: answer\n  decision: n_x\n  value: {boolean: false}\n",
    }
}

#[test]
fn apply_allows_exactly_the_rows_of_each_table() {
    for kind in NodeKind::ALL {
        for from in State::ALL.into_iter().filter(|state| state.legal_for(kind)) {
            for step in Move::ALL {
                let records = one_node(kind, from);
                let result = journey_patch(&records, mutation(step));
                let label = format!("{kind:?} {from:?} {step:?}");
                match (expected(kind, step, from), result) {
                    (Some(to), Ok(applied)) => {
                        let stored = &graph(applied.records()).state.nodes[&key("n_x")];
                        assert_eq!(stored.state, to, "{label}");
                    }
                    (None, Err(_)) => {}
                    (Some(_), Err(rejection)) => panic!("{label}: rejected {rejection:#?}"),
                    (None, Ok(_)) => panic!("{label}: accepted a transition the table lacks"),
                }
            }
        }
    }
}

#[test]
fn transitions_record_and_clear_their_dates() {
    let today = support::fixed_inputs().today;
    let started = accepted(&one_node(Deliverable, Todo), mutation(Start));
    assert_eq!(
        graph(&started).state.nodes[&key("n_x")].started_on,
        Some(today)
    );
    let stopped = accepted(&started, mutation(Stop));
    assert_eq!(graph(&stopped).state.nodes[&key("n_x")].started_on, None);
    let done = accepted(&started, mutation(Complete));
    assert_eq!(
        graph(&done).state.nodes[&key("n_x")].finished_on,
        Some(today)
    );
    let reopened = accepted(&done, mutation(Reopen));
    let stored = &graph(&reopened).state.nodes[&key("n_x")];
    assert_eq!((stored.started_on, stored.finished_on), (None, None));
    let reached = accepted(&one_node(Milestone, Pending), mutation(Reach));
    assert_eq!(
        graph(&reached).state.nodes[&key("n_x")].finished_on,
        Some(today)
    );
    let unreached = accepted(&reached, mutation(Reopen));
    assert_eq!(graph(&unreached).state.nodes[&key("n_x")].finished_on, None);
}

#[test]
fn reopening_a_decision_clears_its_answer() {
    let decided = accepted(&one_node(Decision, Open), mutation(Answer));
    assert!(graph(&decided).state.answers.contains_key(&key("n_x")));
    let reopened = accepted(&decided, mutation(Reopen));
    assert!(!graph(&reopened).state.answers.contains_key(&key("n_x")));
    assert_eq!(graph(&reopened).state.nodes[&key("n_x")].state, Open);
}

#[test]
fn a_container_skip_is_one_event_that_leaves_its_descendants_stored_states_alone() {
    let records = journey(
        "- op: add_node\n  node: {key: n_g, id: g, kind: group, title: G}\n\
         - op: add_node\n  node: {key: n_a, id: a, parent: n_g, kind: action, title: A}\n\
         - op: transition\n  node: n_a\n  transition: start\n",
    );
    let skip = "- op: transition\n  node: n_g\n  transition: {skip: {reason: Out of scope.}}\n";
    let applied = journey_patch(&records, skip).unwrap();
    let [event] = applied.events() else {
        panic!("one event for the skip (D1a)")
    };
    assert_eq!(event.delta.len(), 1, "{:#?}", event.delta);
    let after = graph(applied.records());
    assert_eq!(after.state.nodes[&key("n_g")].state, Skipped);
    assert_eq!(after.state.nodes[&key("n_a")].state, Active);
    let reopened = accepted(
        applied.records(),
        "- op: transition\n  node: n_g\n  transition: reopen\n",
    );
    assert_eq!(graph(&reopened).state.nodes[&key("n_g")].state, Derived);
    assert_eq!(graph(&reopened).state.nodes[&key("n_a")].state, Active);
}

#[test]
fn a_transition_on_a_snoozed_node_clears_the_snooze_in_its_own_event() {
    let records = journey(
        "- op: add_node\n  node: {key: n_a, id: a, kind: action, title: A}\n\
         - op: snooze\n  node: n_a\n  until: {date: \"2026-12-01\"}\n",
    );
    assert!(graph(&records).state.snoozes.contains_key(&key("n_a")));
    let applied = journey_patch(
        &records,
        "- op: transition\n  node: n_a\n  transition: start\n",
    )
    .unwrap();
    let [event] = applied.events() else {
        panic!("one event")
    };
    let lifted = event.delta.iter().any(|write| match write {
        Write::Remove(cairn_schema::RecordKey::InGraph { key, .. }) => {
            *key == GraphKey::Snooze(key_a())
        }
        Write::Remove(_) | Write::Put(_) | Write::CopyGraph { .. } => false,
    });
    assert!(lifted, "{:#?}", event.delta);
    assert!(
        event
            .delta
            .iter()
            .any(|write| matches!(write, Write::Put(Record::Graph { .. })))
    );
    assert!(
        !graph(applied.records())
            .state
            .snoozes
            .contains_key(&key("n_a"))
    );
}

fn key_a() -> cairn_schema::NodeKey {
    key("n_a")
}
