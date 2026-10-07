//! Pass 6 (PRD Priority; C6, C8, C10): gravity with the undecided discount on each node's own
//! weight, count-once downstream sets, not-relevant pruning, and terminal traversal; leverage
//! with the owner factor, derived group completion, kept work, and each target once. Hand-built
//! journeys are created on 2026-10-06 and derived at that day.
#![cfg(test)]

use crate::support;

use cairn_engine::{Derived, Records};
use cairn_schema::{Contribution, Score};
use support::{add_nodes as add, key};

fn transition(node: &str, transition: &str) -> String {
    format!("- op: transition\n  node: {node}\n  transition: {transition}\n")
}

fn derived(records: &Records) -> Derived {
    support::derived(records, support::JOURNEY)
}

fn gravity(derived: &Derived, node: &str) -> f64 {
    derived.priority().gravity(&key(node)).value()
}

fn leverage(derived: &Derived, node: &str) -> f64 {
    derived.priority().leverage(&key(node)).value()
}

/// Contributions as (node, value, other owner), in the order listed.
fn listed(contributions: &[Contribution]) -> Vec<(&str, f64, bool)> {
    contributions
        .iter()
        .map(|found| (found.node.as_str(), found.score.value(), found.other_owner))
        .collect()
}

const CHOOSE: &str =
    "{key: n_choose, id: choose, kind: decision, title: Choose, prompt: Go?, answer_type: boolean}";
const MAYBE: &str = "{key: n_maybe, id: maybe, kind: action, title: Maybe, weight: 4, relevant_when: {equals: {decision: n_choose, value: true}}}";
const AFTER: &str =
    "{key: n_after, id: after, kind: action, title: After, weight: 3, requires: [n_maybe]}";

/// Priority, Gravity: an undecided node counts at half, in its own gravity and in what is
/// upstream of it; a node that is relevant whatever the answer counts in full even behind
/// undecided work; and the condition gate makes everything the answer decides downstream of
/// the decision.
#[test]
fn an_undecided_node_counts_at_half_and_what_follows_it_in_full() {
    let records = support::journey(&add(&[CHOOSE, MAYBE, AFTER]));
    let start = derived(&records);
    assert_eq!(gravity(&start, "n_after"), 3.0);
    assert_eq!(
        gravity(&start, "n_maybe"),
        2.0 + 3.0,
        "own weight discounted"
    );
    assert_eq!(gravity(&start, "n_choose"), 1.0 + 2.0 + 3.0);
    assert_eq!(
        listed(&start.priority().gravity_from(&key("n_choose"))),
        [("n_after", 3.0, false), ("n_maybe", 2.0, false)],
        "largest first"
    );
    let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
    inputs.rank.undecided_discount = 250.try_into().unwrap();
    let graph = support::journey_graph(&records, support::JOURNEY);
    let quartered = cairn_engine::derive(&graph, None, &inputs);
    assert_eq!(
        gravity(&quartered, "n_maybe"),
        1.0 + 3.0,
        "the configured discount"
    );
}

/// Priority, Gravity: a not-relevant branch is pruned, so what waits on it is no longer
/// downstream of what it waits on.
#[test]
fn a_not_relevant_branch_carries_no_gravity() {
    let records = support::journey(&add(&[
        CHOOSE,
        "{key: n_first, id: first, kind: action, title: First}",
        "{key: n_maybe, id: maybe, kind: action, title: Maybe, weight: 4, requires: [n_first], relevant_when: {equals: {decision: n_choose, value: true}}}",
        AFTER,
    ]));
    assert_eq!(gravity(&derived(&records), "n_first"), 1.0 + 2.0 + 3.0);
    let no = support::accepted(
        &records,
        "- op: answer\n  decision: n_choose\n  value: {boolean: false}\n",
    );
    let pruned = derived(&no);
    assert_eq!(gravity(&pruned, "n_first"), 1.0);
    assert_eq!(gravity(&pruned, "n_maybe"), 0.0, "not relevant");
    assert_eq!(gravity(&pruned, "n_after"), 3.0);
}

const TOP: &str = "{key: n_top, id: top, kind: action, title: Top}";
const LEFT: &str = "{key: n_left, id: left, kind: action, title: Left, requires: [n_top]}";
const RIGHT: &str = "{key: n_right, id: right, kind: action, title: Right, requires: [n_top]}";
const BOTTOM: &str = "{key: n_bottom, id: bottom, kind: action, title: Bottom, weight: 10, requires: [n_left, n_right]}";

