//! Brief 5.2 (native half): the in-browser root reads a route and its published versions as
//! the API answers them, and a route's graph, which has no state, has a canvas level by the
//! journey level's rules (C2): every node of the graph when every kind is shown, its implicit
//! gates among the edges, and hidden actions rolled up into their deliverable.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_schema::{EdgeOrigin, Level, NodeKind, Route, RouteVersion};
use cairn_wasm::{BrowserRoot, HostError, RouteLevelRequest, route_level, route_level_of};

fn version_one(root: &BrowserRoot) -> RouteVersion {
    serde_json::from_str(&root.route_version("vendor-evaluation", "1").unwrap()).unwrap()
}

fn level(version: &RouteVersion, shown: &[NodeKind]) -> Level {
    route_level_of(&RouteLevelRequest {
        graph: version.graph.clone(),
        deployment: serde_json::from_str(&BrowserRoot::seeded().unwrap().deployment().unwrap())
            .unwrap(),
        today: "2026-10-06".parse().unwrap(),
        shown: shown.iter().copied().collect(),
        container: None,
    })
    .unwrap()
}

#[test]
fn the_root_reads_a_route_and_its_versions() {
    let root = BrowserRoot::seeded().unwrap();
    let route: Route = serde_json::from_str(&root.route("vendor-evaluation").unwrap()).unwrap();
    assert!(
        route
            .versions
            .iter()
            .any(|number| number.to_string() == "1")
    );
    assert_eq!(version_one(&root).version.to_string(), "1");
    let missing: HostError =
        serde_json::from_str(&root.route("no-such-route").unwrap_err()).unwrap();
    assert!(matches!(missing, HostError::Missing { .. }));
    let missing: HostError =
        serde_json::from_str(&root.route_version("vendor-evaluation", "99").unwrap_err()).unwrap();
    assert!(matches!(missing, HostError::Missing { .. }));
}

#[test]
fn every_node_of_a_route_is_on_its_canvas_with_its_implicit_gates() {
    let version = version_one(&BrowserRoot::seeded().unwrap());
    let all = level(&version, &NodeKind::ALL);
    let drawn: BTreeSet<_> = all.nodes.iter().map(|node| node.key.clone()).collect();
    let graph: BTreeSet<_> = version
        .graph
        .nodes
        .values()
        .map(|node| node.key.clone())
        .collect();
    assert_eq!(drawn, graph);
    let origins: BTreeSet<_> = all
        .edges
        .iter()
        .flat_map(|edge| edge.underlying.iter().map(|underlying| underlying.origin))
        .collect();
    assert_eq!(
        origins,
        BTreeSet::from([
            EdgeOrigin::Explicit,
            EdgeOrigin::Condition,
            EdgeOrigin::StageOpening
        ])
    );
}

#[test]
fn hidden_actions_roll_up_into_their_deliverable() {
    let version = version_one(&BrowserRoot::seeded().unwrap());
    let shown = [
        NodeKind::Group,
        NodeKind::Decision,
        NodeKind::Deliverable,
        NodeKind::Milestone,
    ];
    let plan = level(&version, &shown)
        .nodes
        .into_iter()
        .find(|node| node.key.to_string() == "n_plan")
        .unwrap();
    let rolled: Vec<String> = plan.rolled_up.iter().map(ToString::to_string).collect();
    assert_eq!(rolled, ["n_plan_draft", "n_plan_review"]);
}

#[test]
fn an_unreadable_request_is_refused() {
    let refused: HostError = serde_json::from_str(&route_level("{}").unwrap_err()).unwrap();
    assert!(matches!(refused, HostError::Unreadable { .. }));
}
