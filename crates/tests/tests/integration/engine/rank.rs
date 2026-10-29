//! Pass 7 (PRD Priority; C5, C10): urgency and lateness from slack, null slack, normalization
//! over the normalization set, the tie order, and the unlocks gate. Hand-built journeys are created on 2026-10-06 and
//! derived at that day.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Derived, Records};
use cairn_schema::NodeKey;
use support::{add_nodes as add, key};

fn derived(records: &Records) -> Derived {
    support::derived(records, support::JOURNEY)
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
/// then key. Every slack here is past the horizon, so urgency is 0 and equal ranks follow
/// from equal gravity.
#[test]
fn ties_break_by_slack_then_key() {
    let records = journey(
        &[
            "{key: n_far, id: far, kind: action, title: Far}",
            "{key: n_near, id: near, kind: action, title: Near}",
            "{key: n_light_b, id: light-b, kind: action, title: Light}",
            "{key: n_light_a, id: light-a, kind: action, title: Light}",
        ],
        &[
            ("n_far_due", "n_far", "2026-11-05"),
            ("n_near_due", "n_near", "2026-10-26"),
        ],
    );
    let derived = derived(&records);
    let ranking = derived.ranking();
    assert_eq!(ranking.rank(&key("n_far")), ranking.rank(&key("n_near")));
    assert_eq!(
        names(ranking.frontier()),
        ["n_near", "n_far", "n_light_a", "n_light_b"]
    );
}

const PEOPLE: &str = "- op: create_identity\n  identity: {key: who_a, name: A}\n- op: create_identity\n  identity: {key: who_b, name: B}\n";

/// Priority, Unlocks, Rank: a small gate in front of others' parallel work ranks above a
/// heavy standalone task.
#[test]
fn a_small_gate_ranks_early() {
    let records = support::journey(&format!(
        "{PEOPLE}{}",
        add(&[
            "{key: n_gate, id: gate, kind: action, title: Gate, participations: {k_owner: [who_a]}}",
            "{key: n_heavy, id: heavy, kind: action, title: Heavy, weight: 10, participations: {k_owner: [who_a]}}",
            "{key: n_one, id: one, kind: action, title: One, requires: [n_gate], participations: {k_owner: [who_b]}}",
            "{key: n_two, id: two, kind: action, title: Two, requires: [n_gate], participations: {k_owner: [who_b]}}",
            "{key: n_three, id: three, kind: action, title: Three, requires: [n_gate], participations: {k_owner: [who_b]}}",
        ])
    ));
    assert_eq!(
        names(derived(&records).ranking().frontier()),
        ["n_gate", "n_heavy"]
    );
}
