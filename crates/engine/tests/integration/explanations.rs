//! Priority, Gravity and Leverage explained (C2, C8): a container's subtree gravity, and the
//! direct dependents completing a node would not yet free with what else each waits on, over
//! the fixtures and small constructed journeys.
#![cfg(test)]

use crate::support;

use cairn_engine::{Derived, DerivedJourney, Records};
use cairn_schema::{
    Blocker, Cursor, DependencyVia, Deployment, ExplainedField, HeldDependent, LevelQuery, Score,
};
use support::{add_nodes as add, key};

/// A fixture's journey after its first `steps` steps, and its derive at the fixed clock.
fn fixture(name: &str, steps: usize) -> (Records, String) {
    let journey = support::scenario(name).journey.to_string();
    (support::after(name, steps), journey)
}

fn gravity(derived: &Derived, node: &str) -> f64 {
    derived.priority().gravity(&key(node)).value()
}

fn area(derived: &Derived, node: &str) -> Option<f64> {
    derived
        .priority()
        .subtree_gravity(&key(node))
        .map(Score::value)
}

/// Priority: a container's subtree gravity is the gravity of its whole area, the container and
/// its open descendants and everything downstream of any of them, each once. Two children
/// share a dependent, so it counts once (the sum of the children's gravities would count it
/// twice), the number is at least every member's own gravity, and it drops as the children
/// finish, to nothing when they all have.
#[test]
fn a_container_shows_the_gravity_of_its_whole_area() {
    let records = support::journey(&add(&[
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_a, id: a, parent: n_stage, kind: action, title: A}",
        "{key: n_b, id: b, parent: n_stage, kind: action, title: B}",
        "{key: n_shared, id: shared, kind: action, title: Shared, weight: 5, requires: [n_a, n_b]}",
    ]));
    let derived = support::derived(&records, support::JOURNEY);
    assert_eq!(gravity(&derived, "n_a"), 6.0);
    assert_eq!(gravity(&derived, "n_b"), 6.0);
    assert_eq!(area(&derived, "n_stage"), Some(7.0));
    assert_eq!(area(&derived, "n_a"), None, "a node with no children");

    let one_done = support::accepted(
        &records,
        "- op: transition\n  node: n_a\n  transition: complete\n",
    );
    let derived = support::derived(&one_done, support::JOURNEY);
    assert_eq!(
        area(&derived, "n_stage"),
        Some(6.0),
        "B and what waits on it"
    );

    let all_done = support::accepted(
        &one_done,
        "- op: transition\n  node: n_b\n  transition: complete\n",
    );
    let derived = support::derived(&all_done, support::JOURNEY);
    assert_eq!(area(&derived, "n_stage"), Some(0.0));
    assert_eq!(gravity(&derived, "n_stage"), 0.0, "the stage is done too");
}

/// C2: the level's roll-up carries the container's subtree gravity.
#[test]
fn the_level_roll_up_carries_the_subtree_gravity() {
    let (records, journey) = fixture("hiring-loop", 1);
    let graph = support::journey_graph(&records, &journey);
    let derived = support::derived(&records, &journey);
    let level = DerivedJourney::new(&graph, &derived)
        .level(
            &LevelQuery::of_kinds(
                cairn_schema::NodeKind::ALL.into_iter().collect(),
                Some(key("n_onsite")),
            ),
            &Deployment::default(),
        )
        .unwrap_or_else(|error| panic!("{error:?}"));
    let debrief = level
        .nodes
        .iter()
        .find(|node| node.key == key("n_debrief"))
        .unwrap();
    assert_eq!(
        debrief.roll_up.as_ref().unwrap().subtree_gravity,
        derived.priority().subtree_gravity(&key("n_debrief"))
    );
}

/// What a still-waiting entry reads as: the dependent, then what else it waits on as
/// (node, how).
fn held(entries: &[HeldDependent]) -> Vec<(&str, Vec<(&str, String)>)> {
    entries
        .iter()
        .map(|entry| {
            let also = entry
                .also_waits_on
                .iter()
                .map(|blocker| (blocker.node.as_str(), via(&blocker.via)))
                .collect();
            (entry.node.as_str(), also)
        })
        .collect()
}

