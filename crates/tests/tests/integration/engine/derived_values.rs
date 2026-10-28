//! The values the web app reads instead of recomputing (D3, D4, D8, C2, B11, F6): days
//! overdue, a container's progress, the guards a completing transition would fail, started
//! early, what a node waits on, the decisions an undecided node waits on, and the
//! completion suggestion. At the fixed clock (2026-10-06).
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{DerivedJourney, Records};
use cairn_schema::{
    DependencyVia, DisplayState, Guard, GuardFailure, Journey, NodeDerived, NodeKey, Progress,
    Rejection, ViolationCode,
};
use support::{add_nodes as add, key};

/// The schema's derived node for `name` in the test journey.
fn derived_of(records: &Records, name: &str) -> NodeDerived {
    let graph = support::journey_graph(records, support::JOURNEY);
    let derived = support::derived(records, support::JOURNEY);
    derived.node_derived(&graph, &key(name))
}

fn guards(records: &Records, name: &str) -> BTreeSet<Guard> {
    derived_of(records, name).failing_guards
}

fn transition(node: &str, step: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {step}\n")
}

const FIRST: &str = "{key: n_first, id: first, kind: action, title: First}";
const SECOND: &str =
    "{key: n_second, id: second, kind: action, title: Second, requires: [n_first]}";
const FLAG: &str =
    "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";

/// F6, D3: days overdue is today minus the due date for an overdue node, and none when the
/// due date is today or later, or the node is finished; the flag agrees with the number.
#[test]
fn days_overdue_measure_the_overdue_flag() {
    let work = "{key: n_work, id: work, kind: action, title: Work, estimate: 1, due_by: {before: n_cutoff}}";
    let cutoff = "{key: n_cutoff, id: cutoff, kind: milestone, title: Cutoff}";
    let pin = |on: &str| format!("- op: set_pin\n  node: n_cutoff\n  date: \"{on}\"\n");
    let records = support::journey(&add(&[cutoff, work]));
    for (on, days) in [("2026-10-01", Some(5)), ("2026-10-05", Some(1))] {
        let late = support::accepted(&records, &pin(on));
        let found = derived_of(&late, "n_work");
        assert_eq!(found.dates.overdue_days, days, "due {on}");
        assert!(found.overdue);
    }
    let on_time = support::accepted(&records, &pin("2026-10-06"));
    let found = derived_of(&on_time, "n_work");
    assert_eq!(found.dates.overdue_days, None);
    assert!(!found.overdue);
    let late = support::accepted(&records, &pin("2026-10-01"));
    let done = support::accepted(&late, &transition("n_work", "complete"));
    let found = derived_of(&done, "n_work");
    assert_eq!(
        found.dates.overdue_days, None,
        "finished work is not overdue"
    );
    assert!(!found.overdue);
}

/// A stage with work at every depth: a deliverable with two actions (one done, one late), a
/// group of two decisions (one snoozed), a skipped deliverable with a kept action and a loose
/// one, an empty group, work pending on an unanswerable decision, and an milestone done by its date.
fn stage_records() -> Records {
    let records = support::journey(&add(&[
        "{key: n_cutoff, id: cutoff, kind: milestone, title: Cutoff}",
        FLAG,
        "{key: n_d2, id: d2, kind: decision, title: D2, prompt: D2?, answer_type: boolean, relevant_when: {equals: {decision: n_flag, value: true}}}",
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_deliv, id: deliv, parent: n_stage, kind: deliverable, title: Deliv}",
        "{key: n_a1, id: a1, parent: n_deliv, kind: action, title: A1}",
        "{key: n_a2, id: a2, parent: n_deliv, kind: action, title: A2, estimate: 1, due_by: {before: n_cutoff}}",
        "{key: n_inner, id: inner, parent: n_stage, kind: group, title: Inner}",
        "{key: n_ask, id: ask, parent: n_inner, kind: decision, title: Ask, prompt: Go?, answer_type: boolean}",
        "{key: n_ask_later, id: ask_later, parent: n_inner, kind: decision, title: Later, prompt: Later?, answer_type: boolean}",
        "{key: n_skipped, id: skipped, parent: n_stage, kind: deliverable, title: Skipped}",
        "{key: n_kept, id: kept, parent: n_skipped, kind: action, title: Kept}",
        "{key: n_loose, id: loose, parent: n_skipped, kind: action, title: Loose}",
        "{key: n_empty, id: empty, parent: n_stage, kind: group, title: Empty}",
        "{key: n_pending, id: pending, parent: n_stage, kind: action, title: Pending, relevant_when: {equals: {decision: n_d2, value: true}}}",
        "{key: n_goal, id: goal, parent: n_stage, kind: milestone, title: Goal, auto_done: true}",
    ]));
    support::accepted(
        &records,
        &format!(
            "- op: set_pin\n  node: n_cutoff\n  date: \"2026-10-05\"\n\
- op: set_pin\n  node: n_goal\n  date: \"2026-10-06\"\n\
- op: snooze\n  node: n_ask_later\n  until: {{date: \"2026-10-20\"}}\n\
- op: apply_override\n  node: n_kept\n  override: {{keep: {{reason: Still needed.}}}}\n{}{}",
            transition("n_a1", "complete"),
            transition("n_skipped", "{skip: {reason: Not needed.}}"),
        ),
    )
}

