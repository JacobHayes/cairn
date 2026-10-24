//! I3, the snapshot: what it holds on the vendor evaluation, subtree scoping, the cut of its
//! acting frontier, counts that add up, and agreement with the next list; the node list's
//! depth scoping and its lean rows.
//! Derived at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{
    AgentRow, DisplayState, FrontierEntry, ListQuery, NextQuery, NodeKey, Real, Snapshot,
    SnapshotScope,
};
use support::{add_nodes as add, key};

const VENDOR: &str = "j_vendor_eval";

fn snapshot(records: &Records, journey: &str, scope: &SnapshotScope) -> Snapshot {
    let graph = support::journey_graph(records, journey);
    let derived = support::derived(records, journey);
    DerivedJourney::new(&graph, &derived)
        .snapshot(scope, &support::journey_header(records, journey))
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
        ["n_access", "n_workload"]
    );
    assert_eq!(
        names(whole.open_decisions.iter().map(|each| each.key.clone())),
        ["n_comparison_set", "n_findings_reviewer"]
    );
    assert_eq!(names(whole.needs_breakdown.clone()), ["n_workload"]);
    let counts = &whole.counts;
    assert_eq!(counts.by_state.values().sum::<u32>(), counts.in_scope);
    assert_eq!(counts.not_relevant, 3, "the partner-led subset");
    let graph = support::journey_graph(&records, VENDOR);
    assert_eq!(
        usize::try_from(counts.in_scope + counts.not_relevant).unwrap(),
        graph.document().nodes.len()
    );
    assert_eq!(counts.acting_frontier, 2);
}

/// I3, C9: a subtree scopes the snapshot; the node list scoped to it and to a depth lists that
/// node and its levels, and from the roots its first level.
#[test]
fn a_subtree_scopes_the_snapshot_and_a_depth_the_node_list() {
    let records = support::vendor_after(3);
    let setup = snapshot(
        &records,
        VENDOR,
        &SnapshotScope {
            subtree: Some(key("n_setup")),
        },
    );
    assert_eq!(setup.counts.in_scope, 6);
    assert_eq!(
        names(setup.acting_frontier.iter().map(|row| row.key.clone())),
        ["n_access", "n_workload"]
    );
    assert_eq!(setup.answers.len(), 0, "no decision sits in Setup");
    let graph = support::journey_graph(&records, VENDOR);
    let derived = support::derived(&records, VENDOR);
    let projected = DerivedJourney::new(&graph, &derived);
    let listed = |within: Option<NodeKey>, depth: Option<u32>| -> BTreeSet<String> {
        let query = ListQuery {
            within,
            depth,
            ..ListQuery::default()
        };
        let rows = projected.list(&query, &BTreeSet::new()).unwrap().rows;
        rows.into_iter()
            .map(|row| row.key.as_str().to_owned())
            .collect()
    };
    assert_eq!(
        listed(Some(key("n_setup")), Some(1)),
        ["n_setup", "n_access", "n_plan", "n_workload"]
            .map(str::to_owned)
            .into()
    );
    let deeper = listed(Some(key("n_setup")), None);
    assert!(deeper.len() > 4 && deeper.contains("n_setup"));
    let roots = projected
        .list(
            &ListQuery {
                depth: Some(1),
                ..ListQuery::default()
            },
            &BTreeSet::new(),
        )
        .unwrap()
        .rows;
    assert!(!roots.is_empty() && roots.iter().all(|row| row.ancestors.is_empty()));
    let unknown = SnapshotScope {
        subtree: Some(key("n_nowhere")),
    };
    assert_eq!(
        projected.snapshot(&unknown, &support::journey_header(&records, VENDOR)),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

/// I3: past `page_item_count_max`, the acting frontier keeps its top N with the rest as keys;
/// the top N are the next list's.
#[test]
fn the_snapshot_cuts_the_frontier_at_its_limit() {
    let nodes: Vec<String> = (0..205)
        .map(|at| format!("{{key: n_a{at:03}, id: a{at:03}, kind: action, title: Action}}"))
        .collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let first = snapshot(&records, support::JOURNEY, &SnapshotScope::default());
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
    let entries: Vec<_> = next.items[..200].iter().map(FrontierEntry::of).collect();
    assert_eq!(first.acting_frontier, entries);
}

/// C10: a node list row says what the full row says, less what an agent cannot use: a settled
/// node keeps its dates, flags and containment but no slack or rank, and the rank of a node
/// still to do is the full rank to three figures.
#[test]
fn a_list_row_keeps_the_facts_and_drops_the_working() {
    let mut settled = 0;
    for name in support::fixture_names() {
        for records in [support::after(&name, 3), support::finished(&name)] {
            for id in records.journeys.keys() {
                let journey = id.as_str();
                let graph = support::journey_graph(&records, journey);
                let derived = support::derived(&records, journey);
                let projected = DerivedJourney::new(&graph, &derived);
                let rows = projected
                    .list(&ListQuery::default(), &BTreeSet::new())
                    .unwrap()
                    .rows;
                for row in &rows {
                    let lean = &AgentRow::of(row);
                    assert_eq!(
                        (lean.due, lean.shortfall_days, lean.stale, lean.overdue),
                        (row.due, row.shortfall_days, row.stale, row.overdue)
                    );
                    assert_eq!(lean.owners, row.owners);
                    assert_eq!(lean.ancestors, row.ancestors);
                    if matches!(
                        row.display_state,
                        DisplayState::Done | DisplayState::Skipped | DisplayState::NotRelevant
                    ) {
                        settled += 1;
                        assert_eq!((lean.slack_days, lean.rank), (None, None));
                    } else {
                        assert_eq!(lean.slack_days, row.slack_days);
                        let rank = row
                            .rank
                            .and_then(|terms| Real::three_figures(terms.rank.get()));
                        assert_eq!(lean.rank, rank);
                    }
                }
            }
        }
    }
    assert!(settled > 0, "the fixtures exercise settled nodes");
}