/// Priority, Gravity: a node reached along two paths (a diamond) counts once.
#[test]
fn a_diamond_counts_its_bottom_once() {
    let records = support::journey(&add(&[TOP, LEFT, RIGHT, BOTTOM]));
    let derived = derived(&records);
    assert_eq!(gravity(&derived, "n_top"), 1.0 + 1.0 + 1.0 + 10.0);
    assert_eq!(gravity(&derived, "n_left"), 1.0 + 10.0);
    assert_eq!(derived.priority().gravity_from(&key("n_top")).len(), 3);
}

/// Priority, Gravity: terminal nodes are traversed to reach the unfinished work behind them
/// but count nothing, not even toward their own gravity.
#[test]
fn a_terminal_node_is_traversed_but_not_counted() {
    let chain = [
        "{key: n_one, id: one, kind: action, title: One}",
        "{key: n_two, id: two, kind: action, title: Two, weight: 7, requires: [n_one]}",
        "{key: n_three, id: three, kind: action, title: Three, weight: 5, requires: [n_two]}",
    ];
    let records = support::journey(&add(&chain));
    let done = support::accepted(
        &records,
        &[
            transition("n_one", "complete"),
            transition("n_two", "complete"),
        ]
        .concat(),
    );
    let reopened = support::accepted(&done, &transition("n_one", "reopen"));
    let derived = derived(&reopened);
    assert_eq!(gravity(&derived, "n_one"), 1.0 + 5.0);
    assert_eq!(gravity(&derived, "n_two"), 5.0);
    assert_eq!(
        listed(&derived.priority().gravity_from(&key("n_one"))),
        [("n_three", 5.0, false)]
    );
    assert_eq!(
        derived.priority().downstream(&key("n_one")),
        [&key("n_three"), &key("n_two")],
        "the terminal node is still downstream (C7)"
    );
}

/// Priority, Gravity: a child's gravity holds its parent and everything after the parent, so
/// a container's largest child gravity is at least its own.
#[test]
fn a_child_carries_its_parents_gravity() {
    let records = support::journey(&add(&[
        "{key: n_plan, id: plan, kind: deliverable, title: Plan, weight: 2}",
        "{key: n_draft, id: draft, parent: n_plan, kind: action, title: Draft}",
        "{key: n_review, id: review, parent: n_plan, kind: action, title: Review, weight: 3}",
        "{key: n_uses, id: uses, kind: action, title: Uses, weight: 4, requires: [n_plan]}",
    ]));
    let derived = derived(&records);
    let priority = derived.priority();
    assert_eq!(gravity(&derived, "n_plan"), 2.0 + 4.0);
    assert_eq!(gravity(&derived, "n_draft"), 1.0 + 2.0 + 4.0);
    assert_eq!(gravity(&derived, "n_review"), 3.0 + 2.0 + 4.0);
    assert_eq!(
        priority.max_child_gravity(&key("n_plan")),
        Some(priority.gravity(&key("n_review")))
    );
    assert_eq!(priority.max_child_gravity(&key("n_uses")), None);
}

fn owned(node: &str, requires: &str, weight: u32, owner: Option<&str>) -> String {
    let participations = owner.map_or(String::new(), |owner| {
        format!(", participations: {{k_owner: [{owner}]}}")
    });
    format!(
        "{{key: {node}, id: {}, kind: action, title: T, weight: {weight}, requires: [{requires}]{participations}}}",
        &node[2..]
    )
}

const PEOPLE: &str = "- op: create_entity\n  entity: {key: e_a, name: A}\n- op: create_entity\n  entity: {key: e_b, name: B}\n";

/// Priority, Leverage: an unblocked node owned by someone other than the node's owner counts
/// double, and so does one no one owns when the node is owned; when the node is unowned, an
/// owned target counts double and an unowned one once. The split is listed per target.
#[test]
fn the_owner_factor_favors_other_owners_and_the_unowned_target() {
    let nodes = [
        "{key: n_gate, id: gate, kind: action, title: Gate, participations: {k_owner: [e_a]}}"
            .to_owned(),
        owned("n_mine", "n_gate", 2, Some("e_a")),
        owned("n_theirs", "n_gate", 3, Some("e_b")),
        owned("n_nobodys", "n_gate", 4, None),
        "{key: n_free, id: free, kind: action, title: Free}".to_owned(),
        owned("n_owned", "n_free", 3, Some("e_b")),
        owned("n_loose", "n_free", 2, None),
    ];
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&format!("{PEOPLE}{}", add(&nodes)));
    let derived = derived(&records);
    assert_eq!(leverage(&derived, "n_gate"), 2.0 + 3.0 * 2.0 + 4.0 * 2.0);
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_gate"))),
        [
            ("n_nobodys", 8.0, true),
            ("n_theirs", 6.0, true),
            ("n_mine", 2.0, false)
        ]
    );
    assert_eq!(leverage(&derived, "n_free"), 3.0 * 2.0 + 2.0);
    assert_eq!(leverage(&derived, "n_mine"), 0.0, "unblocks nothing");
}