/// C2, C4: a stage's progress counts the in-scope, unskipped, non-group work beneath it at
/// every depth, decisions needed include a snoozed decision, and empty groups, skipped work,
/// work that is not relevant, and work pending on an unanswerable decision count for nothing;
/// a kept node under a skipped container counts on its own.
#[test]
fn progress_counts_the_work_beneath_a_container() {
    let records = stage_records();
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let schema = derived.to_schema(&graph);
    let progress = |name: &str| schema.nodes[&key(name)].progress;
    // Beneath the stage: deliverable, its two actions, both decisions, the kept action and
    // the milestone done by its date; the loose action is skipped through its container.
    assert_eq!(
        progress("n_stage"),
        Some(Progress {
            done: 2,
            total: 7,
            overdue: 1,
            decisions_needed: 2
        })
    );
    assert_eq!(
        progress("n_deliv"),
        Some(Progress {
            done: 1,
            total: 2,
            overdue: 1,
            decisions_needed: 0
        })
    );
    assert_eq!(progress("n_empty"), None, "no children, no progress");
    assert_eq!(progress("n_a1"), None);
    assert_eq!(
        schema.nodes[&key("n_pending")].display_state,
        DisplayState::Conditional
    );
}

/// C2: the level's roll-up of a container carries the same progress as the container's own
/// derived values.
#[test]
fn a_roll_up_carries_the_nodes_progress() {
    let records = stage_records();
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let schema = derived.to_schema(&graph);
    let level = DerivedJourney::new(&graph, &derived)
        .level(
            &cairn_schema::LevelQuery::of_kinds(
                cairn_schema::NodeKind::ALL.into_iter().collect(),
                None,
            ),
            &records.deployment,
            &BTreeSet::new(),
        )
        .unwrap();
    let stage = level
        .nodes
        .iter()
        .find(|node| node.key == key("n_stage"))
        .unwrap();
    assert_eq!(
        stage.roll_up.as_ref().map(|roll| roll.progress),
        schema.nodes[&key("n_stage")].progress
    );
}

/// D4: the evidence guards a completing transition would fail now, one guard at a time; what
/// a node waits on is never one.
#[test]
fn failing_guards_name_each_guard_a_completion_would_fail() {
    let records = support::journey(&add(&[
        FIRST,
        SECOND,
        "{key: n_art, id: art, kind: deliverable, title: Art, requires_artifact: true}",
        "{key: n_note, id: note, kind: action, title: Note, requires_note: true}",
        "{key: n_holder, id: holder, kind: deliverable, title: Holder, placeholder: true}",
        "{key: n_stage, id: stage, kind: group, title: Stage, requires: [n_first]}",
        "{key: n_inside, id: inside, parent: n_stage, kind: action, title: Inside}",
    ]));
    let set = |guards: &[Guard]| guards.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(guards(&records, "n_first"), set(&[]));
    assert_eq!(guards(&records, "n_second"), set(&[]), "waiting");
    assert_eq!(guards(&records, "n_inside"), set(&[]), "waiting inherited");
    assert_eq!(
        guards(&records, "n_stage"),
        set(&[]),
        "a group is never completed"
    );
    assert_eq!(guards(&records, "n_art"), set(&[Guard::HasArtifact]));
    assert_eq!(guards(&records, "n_note"), set(&[Guard::HasNote]));
    assert_eq!(guards(&records, "n_holder"), set(&[Guard::BrokenDown]));
    let bypassed = support::accepted(&records, &transition("n_note", "{complete_anyway: {}}"));
    assert_eq!(guards(&bypassed, "n_note"), set(&[]), "finished");
    let skipped = support::accepted(
        &records,
        &transition("n_second", "{skip: {reason: Not needed.}}"),
    );
    assert_eq!(guards(&skipped, "n_second"), set(&[]), "skipped");
    let atomic = support::accepted(
        &records,
        "- op: set_atomic\n  node: n_holder\n  atomic: true\n",
    );
    assert_eq!(guards(&atomic, "n_holder"), set(&[]), "atomic");
}

