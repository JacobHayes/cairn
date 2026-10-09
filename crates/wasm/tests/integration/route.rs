//! Brief 5.2 (native half): the in-browser root reads a route and its published versions as
//! the API answers them, and a route's graph, which has no state, has a canvas level by the
//! journey level's rules (C2): every node of the graph when every kind is shown, its implicit
//! gates among the edges, and hidden actions rolled up into their deliverable.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_schema::{EdgeOrigin, Level, NodeKind, Rejection, Route, RouteVersion, ViolationCode};
use cairn_wasm::{
    BrowserRoot, HostError, RouteApplyRequest, RouteLevelRequest, RouteNoticesRequest,
    apply_route_locally, route_level, route_level_of, route_notices,
};

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

/// A20: the notices of a route graph as the authoring view reads them.
#[test]
fn a_route_graph_lists_the_work_its_final_milestone_cannot_see() {
    let request = RouteNoticesRequest {
        graph: version_one(&BrowserRoot::seeded().unwrap()).graph,
    };
    let answer = route_notices(&serde_json::to_string(&request).unwrap()).unwrap();
    let listed: Vec<cairn_schema::Notice> = serde_json::from_str(&answer).unwrap();
    let paths: Vec<String> = listed
        .iter()
        .map(|notice| notice.path.to_string())
        .collect();
    assert_eq!(paths, ["purpose", "setup/workload"]);
}

#[test]
fn an_unreadable_request_is_refused() {
    let refused: HostError = serde_json::from_str(&route_level("{}").unwrap_err()).unwrap();
    assert!(matches!(refused, HostError::Unreadable { .. }));
}

fn route_apply(route: Option<Route>, patch: &str) -> Result<Route, HostError> {
    let deployment =
        serde_json::from_str(&BrowserRoot::seeded().unwrap().deployment().unwrap()).unwrap();
    apply_route_locally(&RouteApplyRequest {
        route,
        versions: Vec::new(),
        deployment,
        patch: serde_json::from_str(patch).unwrap(),
        today: "2026-10-06".parse().unwrap(),
        at: "2026-10-06T12:00:00Z".parse().unwrap(),
        actor: serde_json::from_str(r#"{"user": "u_local"}"#).unwrap(),
    })
}

/// Brief 5.6: a route authored by hand (A12) is previewed locally: a new route with an empty
/// draft, a node added to the draft, and a node whose id a sibling holds refused with the
/// engine's violation (A15), each without committing anything.
#[test]
fn a_route_draft_is_authored_locally_and_refused_with_its_violations() {
    let created = route_apply(
        None,
        r#"{"id": "p_one", "target": {"route": "drafted"}, "base_revision": 0, "mutations": [
            {"op": "create_route", "name": "Drafted"}, {"op": "open_draft", "source": "edit"}]}"#,
    )
    .unwrap();
    assert!(created.draft.as_ref().unwrap().graph.nodes.is_empty());
    let node = |key: &str| {
        format!(
            r#"{{"id": "p_{key}", "target": {{"route": "drafted"}}, "base_revision": {}, "mutations": [
            {{"op": "add_node", "node": {{"key": "n_{key}", "id": "same", "kind": "milestone", "title": "Same"}}}}]}}"#,
            created.revision
        )
    };
    let added = route_apply(Some(created.clone()), &node("first")).unwrap();
    assert_eq!(added.draft.as_ref().unwrap().graph.nodes.len(), 1);
    assert!(added.revision > created.revision);
    let mut again = added.clone();
    again.revision = created.revision;
    let refused = route_apply(Some(again), &node("second")).unwrap_err();
    let HostError::Rejected { rejection } = refused else {
        panic!("expected a rejection, got {refused:?}")
    };
    let Rejection::Invalid { violations } = rejection else {
        panic!("expected violations, got {rejection:?}")
    };
    let codes: Vec<_> = violations
        .as_slice()
        .iter()
        .map(|violation| violation.code)
        .collect();
    assert_eq!(codes, [ViolationCode::DuplicateSiblingId]);
    let elsewhere = route_apply(
        Some(created),
        r#"{"id": "p_x", "target": {"route": "other"}, "base_revision": 1, "mutations": [{"op": "publish_draft"}]}"#,
    );
    assert!(matches!(elsewhere, Err(HostError::Missing { .. })));
}