/// Priority, Leverage: completing the last open child completes its group (derived group
/// completion), which unblocks what waits on the group; each target counts once even when
/// it waits on both; what still waits on other open work is not unblocked.
#[test]
fn leverage_cascades_through_group_completion_once_per_target() {
    let records = support::journey(&add(&[
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_last, id: last, parent: n_stage, kind: action, title: Last}",
        "{key: n_next, id: next, kind: action, title: Next, weight: 2, requires: [n_stage, n_last]}",
        "{key: n_other, id: other, kind: action, title: Other}",
        "{key: n_both, id: both, kind: action, title: Both, weight: 5, requires: [n_stage, n_other]}",
    ]));
    let derived = derived(&records);
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_last"))),
        [("n_next", 2.0, false)]
    );
    assert_eq!(
        leverage(&derived, "n_other"),
        0.0,
        "the stage is still open"
    );
    assert_eq!(
        derived.priority().leverage(&key("n_stage")),
        Score::default(),
        "a group has no leverage of its own"
    );
}

/// Priority, Leverage, F1: an `auto_reach` milestone due today or earlier reads as reached as
/// soon as it is unblocked, so completing what gates it also unblocks what waits on it; one
/// still ahead of its date is unblocked and holds what follows.
#[test]
fn leverage_cascades_through_a_due_auto_reach_milestone() {
    let records = support::journey(&format!(
        "{}- op: set_pin\n  node: n_due\n  date: \"2026-10-01\"\n- op: set_pin\n  node: n_ahead\n  date: \"2026-10-20\"\n",
        add(&[
            "{key: n_gate, id: gate, kind: action, title: Gate}",
            "{key: n_due, id: due, kind: milestone, title: Due, auto_reach: true, requires: [n_gate]}",
            "{key: n_after, id: after, kind: action, title: After, weight: 10, requires: [n_due]}",
            "{key: n_other, id: other, kind: action, title: Other}",
            "{key: n_ahead, id: ahead, kind: milestone, title: Ahead, auto_reach: true, requires: [n_other]}",
            "{key: n_later, id: later, kind: action, title: Later, weight: 10, requires: [n_ahead]}",
        ])
    ));
    let derived = derived(&records);
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_gate"))),
        [("n_after", 10.0, false), ("n_due", 1.0, false)]
    );
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_due"))),
        [("n_after", 10.0, false)],
        "completing the milestone itself"
    );
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_other"))),
        [("n_ahead", 1.0, false)]
    );
}

/// Priority, Leverage, D1a: completing the kept work under a skipped container lets the
/// container satisfy its dependents.
#[test]
fn completing_kept_work_unblocks_the_skipped_containers_dependents() {
    let records = support::journey(&add(&[
        "{key: n_plan, id: plan, kind: deliverable, title: Plan}",
        "{key: n_draft, id: draft, parent: n_plan, kind: action, title: Draft}",
        "{key: n_review, id: review, parent: n_plan, kind: action, title: Review}",
        "{key: n_uses, id: uses, kind: action, title: Uses, weight: 6, requires: [n_plan]}",
    ]));
    let skipped = support::accepted(
        &records,
        "- op: apply_override\n  node: n_review\n  override: {keep: {reason: Still needed.}}\n- op: transition\n  node: n_plan\n  transition: {skip: {reason: Not needed.}}\n",
    );
    let derived = derived(&skipped);
    assert_eq!(
        listed(&derived.priority().leverage_from(&key("n_review"))),
        [("n_uses", 6.0, false)]
    );
}

