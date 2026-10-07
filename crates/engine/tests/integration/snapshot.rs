//! I3, the snapshot: what it holds on the vendor evaluation, subtree and depth scoping, paging
//! of its node list and acting frontier, counts that add up, and agreement with the next list.
//! Derived at 2026-10-06.
#![cfg(test)]

use crate::support;

use std::collections::BTreeSet;

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{Cursor, NextQuery, NodeKey, Snapshot, SnapshotScope};
use support::{add_nodes as add, key};

const VENDOR: &str = "j_vendor_eval";

fn snapshot(records: &Records, journey: &str, scope: &SnapshotScope) -> Snapshot {
    let graph = support::journey_graph(records, journey);
    let derived = support::derived(records, journey);
    DerivedJourney::new(&graph, &derived)
        .snapshot(scope)
        .unwrap()
}

fn names(keys: impl IntoIterator<Item = NodeKey>) -> Vec<String> {
    keys.into_iter()
        .map(|key| key.as_str().to_owned())
        .collect()
}

/// I3: after kickoff, the snapshot holds the up-front answers, every in-scope node, the
/// acting frontier as the next list has it, the open decisions by rank, and counts that add
/// up.
#[test]
fn the_snapshot_holds_answers_nodes_frontier_and_counts() {
    let records = support::vendor_after(3);
    let whole = snapshot(&records, VENDOR, &SnapshotScope::default());
    assert_eq!(
        names(whole.answers.keys().cloned()),
        [
            "n_meeting_date",
            "n_partner_runs",
            "n_purpose",
            "n_who_informed",
            "n_who_owns"
        ]
    );
    assert_eq!(
        names(whole.acting_frontier.iter().map(|row| row.key.clone())),
        ["n_access", "n_decision_meeting", "n_workload"]
    );
    assert_eq!(
        names(whole.open_decisions.clone()),
        ["n_comparison_set", "n_findings_reviewer"]
    );
    assert_eq!(names(whole.needs_breakdown.clone()), ["n_workload"]);
    let counts = &whole.counts;
    assert_eq!(counts.by_state.values().sum::<u32>(), counts.in_scope);
    assert_eq!(counts.listed, counts.in_scope);
    assert_eq!(whole.nodes.len(), usize::try_from(counts.listed).unwrap());
    assert_eq!(counts.not_relevant, 3, "the partner-led subset");
    let graph = support::journey_graph(&records, VENDOR);
    assert_eq!(
        usize::try_from(counts.in_scope + counts.not_relevant).unwrap(),
        graph.document().nodes.len()
    );
    assert_eq!(counts.acting_frontier, 3);
    let access = whole
        .nodes
        .iter()
        .find(|node| node.row.key == key("n_access"));
    assert!(
        access
            .unwrap()
            .participations
            .contains_key(&cairn_schema::KindKey::owner())
    );
}

/// I3: a subtree scopes everything; a depth bounds only the node list.
#[test]
fn a_subtree_and_a_depth_scope_the_snapshot() {
    let records = support::vendor_after(3);
    let setup = snapshot(
        &records,
        VENDOR,
        &SnapshotScope {
            subtree: Some(key("n_setup")),
            depth: Some(1),
            cursor: Cursor::START,
        },
    );
    assert_eq!(
        names(setup.nodes.iter().map(|node| node.row.key.clone())),
        ["n_setup", "n_access", "n_plan", "n_workload"]
    );
    assert_eq!((setup.counts.listed, setup.counts.in_scope), (4, 6));
    assert_eq!(
        names(setup.acting_frontier.iter().map(|row| row.key.clone())),
        ["n_access", "n_workload"]
    );
    assert_eq!(setup.answers.len(), 0, "no decision sits in Setup");
    let roots = snapshot(
        &records,
        VENDOR,
        &SnapshotScope {
            depth: Some(1),
            ..SnapshotScope::default()
        },
    );
    assert!(roots.nodes.iter().all(|node| node.row.ancestors.is_empty()));
    let graph = support::journey_graph(&records, VENDOR);
    let derived = support::derived(&records, VENDOR);
    let unknown = SnapshotScope {
        subtree: Some(key("n_nowhere")),
        ..SnapshotScope::default()
    };
    assert_eq!(
        DerivedJourney::new(&graph, &derived).snapshot(&unknown),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

/// I3: past `page_item_count_max`, the node list pages and the acting frontier keeps its top
/// N with the rest as keys; the top N are the next list's.
#[test]
fn the_snapshot_pages_and_cuts_the_frontier_at_its_limit() {
    let nodes: Vec<String> = (0..205)
        .map(|at| format!("{{key: n_a{at:03}, id: a{at:03}, kind: action, title: Action}}"))
        .collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let first = snapshot(&records, support::JOURNEY, &SnapshotScope::default());
    assert_eq!((first.nodes.len(), first.counts.listed), (200, 205));
    assert_eq!(
        (
            first.acting_frontier.len(),
            first.acting_frontier_rest.len()
        ),
        (200, 5)
    );
    assert_eq!(first.counts.acting_frontier, 205);
    let graph = support::journey_graph(&records, support::JOURNEY);
    let derived = support::derived(&records, support::JOURNEY);
    let next = DerivedJourney::new(&graph, &derived)
        .next(&NextQuery::default(), &BTreeSet::new())
        .unwrap();
    assert_eq!(first.acting_frontier, next.items[..200]);
    let second = snapshot(
        &records,
        support::JOURNEY,
        &SnapshotScope {
            cursor: first.next.unwrap(),
            ..SnapshotScope::default()
        },
    );
    assert_eq!((second.nodes.len(), second.next), (5, None));
}
