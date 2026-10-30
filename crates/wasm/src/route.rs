//! A route's graph on the canvas: a route draft or a published version is a graph with no
//! state, so it is derived once as a journey that starts on day 0 and in which nothing has
//! happened ([`RouteDerivation`]), and every projection runs on that one derivation as on a
//! journey's: its canvas level (C2), its trace (C7) and its schedule (C13). The rules for what
//! is visible, what rolls up, which edges are drawn and what a node traces are the engine's, as
//! on a journey; the canvas draws the structure and its trace, and none of the state.
//!
//! Cost: the engine's derive of the graph (ARCHITECTURE, Read path) once, then each projection
//! costs what its engine module states at `node_count_max`.

use std::collections::BTreeSet;

use cairn_engine::{Derived, DerivedJourney, Graph, derive, notices};
use cairn_schema::{
    Date, Deployment, DeriveInputs, Graph as GraphDocument, Notice, RouteSchedule, Timestamp,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::derivation::{Projection, projected};
use crate::error::{HostError, json, read};

/// The day a route's journey starts on: any day will do, because only the distance from it is
/// read, and no calendar date leaves the derivation.
const SCHEDULE_START: Date = Date::constant(2000, 1, 1);

/// C13: a route's graph to derive, with the deployment its conditions read identities from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDeriveRequest {
    /// The route's graph.
    pub graph: GraphDocument,
    /// The deployment context (E6).
    pub deployment: Deployment,
}

/// A route's graph validated and derived as a journey that starts on day 0 and in which nothing
/// has happened (C2, C7, C13): what every projection of a route's canvas, list and timeline
/// reads.
#[wasm_bindgen]
#[derive(Debug)]
pub struct RouteDerivation {
    graph: Graph,
    derived: Derived,
    deployment: Deployment,
}

impl RouteDerivation {
    /// Derives the route's graph.
    ///
    /// # Errors
    ///
    /// [`HostError::Invalid`] when the graph breaks an invariant, which a stored route never
    /// does.
    pub fn of(request: &RouteDeriveRequest) -> Result<Self, HostError> {
        let graph = Graph::new(request.graph.clone(), &request.deployment)
            .map_err(|violations| HostError::Invalid { violations })?;
        let inputs = DeriveInputs {
            today: SCHEDULE_START,
            now: Timestamp::UNIX_EPOCH,
            timezone: "UTC"
                .parse()
                .map_err(|error| HostError::unreadable("timezone", error))?,
            viewer: BTreeSet::new(),
            deployment: request.deployment.clone(),
        };
        let derived = derive(&graph, Some(SCHEDULE_START), &inputs);
        Ok(Self {
            graph,
            derived,
            deployment: request.deployment.clone(),
        })
    }

    /// The projection `request`, as a journey's derivation answers it, for a viewer who
    /// participates in nothing.
    ///
    /// # Errors
    ///
    /// [`HostError::Missing`] when the request names a node the graph does not hold.
    pub fn projected(&self, request: &Projection) -> Result<String, HostError> {
        projected(
            &DerivedJourney::new(&self.graph, &self.derived),
            None,
            &self.deployment,
            &BTreeSet::new(),
            request,
        )
    }

    /// C13: the earliest day each node can start and finish if a journey of it starts on day 0.
    #[must_use]
    pub fn scheduled(&self) -> RouteSchedule {
        DerivedJourney::new(&self.graph, &self.derived).schedule(SCHEDULE_START)
    }
}

#[wasm_bindgen]
impl RouteDerivation {
    /// Reads a [`RouteDeriveRequest`] and derives it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: an unreadable request or an invalid graph.
    #[wasm_bindgen(constructor)]
    pub fn new(request: &str) -> Result<RouteDerivation, String> {
        Ok(Self::of(&read("route derive request", request)?)?)
    }

    /// One projection: `request` is the JSON of a [`Projection`].
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: an unreadable request, or a node the graph lacks.
    pub fn project(&self, request: &str) -> Result<String, String> {
        Ok(self.projected(&read("projection", request)?)?)
    }

    /// C13: the schedule, as the JSON of a `RouteSchedule`.
    #[must_use]
    pub fn schedule(&self) -> String {
        json(&self.scheduled())
    }
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