/// D4: an undecided node fails no guard (finishing it is accepted with a warning), and a
/// decision and a milestone, which carry no evidence, fail none while they wait.
#[test]
fn failing_guards_follow_the_transition_each_kind_completes_with() {
    let records = support::journey(&add(&[
        FLAG,
        "{key: n_maybe, id: maybe, kind: action, title: Maybe, relevant_when: {equals: {decision: n_flag, value: true}}}",
        FIRST,
        "{key: n_ask, id: ask, kind: decision, title: Ask, prompt: Ask?, answer_type: boolean, requires: [n_first]}",
        "{key: n_end, id: end, kind: milestone, title: End, requires: [n_first]}",
    ]));
    assert_eq!(guards(&records, "n_maybe"), BTreeSet::new());
    assert_eq!(
        derived_of(&records, "n_maybe").display_state,
        DisplayState::Conditional
    );
    assert_eq!(guards(&records, "n_ask"), BTreeSet::new());
    assert_eq!(guards(&records, "n_end"), BTreeSet::new());
    let answered = support::accepted(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: false}\n",
    );
    assert_eq!(
        guards(&answered, "n_flag"),
        BTreeSet::new(),
        "a decided decision"
    );
    assert_eq!(
        guards(&answered, "n_maybe"),
        BTreeSet::new(),
        "not relevant"
    );
}

/// The move that completes `key` from its stored state, none when it has none to attempt.
fn completing_step(journey: &Journey, key: &NodeKey) -> Option<&'static str> {
    let step = match journey.graph.nodes.as_map()[key].kind() {
        cairn_schema::NodeKind::Deliverable
        | cairn_schema::NodeKind::Action
        | cairn_schema::NodeKind::Milestone => "complete",
        cairn_schema::NodeKind::Decision | cairn_schema::NodeKind::Group => return None,
    };
    let completes = matches!(
        journey
            .graph
            .state
            .nodes
            .get(key)
            .map(|stored| stored.state),
        Some(
            cairn_schema::State::Todo | cairn_schema::State::Active | cairn_schema::State::Pending
        )
    );
    completes.then_some(step)
}

/// D4: for every node of every fixture journey, at every step of its scenario, that has a
/// completing transition and is in scope, attempting it with no bypass is accepted whenever
/// `failing_guards` is empty, and when refused every violation is a guard failure whose
/// bypassable guards are exactly `failing_guards`.
#[test]
fn failing_guards_are_what_applying_the_transition_refuses() {
    let (mut accepted, mut refused) = (0, 0);
    for name in support::fixture_names() {
        let steps = support::scenario(&name).steps.as_slice().len();
        for step in 0..=steps {
            let records = support::after(&name, step);
            for (id, journey) in &records.journeys {
                let graph = support::journey_graph(&records, id.as_str());
                let derived = support::derived(&records, id.as_str());
                for key in journey.graph.nodes.as_map().keys() {
                    let found = derived.node_derived(&graph, key);
                    let Some(step) = completing_step(journey, key) else {
                        continue;
                    };
                    if found.relevance.value == cairn_schema::Relevance::NotRelevant {
                        continue;
                    }
                    let patch = support::patch_to(
                        &records,
                        &format!("{{journey: {id}}}"),
                        &transition(key.as_str(), step),
                    );
                    match support::apply(&records, &patch, &support::fixed_inputs()) {
                        Ok(_) => {
                            assert!(found.failing_guards.is_empty(), "{name} {key}");
                            accepted += 1;
                        }
                        Err(Rejection::Invalid { violations }) => {
                            if violations
                                .as_slice()
                                .iter()
                                .any(|violation| violation.code != ViolationCode::GuardFailed)
                            {
                                continue;
                            }
                            refused += 1;
                            let refused_guards: BTreeSet<Guard> = violations
                                .as_slice()
                                .iter()
                                .flat_map(|violation| {
                                    violation.failures.iter().map(GuardFailure::guard)
                                })
                                .collect();
                            assert_eq!(found.failing_guards, refused_guards, "{name} {key}");
                        }
                        Err(other) => panic!("{other:#?}"),
                    }
                }
            }
        }
    }
    assert!(accepted > 0 && refused > 0, "the oracle saw both outcomes");
}