/// Priority, Leverage: a small gate in front of parallel work owned by others has high
/// leverage and low gravity next to a heavy standalone task.
#[test]
fn a_small_gate_has_high_leverage_and_low_gravity() {
    let mut nodes = vec![
        "{key: n_gate, id: gate, kind: action, title: Gate, participations: {k_owner: [e_a]}}"
            .to_owned(),
        "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 10, participations: {k_owner: [e_a]}}".to_owned(),
    ];
    nodes.extend((1..=3).map(|at| owned(&format!("n_part{at}"), "n_gate", 1, Some("e_b"))));
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&format!("{PEOPLE}{}", add(&nodes)));
    let derived = derived(&records);
    assert!(gravity(&derived, "n_gate") < gravity(&derived, "n_heavy"));
    assert!(leverage(&derived, "n_gate") > leverage(&derived, "n_heavy"));
    assert_eq!(leverage(&derived, "n_gate"), 6.0);
}

/// Illustrative example: with the meeting pinned, environment access has higher gravity than
/// the plan, since the plan and everything downstream of it are downstream of access too.
#[test]
fn environment_access_outweighs_the_plan() {
    let decided = support::derived(&support::vendor_after(2), "j_vendor_eval");
    let priority = decided.priority();
    assert!(priority.gravity(&key("n_access")) > priority.gravity(&key("n_plan")));
    let plan_downstream = priority.downstream(&key("n_plan"));
    let access_downstream = priority.downstream(&key("n_access"));
    assert!(access_downstream.contains(&&key("n_plan")));
    assert!(
        plan_downstream
            .iter()
            .all(|node| access_downstream.contains(node))
    );
}

/// D3, C6, C10: the schema's `Derived` carries every node's gravity with its contributors,
/// largest child gravity, leverage with the split by owner, and rank, and the frontiers in
/// rank order; it reads back from JSON as written.
#[test]
fn the_projection_carries_the_signals_and_their_inputs() {
    let records = support::vendor_after(2);
    let graph = support::journey_graph(&records, "j_vendor_eval");
    let derived = support::derived(&records, "j_vendor_eval");
    let projected = derived.to_schema(&graph);
    assert_eq!(projected.frontier, derived.ranking().frontier());
    assert_eq!(projected.nodes.len(), graph.document().nodes.len());
    let kickoff = &projected.nodes[&key("n_kickoff")];
    assert_eq!(
        kickoff.gravity,
        derived.priority().gravity(&key("n_kickoff"))
    );
    assert_eq!(kickoff.gravity_from.total, 11);
    assert_eq!(
        kickoff
            .gravity_from
            .entries
            .as_slice()
            .first()
            .map(|found| found.node.as_str()),
        Some("n_final_report"),
        "largest first"
    );
    assert_eq!(
        listed(kickoff.leverage_from.entries.as_slice()),
        [("n_access", 1.0, false), ("n_workload", 1.0, false)]
    );
    assert_eq!(
        kickoff.rank.map(cairn_schema::Real::get),
        derived.ranking().rank(&key("n_kickoff"))
    );
    let setup = &projected.nodes[&key("n_setup")];
    assert_eq!(
        setup.max_child_gravity,
        derived.priority().gravity(&key("n_access")).into()
    );
    assert_eq!(setup.rank, None, "a group is not ranked");
    let json = cairn_schema::to_json(&projected).unwrap();
    assert_eq!(
        cairn_schema::from_json::<cairn_schema::Derived>(&json).unwrap(),
        projected
    );
}

/// ARCHITECTURE, Read path: a response carries the largest explanation entries up to the
/// limit and the total; the engine lists them all.
#[test]
fn the_projection_cuts_long_explanations_to_the_limit() {
    let mut nodes = vec!["{key: n_root, id: root, kind: action, title: Root}".to_owned()];
    nodes.extend((0..60).map(|at| {
        format!("{{key: n_after{at:02}, id: after{at:02}, kind: action, title: After, requires: [n_root]}}")
    }));
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = derived(&records);
    let root = &derived.to_schema(&graph).nodes[&key("n_root")];
    let limit = usize::try_from(cairn_schema::limits::EXPLANATION_ENTRY_COUNT_MAX).unwrap();
    assert_eq!(
        (root.gravity_from.entries.len(), root.gravity_from.total),
        (limit, 60)
    );
    assert_eq!(
        (root.leverage_from.entries.len(), root.leverage_from.total),
        (limit, 60)
    );
    assert_eq!(derived.priority().gravity_from(&key("n_root")).len(), 60);
    assert_eq!(root.gravity.value(), 61.0);
}
