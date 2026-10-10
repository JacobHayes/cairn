//! Pass 7 (PRD Priority; C5, C10): urgency and lateness from slack, null slack, normalization
//! over the normalization set, the tie order, "prioritize for me" leaving the global rank
//! unchanged, and effort-adjusted ordering. Hand-built journeys are created on 2026-10-06 and
//! derived at that day.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{Derived, Records};
use cairn_schema::{EntityKey, NodeKey, RankConstants, Real};
use support::{add_nodes as add, key};

fn derived(records: &Records) -> Derived {
    support::derived(records, support::JOURNEY)
}

fn derived_with(records: &Records, constants: RankConstants) -> Derived {
    let mut inputs = cairn_engine::testing::derive_inputs(records.deployment.clone());
    inputs.rank = constants;
    let graph = support::journey_graph(records, support::JOURNEY);
    cairn_engine::derive(&graph, None, &inputs)
}

fn names(keys: &[NodeKey]) -> Vec<&str> {
    keys.iter().map(NodeKey::as_str).collect()
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-12
}

/// A milestone pinned on `date` that requires `node`, so `node`'s latest start is `date`.
fn deadline(milestone: &str, node: &str, date: &str) -> (String, String) {
    let id = &milestone[2..];
    (
        format!("{{key: {milestone}, id: {id}, kind: milestone, title: Due, requires: [{node}]}}"),
        format!("- op: set_pin\n  node: {milestone}\n  date: \"{date}\"\n"),
    )
}

/// Builds a journey of `plain` nodes and actions with deadlines, pinned.
fn journey(plain: &[&str], deadlines: &[(&str, &str, &str)]) -> Records {
    let mut nodes: Vec<String> = plain.iter().map(|node| (*node).to_owned()).collect();
    let mut pins = String::new();
    for (milestone, node, date) in deadlines {
        let (node_yaml, pin) = deadline(milestone, node, date);
        nodes.push(node_yaml);
        pins.push_str(&pin);
    }
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    support::journey(&format!("{}{pins}", add(&nodes)))
}

const SOON: &str = "{key: n_soon, id: soon, kind: action, title: Soon}";
const LATE: &str = "{key: n_late, id: late, kind: action, title: Late}";
const FREE: &str = "{key: n_free, id: free, kind: action, title: Free}";

/// Priority, Rank: seven days of slack is half urgent; seven days late is fully urgent and
/// half late; null slack gives neither; gravity and unlocks normalize to the largest in the
/// normalization set, the blocked milestones included; ranked by rank.
#[test]
fn urgency_lateness_and_null_slack() {
    let records = journey(
        &[SOON, LATE, FREE],
        &[
            ("n_soon_due", "n_soon", "2026-10-13"),
            ("n_late_due", "n_late", "2026-09-29"),
        ],
    );
    let derived = derived(&records);
    let ranking = derived.ranking();
    let soon = ranking.terms(&key("n_soon")).unwrap();
    assert!(
        close(soon.urgency, 0.5) && close(soon.late, 0.0),
        "{soon:?}"
    );
    let late = ranking.terms(&key("n_late")).unwrap();
    assert!(
        close(late.urgency, 1.0) && close(late.late, 0.5),
        "{late:?}"
    );
    let free = ranking.terms(&key("n_free")).unwrap();
    assert!(
        close(free.urgency, 0.0) && close(free.late, 0.0),
        "{free:?}"
    );
    assert!(close(free.gravity_norm, 0.5), "gravity 1 of 2: {free:?}");
    assert!(close(soon.rank, 0.4 * 0.5 + 0.25 + 0.2), "{soon:?}");
    assert!(close(late.rank, 0.4 + 0.15 * 0.5 + 0.25 + 0.2), "{late:?}");
    assert!(close(free.rank, 0.25 * 0.5), "{free:?}");
    assert!(
        ranking.rank(&key("n_soon_due")).is_some(),
        "a blocked node is in the normalization set"
    );
    assert_eq!(
        names(ranking.frontier()),
        ["n_late", "n_soon", "n_free"],
        "the frontier is ordered by rank"
    );
}

/// Priority, Rank: done, not-relevant, and group nodes are not ranked, and a set whose
/// largest gravity and unlocks are zero normalizes both to zero.
#[test]
fn only_the_normalization_set_is_ranked() {
    let records = support::journey(&add(&[
        "{key: n_stage, id: stage, kind: group, title: Stage}",
        "{key: n_inside, id: inside, parent: n_stage, kind: action, title: Inside, weight: 0}",
        "{key: n_done, id: done, kind: action, title: Done}",
    ]));
    let done = support::accepted(
        &records,
        "- op: transition\n  node: n_done\n  transition: complete\n",
    );
    let derived = derived(&done);
    let ranking = derived.ranking();
    assert_eq!(ranking.rank(&key("n_stage")), None, "a group");
    assert_eq!(ranking.rank(&key("n_done")), None, "terminal");
    let inside = ranking.terms(&key("n_inside")).unwrap();
    assert_eq!(
        (inside.gravity_norm, inside.unlocks_norm, inside.rank),
        (0.0, 0.0, 0.0)
    );
}