fn via(how: &DependencyVia) -> String {
    match how {
        DependencyVia::Explicit => "explicit".to_owned(),
        DependencyVia::Containment => "containment".to_owned(),
        DependencyVia::Inherited { ancestor } => format!("inherited {ancestor}"),
        DependencyVia::Condition { condition_on } => format!("condition {condition_on}"),
        DependencyVia::StageOpening { group } => format!("opening {group}"),
    }
}

const FLAG: &str =
    "{key: n_flag, id: flag, kind: decision, title: Flag, prompt: Flag?, answer_type: boolean}";

/// A gate milestone, the one dependent finishing it frees (weight 10), one it does not because
/// its relevance waits on an unanswered decision, and one it does not because another action
/// is open.
fn held_journey() -> Records {
    support::journey(&add(&[
        FLAG,
        "{key: n_gate, id: gate, kind: milestone, title: Gate}",
        "{key: n_freed, id: freed, kind: action, title: Freed, weight: 10, requires: [n_gate]}",
        "{key: n_cond, id: cond, kind: action, title: Cond, weight: 3, requires: [n_gate], relevant_when: {equals: {decision: n_flag, value: true}}}",
        "{key: n_other, id: other, kind: action, title: Other}",
        "{key: n_both, id: both, kind: action, title: Both, weight: 2, requires: [n_gate, n_other]}",
    ]))
}

/// C8, Priority: a node with leverage from one dependent lists that unblock, and lists the
/// dependents it would not free with what else each waits on, here a condition and another
/// action, largest weight first.
#[test]
fn a_node_lists_what_it_unblocks_and_what_is_still_waiting_with_its_condition() {
    let records = held_journey();
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let journey = DerivedJourney::new(&graph, &derived);
    assert_eq!(derived.priority().leverage(&key("n_gate")).value(), 10.0);
    let freed = derived.priority().leverage_from(&key("n_gate"));
    assert_eq!(
        freed
            .iter()
            .map(|found| found.node.as_str())
            .collect::<Vec<_>>(),
        ["n_freed"]
    );
    let waiting = journey.still_waiting(&key("n_gate")).unwrap();
    assert_eq!(
        held(&waiting),
        [
            ("n_cond", vec![("n_flag", "condition n_cond".to_owned())]),
            ("n_both", vec![("n_other", "explicit".to_owned())]),
        ]
    );
}

/// Priority: answering the decision frees the held dependent, which then leaves the list and
/// joins the unblocks; a dependent is never listed with nothing it waits on.
#[test]
fn a_dependent_leaves_still_waiting_once_nothing_else_holds_it() {
    let records = support::accepted(
        &held_journey(),
        "- op: answer\n  decision: n_flag\n  value: {boolean: true}\n",
    );
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let waiting = DerivedJourney::new(&graph, &derived)
        .still_waiting(&key("n_gate"))
        .unwrap();
    assert_eq!(
        held(&waiting),
        [("n_both", vec![("n_other", "explicit".to_owned())])]
    );
    let freed: Vec<_> = derived
        .priority()
        .leverage_from(&key("n_gate"))
        .into_iter()
        .map(|found| found.node)
        .collect();
    assert_eq!(freed, [key("n_freed"), key("n_cond")]);
}

/// Priority, Leverage: completing a node cascades through derived group completion, so a
/// dependent that waits on the node and on the group only it holds open is not listed as
/// waiting on that group, and a group the cascade completes is not listed at all.
#[test]
fn a_cascading_group_completion_is_not_still_waiting() {
    let records = support::journey(&add(&[
        "{key: n_box, id: box, kind: group, title: Box}",
        "{key: n_a, id: a, parent: n_box, kind: action, title: A}",
        "{key: n_other, id: other, kind: action, title: Other}",
        "{key: n_held, id: held, kind: action, title: Held, weight: 2, requires: [n_a, n_box, n_other]}",
        "{key: n_after, id: after, kind: group, title: After, requires: [n_a, n_box]}",
    ]));
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let waiting = DerivedJourney::new(&graph, &derived)
        .still_waiting(&key("n_a"))
        .unwrap();
    assert_eq!(
        held(&waiting),
        [("n_held", vec![("n_other", "explicit".to_owned())])]
    );
}