/// Gating, D8: what a node waits on lists its own and inherited unsatisfied gates once each,
/// nearest first, and leaves its children out; a force-included node keeps an ancestor's
/// requirement and drops its condition.
#[test]
fn waits_on_lists_each_held_gate_once() {
    let records = support::journey(&add(&[
        FLAG,
        FIRST,
        "{key: n_req, id: req, kind: action, title: Req}",
        "{key: n_group, id: group, kind: group, title: Group, requires: [n_req], relevant_when: {equals: {decision: n_flag, value: true}}}",
        "{key: n_forced, id: forced, parent: n_group, kind: action, title: Forced, requires: [n_first, n_req]}",
        "{key: n_plain, id: plain, parent: n_group, kind: action, title: Plain}",
    ]));
    let forced = support::accepted(
        &records,
        "- op: apply_override\n  node: n_forced\n  override: {force_include: {reason: In scope anyway.}}\n",
    );
    let waits = |records: &Records, name: &str| -> Vec<(String, bool)> {
        derived_of(records, name)
            .waits_on
            .entries
            .as_slice()
            .iter()
            .map(|blocker| {
                (
                    blocker.node.to_string(),
                    matches!(blocker.via, DependencyVia::Explicit),
                )
            })
            .collect()
    };
    // Own first (the explicit requirements), once each though n_req is also inherited.
    assert_eq!(
        waits(&forced, "n_forced"),
        [("n_first".to_owned(), true), ("n_req".to_owned(), true)]
    );
    let plain = waits(&records, "n_plain");
    assert!(plain.iter().any(|(node, _)| node == "n_req"));
    assert!(
        plain.iter().any(|(node, _)| node == "n_flag"),
        "its ancestor's condition"
    );
    assert!(
        !waits(&forced, "n_forced")
            .iter()
            .any(|(node, _)| node == "n_flag"),
        "a force include drops the condition gate"
    );
    let children = support::journey(&add(&[
        "{key: n_deliv, id: deliv, kind: deliverable, title: Deliv}",
        "{key: n_child, id: child, parent: n_deliv, kind: action, title: Child}",
    ]));
    assert_eq!(
        waits(&children, "n_deliv"),
        [],
        "its children are not listed"
    );
    let node = derived_of(&forced, "n_forced").waits_on;
    assert_eq!(
        usize::try_from(node.total).unwrap(),
        node.entries.as_slice().len()
    );
}

