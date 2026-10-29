//! I3, the snapshot: its counts on the vendor evaluation, each the total of the node list under
//! the matching filter, and counts that add up; the node list's depth scoping and its lean
//! rows. Derived at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_engine::{DerivedJourney, ProjectionError, Records};
use cairn_schema::{AgentRow, DisplayState, ListFlag, ListQuery, NodeKey, Real, Snapshot};
use support::{add_nodes as add, key};

const VENDOR: &str = "j_vendor_eval";

fn snapshot(records: &Records, journey: &str) -> Snapshot {
    let graph = support::journey_graph(records, journey);
    let derived = support::derived(records, journey);
    DerivedJourney::new(&graph, &derived).snapshot(&support::journey_header(records, journey))
}

/// How many nodes the node list holds for under `flag`.
fn listed(records: &Records, journey: &str, flag: ListFlag) -> u32 {
    let graph = support::journey_graph(records, journey);
    let derived = support::derived(records, journey);
    let query = ListQuery {
        flags: [flag].into(),
        ..ListQuery::default()
    };
    DerivedJourney::new(&graph, &derived)
        .list(&query, &BTreeSet::new())
        .unwrap()
        .total
}

/// I3: after kickoff, the snapshot's counts add up, and each count of nodes needing attention
/// is the total the node list gives under the matching filter.
#[test]
fn the_snapshot_counts_add_up_and_match_the_node_list_filters() {
    let records = support::vendor_after(3);
    let whole = snapshot(&records, VENDOR);
    let counts = &whole.counts;
    assert_eq!(counts.by_state.values().sum::<u32>(), counts.in_scope);
    assert_eq!(
        counts.by_display_state.values().sum::<u32>(),
        counts.in_scope
    );
    assert_eq!(counts.not_relevant, 3, "the partner-led subset");
    let graph = support::journey_graph(&records, VENDOR);
    assert_eq!(
        usize::try_from(counts.in_scope + counts.not_relevant).unwrap(),
        graph.document().nodes.len()
    );
    assert_eq!(counts.acting_frontier, 2);
    assert_eq!(counts.open_decisions, 2, "the comparison set and reviewer");
    assert_eq!(counts.needs_breakdown, 1, "the workload");
    for (count, flag) in [
        (counts.acting_frontier, ListFlag::NextUp),
        (counts.open_decisions, ListFlag::OpenDecision),
        (counts.needs_breakdown, ListFlag::NeedsBreakdown),
        (counts.unassigned, ListFlag::Unassigned),
        (counts.shortfalls, ListFlag::Shortfall),
        (counts.stalled, ListFlag::Stalled),
    ] {
        assert_eq!(count, listed(&records, VENDOR, flag), "{flag:?}");
    }
}

/// C9: the node list scoped to a node and to a depth lists that node and its levels, and from
/// the roots its first level; a node it does not have is an error.
#[test]
fn a_depth_scopes_the_node_list() {
    let records = support::vendor_after(3);
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
    let unknown = ListQuery {
        within: Some(key("n_nowhere")),
        ..ListQuery::default()
    };
    assert_eq!(
        projected.list(&unknown, &BTreeSet::new()),
        Err(ProjectionError::UnknownNode(key("n_nowhere")))
    );
}

/// I3: past `page_item_count_max` nodes the snapshot is as small as ever: counts only, with the
/// totals the node list pages.
#[test]
fn the_snapshot_stays_small_however_many_nodes_there_are() {
    let nodes: Vec<String> = (0..205)
        .map(|at| format!("{{key: n_a{at:03}, id: a{at:03}, kind: action, title: Action}}"))
        .collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let first = snapshot(&records, support::JOURNEY);
    assert_eq!(first.counts.acting_frontier, 205);
    assert_eq!(first.counts.unassigned, 205);
    assert_eq!(
        listed(&records, support::JOURNEY, ListFlag::Unassigned),
        205
    );
    let small = snapshot(
        &support::journey(&add(&["{key: n_a, id: a, kind: action, title: Action}"])),
        support::JOURNEY,
    );
    let size = |snapshot: &Snapshot| serde_json::to_string(snapshot).unwrap().len();
    assert!(
        size(&first) < size(&small) + 100,
        "the size does not grow with the nodes"
    );
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