/// C8: what an ancestor's hold reaches a dependent through is listed as inherited, and a
/// stage's opening as the opening, both with the node they wait on.
#[test]
fn inherited_holds_and_stage_openings_are_listed_with_their_source() {
    let (records, journey) = fixture("vendor-evaluation", 1);
    let graph = support::journey_graph(&records, &journey);
    let derived = support::derived(&records, &journey);
    let waiting = DerivedJourney::new(&graph, &derived)
        .still_waiting(&key("n_access"))
        .unwrap();
    assert_eq!(
        held(&waiting),
        [("n_plan", vec![("n_kickoff", "opening n_setup".to_owned())])]
    );
    let waiting = DerivedJourney::new(&graph, &derived)
        .still_waiting(&key("n_plan_draft"))
        .unwrap();
    assert_eq!(
        held(&waiting),
        [(
            "n_plan_review",
            vec![
                ("n_access", "inherited n_plan".to_owned()),
                ("n_kickoff", "opening n_setup".to_owned()),
            ]
        )]
    );
}

/// Priority: finished work, groups, and unknown nodes hold nothing back.
#[test]
fn nothing_is_still_waiting_on_closed_work_a_group_or_an_unknown_node() {
    let records = support::accepted(
        &held_journey(),
        "- op: transition\n  node: n_gate\n  transition: reach\n",
    );
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let journey = DerivedJourney::new(&graph, &derived);
    assert_eq!(journey.still_waiting(&key("n_gate")).unwrap(), []);
    assert!(journey.still_waiting(&key("n_nowhere")).is_err());
    let (records, name) = fixture("vendor-evaluation", 1);
    let graph = support::journey_graph(&records, &name);
    let derived = support::derived(&records, &name);
    assert_eq!(
        DerivedJourney::new(&graph, &derived)
            .still_waiting(&key("n_setup"))
            .unwrap(),
        []
    );
}

/// ARCHITECTURE, Read path: a long still-waiting list pages through the explanations the way
/// gravity's does, largest first, with its total.
#[test]
fn a_long_still_waiting_list_pages() {
    let mut nodes = vec![
        "{key: n_root, id: root, kind: action, title: Root}".to_owned(),
        "{key: n_other, id: other, kind: action, title: Other}".to_owned(),
    ];
    nodes.extend((0..60).map(|at| {
        format!(
            "{{key: n_held{at:02}, id: held{at:02}, kind: action, title: Held, requires: [n_root, n_other]}}"
        )
    }));
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let journey = DerivedJourney::new(&graph, &derived);
    let limit = usize::try_from(cairn_schema::limits::EXPLANATION_ENTRY_COUNT_MAX).unwrap();
    let first = journey
        .explanations(&key("n_root"), ExplainedField::StillWaiting, Cursor::START)
        .unwrap();
    assert_eq!((first.held.len(), first.total), (limit, 60));
    assert_eq!(first.entries, []);
    let next = first.next.unwrap();
    let second = journey
        .explanations(&key("n_root"), ExplainedField::StillWaiting, next)
        .unwrap();
    assert_eq!((second.held.len(), second.next), (60 - limit, None));
    assert_eq!(
        first.held.first().map(|found| found.node.clone()),
        Some(key("n_held00"))
    );
    let blocker = Blocker {
        node: key("n_other"),
        via: DependencyVia::Explicit,
    };
    assert_eq!(
        first.held.first().map(|found| found.also_waits_on.clone()),
        Some(vec![blocker])
    );
}
