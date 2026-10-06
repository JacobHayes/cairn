//! What a load names and what it returns (ARCHITECTURE, Store trait: `load` by a typed
//! target). Loads and size caps are per graph, so a route's published versions never weigh
//! on its draft: a route loads with its draft, and each version loads alone.

use cairn_schema::{
    Deployment, Journey, JourneyId, Revision, Route, RouteId, RouteVersion, VersionNumber,
};

/// What to load.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoadTarget {
    /// A journey: its fields, graph, and state.
    Journey(JourneyId),
    /// A route: its fields, published version numbers, and draft.
    Route(RouteId),
    /// One published route version, immutable.
    RouteVersion {
        /// The route.
        route: RouteId,
        /// The version.
        version: VersionNumber,
    },
    /// The deployment: entities and aliases. Always exists, at revision 0 before any
    /// commit.
    Deployment,
}

/// A loaded document.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Document {
    /// A journey.
    Journey(Journey),
    /// A route with its draft.
    Route(Route),
    /// A published route version.
    RouteVersion(RouteVersion),
    /// The deployment.
    Deployment(Deployment),
}

impl Document {
    /// The document's revision (H5). A published version is written once and never moves,
    /// so its revision is always the first one.
    #[must_use]
    pub fn revision(&self) -> Revision {
        match self {
            Document::Journey(journey) => journey.revision,
            Document::Route(route) => route.revision,
            Document::RouteVersion(_) => Revision::NONE.next(),
            Document::Deployment(deployment) => deployment.revision,
        }
    }
}
