//! A route's graph, which has no state, has a canvas level by the journey level's rules (C2): every node of the graph when every kind is shown, its implicit
//! gates among the edges, and hidden actions rolled up into their deliverable.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_engine::from_file;
use cairn_schema::{
    Deployment, EdgeOrigin, Graph, Level, LevelDisplay, LevelQuery, NodeKey, NodeKind, Payload,
    Rejection, Route, RouteFile, RouteSchedule, SequentialKeys, Trace, ViolationCode, from_yaml,
};
use cairn_wasm::{
    Derivation, HostError, Projection, RouteApplyRequest, RouteDerivation, RouteDeriveRequest,
    RouteNoticesRequest, apply_route_locally, read_document, route_notices,
};

/// The graph of a fixture route, built as published.
fn graph_of(text: &str) -> Graph {
    let file: RouteFile = from_yaml(text).unwrap();
    from_file(&file, &mut SequentialKeys::default())
        .unwrap()
        .into_document()
}

fn vendor_evaluation() -> Graph {
    graph_of(include_str!(
        "../../../../../fixtures/vendor-evaluation/route.yaml"
    ))
}

fn derived(graph: &Graph) -> RouteDerivation {
    RouteDerivation::of(&RouteDeriveRequest {
        graph: graph.clone(),
        deployment: Deployment::default(),
    })
    .unwrap()
}

fn level_of(graph: &Graph, query: LevelQuery) -> Level {
    let answer = derived(graph).projected(&Projection::Level(query)).unwrap();
    serde_json::from_str(&answer).unwrap()
}

fn trace_of(derivation: &RouteDerivation, node: &str) -> Trace {
    let request = Projection::Trace {
        key: node.parse().unwrap(),
        standing: Vec::new(),
    };
    serde_json::from_str(&derivation.projected(&request).unwrap()).unwrap()
}

fn keys(nodes: &[NodeKey]) -> BTreeSet<String> {
    nodes.iter().map(ToString::to_string).collect()
}

fn level(graph: &Graph, shown: &[NodeKind]) -> Level {
    level_of(
        graph,
        LevelQuery::of_kinds(shown.iter().copied().collect(), None),
    )
}

#[test]
fn every_node_of_a_route_is_on_its_canvas_with_its_implicit_gates() {
    let graph = vendor_evaluation();
    let all = level(&graph, &NodeKind::ALL);
    let drawn: BTreeSet<_> = all.nodes.iter().map(|node| node.key.clone()).collect();
    let in_graph: BTreeSet<_> = graph.nodes.values().map(|node| node.key.clone()).collect();
    assert_eq!(drawn, in_graph);
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
    let graph = vendor_evaluation();
    let shown = [
        NodeKind::Group,
        NodeKind::Decision,
        NodeKind::Deliverable,
        NodeKind::Milestone,
    ];
    let plan = level(&graph, &shown)
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
        graph: vendor_evaluation(),
    };
    let answer = route_notices(&serde_json::to_string(&request).unwrap()).unwrap();
    let listed: Vec<cairn_schema::Notice> = serde_json::from_str(&answer).unwrap();
    let paths: Vec<String> = listed
        .iter()
        .map(|notice| notice.path.to_string())
        .collect();
    assert_eq!(paths, ["purpose", "setup/workload"]);
}

/// C2, C9: a route's level takes the query a journey's does: a hidden relevance class and a
/// text search narrow it as they narrow a journey's, so the wasm entry carries the whole query.
#[test]
fn a_route_level_honours_the_classes_and_the_search_of_the_query() {
    let graph = vendor_evaluation();
    let everything = level(&graph, &NodeKind::ALL);
    let mut query = LevelQuery::of_kinds(NodeKind::ALL.iter().copied().collect(), None);
    query.display = BTreeSet::from([LevelDisplay::Relevant]);
    let relevant = level_of(&graph, query);
    assert!(relevant.nodes.len() < everything.nodes.len());
    assert!(
        relevant
            .hidden_by_class
            .get(&LevelDisplay::Conditional)
            .copied()
            .unwrap_or(0)
            > 0
    );
    let title = everything.nodes[0].key.clone();
    let word = graph.nodes.get(&title).unwrap().title.to_string();
    let mut query = LevelQuery::of_kinds(NodeKind::ALL.iter().copied().collect(), None);
    query.text = Some(word.parse().unwrap());
    let found = level_of(&graph, query);
    assert!(found.nodes.iter().any(|node| node.key == title));
    assert!(found.nodes.len() < everything.nodes.len());
}

