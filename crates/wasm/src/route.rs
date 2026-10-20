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
    Date, Deployment, DeriveInputs, Graph as GraphDocument, Level, LevelQuery, Notice,
    RankConstants, RouteSchedule,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};

/// A route's graph validated and derived as a journey in which nothing has happened yet,
/// created and read on `day`.
fn route_derived(
    graph: &GraphDocument,
    deployment: &Deployment,
    day: Date,
) -> Result<(Graph, cairn_engine::Derived), HostError> {
    let graph = Graph::new(graph.clone(), deployment)
        .map_err(|violations| HostError::Invalid { violations })?;
    let inputs = DeriveInputs {
        today: day,
        timezone: "UTC"
            .parse()
            .map_err(|error| HostError::unreadable("timezone", error))?,
        rank: RankConstants::default(),
        viewer: BTreeSet::new(),
        deployment: deployment.clone(),
    };
    let derived = derive(&graph, Some(day), &inputs);
    Ok((graph, derived))
}

/// C2: one canvas level of a route's graph (its draft or a version), with the deployment its
/// conditions read identities from and the day its date rules are read on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteLevelRequest {
    /// The route's graph.
    pub graph: GraphDocument,
    /// The deployment context (E6).
    pub deployment: Deployment,
    /// Today, which nothing the level answers depends on but the derive reads.
    pub today: Date,
    /// The level asked for: the same query a journey's level takes (kinds, collapsed
    /// containers, relevance classes, text, the selected node).
    pub query: LevelQuery,
}

/// C2: the level of a route's graph for `query`.
///
/// # Errors
///
/// [`HostError::Invalid`] when the graph breaks an invariant, which a stored route never does;
/// [`HostError::Missing`] when the query names a node the graph lacks.
pub fn route_level_of(request: &RouteLevelRequest) -> Result<Level, HostError> {
    let (graph, derived) = route_derived(&request.graph, &request.deployment, request.today)?;
    DerivedJourney::new(&graph, &derived)
        .level(&request.query, &request.deployment, &BTreeSet::new())
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

/// The day a route's schedule counts from: any day will do, because only the distance from it
/// is read, and no calendar date leaves the function.
const SCHEDULE_START: Date = Date::constant(2000, 1, 1);

/// C13: a route's graph to schedule, with the deployment its conditions read identities from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteScheduleRequest {
    /// The route's graph.
    pub graph: GraphDocument,
    /// The deployment context (E6).
    pub deployment: Deployment,
}

/// C13: the earliest day each node of a route can start and finish if a journey of it starts
/// on day 0.
///
/// # Errors
///
/// [`HostError::Invalid`] when the graph breaks an invariant, which a stored route never does.
pub fn route_schedule_of(request: &RouteScheduleRequest) -> Result<RouteSchedule, HostError> {
    let (graph, derived) = route_derived(&request.graph, &request.deployment, SCHEDULE_START)?;
    Ok(DerivedJourney::new(&graph, &derived).schedule(SCHEDULE_START))
}

/// C13: a route's schedule: `request` is the JSON of a [`RouteScheduleRequest`]; the answer is
/// the JSON of a `RouteSchedule`.
///
/// # Errors
///
/// The JSON of a [`HostError`]: an unreadable request or an invalid graph.
#[wasm_bindgen(js_name = routeSchedule)]
pub fn route_schedule(request: &str) -> Result<String, String> {
    Ok(json(&route_schedule_of(&read(
        "route schedule request",
        request,
    )?)?))
}