/// Gating, D8: a dependency several holders hold is listed once, under the nearest holder,
/// even when another of the node's own gates sorts between them; and the node's own condition
/// gate is nearer than the same decision required by an ancestor.
#[test]
fn waits_on_keeps_the_nearest_holder_of_a_shared_gate() {
    let records = support::journey(&add(&[
        FLAG,
        "{key: n_req, id: req, kind: action, title: Req}",
        "{key: n_zlast, id: zlast, kind: action, title: Zlast}",
        "{key: n_group, id: group, kind: group, title: Group, requires: [n_req, n_flag]}",
        "{key: n_child, id: child, parent: n_group, kind: action, title: Child, requires: [n_req, n_zlast], relevant_when: {equals: {decision: n_flag, value: true}}}",
    ]));
    let waits: Vec<(String, bool)> = derived_of(&records, "n_child")
        .waits_on
        .entries
        .as_slice()
        .iter()
        .map(|blocker| {
            (
                blocker.node.to_string(),
                matches!(blocker.via, DependencyVia::Explicit),
            )
        })
        .collect();
    let nodes: Vec<&str> = waits.iter().map(|(node, _)| node.as_str()).collect();
    assert_eq!(
        nodes.iter().filter(|node| **node == "n_req").count(),
        1,
        "{waits:?}"
    );
    assert_eq!(
        nodes.iter().filter(|node| **node == "n_flag").count(),
        1,
        "{waits:?}"
    );
    let flag = waits.iter().find(|(node, _)| node == "n_flag").unwrap();
    assert!(
        !flag.1,
        "the child's own condition, not the group's requirement"
    );
}

/// D4, Gating: an undecided node names the open decisions it waits on, its own and an
/// ancestor's; any other node names none.
#[test]
fn unanswered_names_the_decisions_an_undecided_node_waits_on() {
    let records = support::journey(&add(&[
        FLAG,
        "{key: n_other, id: other, kind: decision, title: Other, prompt: Other?, answer_type: boolean}",
        "{key: n_group, id: group, kind: group, title: Group, relevant_when: {equals: {decision: n_flag, value: true}}}",
        "{key: n_inside, id: inside, parent: n_group, kind: action, title: Inside, relevant_when: {equals: {decision: n_other, value: true}}}",
    ]));
    let waits = |records: &Records, name: &str| derived_of(records, name).relevance.unanswered;
    assert_eq!(
        waits(&records, "n_inside"),
        BTreeSet::from([key("n_flag"), key("n_other")])
    );
    assert_eq!(waits(&records, "n_group"), BTreeSet::from([key("n_flag")]));
    assert_eq!(waits(&records, "n_flag"), BTreeSet::new());
    let answered = support::accepted(
        &records,
        "- op: answer\n  decision: n_flag\n  value: {boolean: true}\n",
    );
    assert_eq!(
        waits(&answered, "n_inside"),
        BTreeSet::from([key("n_other")])
    );
    assert_eq!(waits(&answered, "n_group"), BTreeSet::new());
}

/// B11: completion is suggested when the journey has a node and none in scope is left open,
/// or its final milestone is done; never for an empty journey. The status summary and
/// the snapshot agree.
#[test]
fn completion_is_suggested_when_nothing_is_left_or_the_end_is_done() {
    let suggested = |records: &Records| {
        let graph = support::journey_graph(records, support::JOURNEY);
        let derived = support::derived(records, support::JOURNEY);
        let journey = DerivedJourney::new(&graph, &derived);
        let summary = journey.status_summary().completion_suggested;
        let snapshot = journey
            .snapshot(
                &cairn_schema::SnapshotScope::default(),
                &support::journey_header(records, support::JOURNEY),
            )
            .unwrap()
            .completion_suggested;
        assert_eq!(summary, snapshot);
        assert_eq!(summary, journey.completion_suggested());
        summary
    };
    assert!(
        !suggested(&support::accepted(
            &Records::default(),
            "- op: create_journey\n  name: Test\n"
        )),
        "an empty journey"
    );
    let records = support::journey(&add(&[
        FIRST,
        "{key: n_end, id: end, kind: milestone, title: End, final: true, auto_done: true}",
        "{key: n_open, id: open, kind: action, title: Open}",
    ]));
    assert!(!suggested(&records), "work remains and the end is pending");
    let finished = support::accepted(
        &records,
        "- op: set_pin\n  node: n_end\n  date: \"2026-10-06\"\n",
    );
    assert!(suggested(&finished), "the final milestone reads as done");
    let all_done = support::accepted(
        &records,
        &format!(
            "{}{}{}",
            transition("n_first", "complete"),
            transition("n_open", "complete"),
            transition("n_end", "complete")
        ),
    );
    assert!(suggested(&all_done));
    let nothing_left = support::journey(&add(&[FIRST]));
    let nothing_left = support::accepted(&nothing_left, &transition("n_first", "complete"));
    assert!(suggested(&nothing_left), "nothing in scope is open");
}
