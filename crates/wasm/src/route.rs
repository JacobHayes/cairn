//! A route's graph on the canvas (brief 5.2): a route draft or a published version is a graph
//! with no state, so its canvas level (C2) is the engine's `level` over the graph derived as a
//! journey in which nothing has happened yet. The rules for what is visible, what rolls up,
//! and which edges are drawn are the engine's, as on a journey; the canvas draws the structure
//! and none of the state.
//!
//! Cost: the engine's derive of the graph (ARCHITECTURE, Read path) and its level (its module
//! states the cost at `node_count_max`).

use std::collections::BTreeSet;

use cairn_engine::{DerivedJourney, Graph, derive, notices};
use cairn_schema::{
    Date, Deployment, DeriveInputs, Graph as GraphDocument, Level, LevelQuery, NodeKey, NodeKind,
    Notice, RankConstants,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};

/// C2: one canvas level of a route's graph (its draft or a version), with the deployment its
/// conditions read entities from and the day its date rules are read on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteLevelRequest {
    /// The route's graph.
    pub graph: GraphDocument,
    /// The deployment context (E6).
    pub deployment: Deployment,
    /// Today, which nothing the level answers depends on but the derive reads.
    pub today: Date,
    /// The kinds shown.
    pub shown: BTreeSet<NodeKind>,
    /// The container drilled into; the top level when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<NodeKey>,
}

/// C2: the level of a route's graph for the shown kinds within the container.
///
/// # Errors
///
/// [`HostError::Invalid`] when the graph breaks an invariant, which a stored route never does;
/// [`HostError::Missing`] when the container is not a node of it.
pub fn route_level_of(request: &RouteLevelRequest) -> Result<Level, HostError> {
    let graph = Graph::new(request.graph.clone(), &request.deployment)
        .map_err(|violations| HostError::Invalid { violations })?;
    let inputs = DeriveInputs {
        today: request.today,
        timezone: "UTC"
            .parse()
            .map_err(|error| HostError::unreadable("timezone", error))?,
        rank: RankConstants::default(),
        viewer: BTreeSet::new(),
        deployment: request.deployment.clone(),
    };
    let derived = derive(&graph, None, &inputs);
    let query = LevelQuery::of_kinds(request.shown.clone(), request.container.clone());
    DerivedJourney::new(&graph, &derived)
        .level(&query, &request.deployment)
        .map_err(|error| HostError::Missing {
            message: error.to_string(),
        })
}

/// C2: a route's canvas level: `request` is the JSON of a [`RouteLevelRequest`]; the answer is
/// the JSON of a `Level`, as a journey's level projection answers it.
///
/// # Errors
///
/// The JSON of a [`HostError`]: an unreadable request, an invalid graph, or a container the
/// graph lacks.
#[wasm_bindgen(js_name = routeLevel)]
pub fn route_level(request: &str) -> Result<String, String> {
    Ok(json(&route_level_of(&read(
        "route level request",
        request,
    )?)?))
}

/// A20: the notices of a route's graph, as the authoring view lists them while the draft
/// changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteNoticesRequest {
    /// The route's graph.
    pub graph: GraphDocument,
}

/// A20: the notices of a route's graph, in path order.
///
/// # Errors
///
/// [`HostError::Invalid`] when the graph breaks an invariant, which a stored route never does.
pub fn route_notices_of(request: &RouteNoticesRequest) -> Result<Vec<Notice>, HostError> {
    // A route has no answers, so no deployment changes what it means.
    let graph = Graph::new(request.graph.clone(), &Deployment::default())
        .map_err(|violations| HostError::Invalid { violations })?;
    Ok(notices(&graph))
}

/// A20: a route graph's notices: `request` is the JSON of a [`RouteNoticesRequest`]; the
/// answer is the JSON list of `Notice`s.
///
/// # Errors
///
/// The JSON of a [`HostError`]: an unreadable request or an invalid graph.
#[wasm_bindgen(js_name = routeNotices)]
pub fn route_notices(request: &str) -> Result<String, String> {
    Ok(json(&route_notices_of(&read(
        "route notices request",
        request,
    )?)?))
}