/// C13: the product launch's schedule from day 0: the feature work starts the build and runs
/// its estimate, hardening starts where it ends, and the code freeze that closes the build is
/// no earlier than hardening's end. The estimates are read off the fixture, not typed in.
#[test]
fn a_route_schedule_places_the_work_in_days_from_the_start() {
    let launch = graph_of(include_str!(
        "../../../../../fixtures/product-launch/route.yaml"
    ));
    let derivation = derived(&launch);
    let schedule: RouteSchedule = serde_json::from_str(&derivation.schedule()).unwrap();
    assert_eq!(schedule, derivation.scheduled());
    let estimate = |node: &str| {
        let payload = &launch.nodes.get(&node.parse().unwrap()).unwrap().payload;
        let days = match payload {
            Payload::Deliverable(deliverable) => deliverable.estimate,
            Payload::Action(action) => action.estimate,
            other => panic!("{node} is not work: {other:?}"),
        };
        i32::try_from(days.unwrap().get()).unwrap()
    };
    let days = |node: &str| {
        let entry = schedule
            .entries
            .iter()
            .find(|entry| entry.node.as_str() == node)
            .unwrap_or_else(|| panic!("{node} has an earliest day"));
        (entry.start_day, entry.finish_day)
    };
    assert_eq!(days("n_features"), (0, estimate("n_features")));
    assert_eq!(
        days("n_hardening"),
        (
            days("n_features").1,
            days("n_features").1 + estimate("n_hardening")
        )
    );
    assert!(days("n_code_freeze").0 >= days("n_hardening").1);
    assert_eq!(schedule.end, Some("n_launch".parse().unwrap()));
}

#[test]
fn an_unreadable_route_request_is_refused() {
    let refused = RouteDerivation::new("{}").err().unwrap();
    let refused: HostError = serde_json::from_str(&refused).unwrap();
    assert!(matches!(refused, HostError::Unreadable { .. }));
}

/// C7: a route version's trace is structural: a node under a stage has the stage's opening
/// and its gating decision upstream and its ancestors downstream, and the held derivation
/// answers the trace of a journey that has just been created from the version identically.
#[test]
fn a_route_version_traces_as_a_journey_just_created_from_it_does() {
    let graph = vendor_evaluation();
    let route = derived(&graph);
    let mut pristine = read_document(&super::document::document()).unwrap();
    pristine.journey.graph = graph.clone();
    let journey = Derivation::of(pristine).unwrap();
    let mut traced_something = false;
    for node in graph.nodes.values().map(|node| &node.key) {
        let request = Projection::Trace {
            key: node.clone(),
            standing: Vec::new(),
        };
        let ours = route.projected(&request).unwrap();
        assert_eq!(ours, journey.projected(&request).unwrap(), "{node}");
        let trace: Trace = serde_json::from_str(&ours).unwrap();
        traced_something |= !trace.upstream.is_empty() && !trace.downstream.is_empty();
    }
    assert!(traced_something, "some node has both sides to its trace");
}

fn route_apply(route: Option<Route>, patch: &str) -> Result<Route, HostError> {
    apply_route_locally(&RouteApplyRequest {
        route,
        versions: Vec::new(),
        segments: Vec::new(),
        deployment: Deployment::default(),
        patch: serde_json::from_str(patch).unwrap(),
        today: "2026-10-06".parse().unwrap(),
        at: "2026-10-06T12:00:00Z".parse().unwrap(),
        actor: serde_json::from_str(r#"{"user": "u_local"}"#).unwrap(),
    })
}

/// A route authored by hand (A12) is previewed locally: a new route with an empty
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
    // The two siblings sharing the id are each reported.
    assert_eq!(codes, [ViolationCode::DuplicateSiblingId; 2]);
    let elsewhere = route_apply(
        Some(created),
        r#"{"id": "p_x", "target": {"route": "other"}, "base_revision": 1, "mutations": [{"op": "publish_draft"}]}"#,
    );
    assert!(matches!(elsewhere, Err(HostError::Missing { .. })));
}

/// C7: a node behind a condition traces to the decision that gates it and to the stages it
/// sits in; a group adds nothing to the gravity of what is inside it.
#[test]
fn a_node_behind_a_condition_traces_to_its_gate_and_its_stages() {
    let graph = vendor_evaluation();
    let trace = trace_of(&derived(&graph), "n_partner_results");
    let set = |nodes: &[&str]| {
        nodes
            .iter()
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        keys(&trace.upstream),
        set(&["n_criteria", "n_partner_runs"])
    );
    assert!(keys(&trace.downstream).is_superset(&set(&[
        "n_partner_led",
        "n_testing",
        "n_reporting"
    ])));
    let contributors = keys(&trace.gravity_contributors);
    assert!(contributors.contains("n_final_report"));
    assert!(contributors.is_disjoint(&set(&["n_partner_led", "n_testing", "n_reporting"])));
}
