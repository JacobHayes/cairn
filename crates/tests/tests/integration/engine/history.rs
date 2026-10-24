//! Explanation paging (ARCHITECTURE, Read path: server responses page each explanation list
//! past `explanation_entry_count_max`) and J4's history, per journey and per node, grouped by
//! patch. Derived at 2026-10-06.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{Derived, DerivedJourney, Graph, history};
use cairn_schema::{Cursor, ExplainedField, NodeKey};
use support::{add_nodes as add, key};

/// ARCHITECTURE, Read path: an explanation list pages `explanation_entry_count_max` entries at
/// a time, largest first; its first page is what the schema's `Derived` carries.
#[test]
fn explanations_page_past_the_response_limit() {
    let mut nodes = vec!["{key: n_root, id: root, kind: action, title: Root}".to_owned()];
    nodes.extend((0..60).map(|at| {
        format!("{{key: n_d{at:02}, id: d{at:02}, kind: action, title: After, requires: [n_root]}}")
    }));
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let graph: Graph = support::journey_graph(&records, support::JOURNEY);
    let derived: Derived = support::derived(&records, support::JOURNEY);
    let journey = DerivedJourney::new(&graph, &derived);
    let root = key("n_root");
    let first = journey
        .explanations(&root, ExplainedField::Gravity, Cursor::START)
        .unwrap();
    assert_eq!((first.entries.len(), first.total), (50, 60));
    let carried = derived.to_schema(&graph).nodes[&root].gravity_from.clone();
    assert_eq!(first.entries, carried.entries.into_vec());
    let second = journey
        .explanations(&root, ExplainedField::Gravity, first.next.unwrap())
        .unwrap();
    assert_eq!((second.entries.len(), second.next), (10, None));
}

/// ARCHITECTURE, Read path: a date chain a response carries is cut to
/// `explanation_entry_count_max` constraints with the total, and the pages of its explanation
/// put together equal the whole chain, in chain order.
#[test]
fn a_date_chain_is_cut_to_the_limit_and_pages_to_the_whole() {
    let mut nodes =
        vec!["{key: n_c000, id: c000, kind: action, title: First, estimate: 1}".to_owned()];
    nodes.extend((1..70).map(|at| {
        format!(
            "{{key: n_c{at:03}, id: c{at:03}, kind: action, title: Step, estimate: 1, requires: [n_c{prev:03}]}}",
            prev = at - 1
        )
    }));
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let records = support::journey(&add(&nodes));
    let graph: Graph = support::journey_graph(&records, support::JOURNEY);
    let derived: Derived = support::derived(&records, support::JOURNEY);
    let journey = DerivedJourney::new(&graph, &derived);
    let last = key("n_c069");
    let whole = derived
        .dates()
        .node(&graph, &last)
        .earliest_start
        .unwrap()
        .chain;
    assert!(whole.constraints.len() > 100, "a chain past the limit");
    let carried = derived.dates().node(&graph, &last).for_response();
    let cut = carried.earliest_start.unwrap().chain;
    assert_eq!(cut.constraints.len(), 50);
    assert_eq!(cut.constraints, whole.constraints[..50]);
    assert_eq!(
        usize::try_from(cut.total).unwrap(),
        whole.constraints.len() + whole.fixed.len()
    );
    let mut constraints = Vec::new();
    let mut fixed = Vec::new();
    let mut cursor = Cursor::START;
    loop {
        let page = journey
            .explanations(&last, ExplainedField::EarliestStart, cursor)
            .unwrap();
        assert_eq!(page.total, cut.total);
        constraints.extend(page.constraints);
        fixed.extend(page.fixed);
        match page.next {
            Some(next) => cursor = next,
            None => break,
        }
    }
    assert_eq!(constraints, whole.constraints);
    assert_eq!(fixed, whole.fixed);
    // A chain within the limit is given whole, with no total.
    let short = derived.dates().node(&graph, &key("n_c003")).for_response();
    assert_eq!(short.earliest_start.unwrap().chain.total, 0);
}

/// J4: history per journey and per node, grouped by patch.
#[test]
fn history_is_grouped_by_patch_per_journey_and_per_node() {
    let (_, applied) = support::run("vendor-evaluation");
    let events: Vec<cairn_schema::Event> = applied
        .iter()
        .flat_map(|step| step.events().iter().cloned())
        .collect();
    let journey = history(&events, None, Cursor::START);
    assert_eq!(usize::try_from(journey.total).unwrap(), events.len());
    assert_eq!(journey.patches.len(), applied.len());
    let access = history(&events, Some(&key("n_access")), Cursor::START);
    let patches: Vec<(&str, usize)> = access
        .patches
        .iter()
        .map(|patch| (patch.patch_id.as_str(), patch.events.len()))
        .collect();
    assert_eq!(patches, [("p_ve_01", 1), ("p_ve_04", 2), ("p_ve_05", 1)]);
    assert_eq!(access.next, None);
    let unknown: NodeKey = key("n_nowhere");
    assert_eq!(history(&events, Some(&unknown), Cursor::START).total, 0);
}