/// Priority, Rank: equal ranks break by smaller slack with null last, then greater gravity,
/// then key. With the gravity and unlocks coefficients at zero and every slack past the
/// horizon, every rank is zero.
#[test]
fn ties_break_by_slack_then_gravity_then_key() {
    let records = journey(
        &[
            "{key: n_far, id: far, kind: action, title: Far}",
            "{key: n_near, id: near, kind: action, title: Near}",
            "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 5}",
            "{key: n_light_b, id: light-b, kind: action, title: Light}",
            "{key: n_light_a, id: light-a, kind: action, title: Light}",
        ],
        &[
            ("n_far_due", "n_far", "2026-11-05"),
            ("n_near_due", "n_near", "2026-10-26"),
        ],
    );
    let constants = RankConstants {
        gravity: Real::try_from(0.0).unwrap(),
        unlocks: Real::try_from(0.0).unwrap(),
        ..RankConstants::default()
    };
    let derived = derived_with(&records, constants);
    assert!(
        derived
            .ranking()
            .frontier()
            .iter()
            .all(|node| derived.ranking().rank(node) == Some(0.0))
    );
    assert_eq!(
        names(derived.ranking().frontier()),
        ["n_near", "n_far", "n_heavy", "n_light_a", "n_light_b"]
    );
}

const PEOPLE: &str = "- op: create_entity\n  entity: {key: e_a, name: A}\n- op: create_entity\n  entity: {key: e_b, name: B}\n";

fn viewer(entity: &str) -> BTreeSet<EntityKey> {
    BTreeSet::from([entity.parse().unwrap()])
}

/// Priority, "prioritize for me": each of two gates unblocks the other owner's work, so
/// globally they tie; for a viewer, the gate that frees someone else's work ranks first. The
/// global ranking is unchanged.
#[test]
fn prioritize_for_me_recomputes_the_owner_factor_for_the_viewer() {
    let records = support::journey(&format!(
        "{PEOPLE}{}",
        add(&[
            "{key: n_gate_a, id: gate-a, kind: action, title: Gate, participations: {k_owner: [e_a]}}",
            "{key: n_gate_b, id: gate-b, kind: action, title: Gate, participations: {k_owner: [e_b]}}",
            "{key: n_work_b, id: work-b, kind: action, title: Work, requires: [n_gate_a], participations: {k_owner: [e_b]}}",
            "{key: n_work_a, id: work-a, kind: action, title: Work, requires: [n_gate_b], participations: {k_owner: [e_a]}}",
        ])
    ));
    let derived = derived(&records);
    let global = derived.ranking().clone();
    assert_eq!(names(global.frontier()), ["n_gate_a", "n_gate_b"]);
    assert_eq!(global.rank(&key("n_gate_a")), global.rank(&key("n_gate_b")));
    assert_eq!(global.unlocks(&key("n_gate_a")).unwrap().value(), 2.0);
    let for_a = derived.rank_for(&viewer("e_a"));
    assert_eq!(names(for_a.frontier()), ["n_gate_a", "n_gate_b"]);
    assert_eq!(for_a.unlocks(&key("n_gate_a")).unwrap().value(), 2.0);
    assert_eq!(for_a.unlocks(&key("n_gate_b")).unwrap().value(), 1.0);
    let for_b = derived.rank_for(&viewer("e_b"));
    assert_eq!(names(for_b.frontier()), ["n_gate_b", "n_gate_a"]);
    assert!(for_b.rank(&key("n_gate_b")) > for_b.rank(&key("n_gate_a")));
    assert_eq!(derived.ranking(), &global, "the shared rank is unchanged");
}

/// Priority, Unlocks, Rank: a small gate in front of others' parallel work ranks above a
/// heavy standalone task.
#[test]
fn a_small_gate_ranks_early() {
    let records = support::journey(&format!(
        "{PEOPLE}{}",
        add(&[
            "{key: n_gate, id: gate, kind: action, title: Gate, participations: {k_owner: [e_a]}}",
            "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 10, participations: {k_owner: [e_a]}}",
            "{key: n_one, id: one, kind: action, title: One, requires: [n_gate], participations: {k_owner: [e_b]}}",
            "{key: n_two, id: two, kind: action, title: Two, requires: [n_gate], participations: {k_owner: [e_b]}}",
            "{key: n_three, id: three, kind: action, title: Three, requires: [n_gate], participations: {k_owner: [e_b]}}",
        ])
    ));
    assert_eq!(
        names(derived(&records).ranking().frontier()),
        ["n_gate", "n_heavy"]
    );
}

/// Priority, Effort-adjusted: gravity per estimated day, greatest first; no or a zero
/// estimate last, in rank order.
#[test]
fn effort_adjusted_ordering_puts_unestimated_work_last() {
    let records = support::journey(&add(&[
        "{key: n_long, id: long, kind: action, title: Long, weight: 4, estimate: 2}",
        "{key: n_short, id: short, kind: action, title: Short, weight: 3, estimate: 1}",
        "{key: n_zero, id: zero, kind: action, title: Zero, weight: 9, estimate: 0}",
        "{key: n_none, id: none, kind: action, title: None, weight: 2}",
    ]));
    let derived = derived(&records);
    let graph = support::journey_graph(&records, support::JOURNEY);
    let frontier = derived.blocking().frontier();
    assert_eq!(
        names(&derived.by_effort(&graph, frontier)),
        ["n_short", "n_long", "n_zero", "n_none"]
    );
}

/// Priority, D6: two derives of the same journey rank identically, per viewer too.
#[test]
fn two_derives_rank_identically() {
    let records = support::vendor_after(2);
    let first = support::derived(&records, "j_vendor_eval");
    let second = support::derived(&records, "j_vendor_eval");
    assert_eq!(first.ranking(), second.ranking());
    assert_eq!(
        first.rank_for(&viewer("e_lead")),
        second.rank_for(&viewer("e_lead"))
    );
}
