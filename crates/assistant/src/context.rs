//! What a turn leads its user message with: the essentials of its target, not a full read.
//! The assistant reads the rest with its tools when the message needs it.

use cairn_schema::{JourneyId, Revision, Route, RouteId, Snapshot, Title, VersionNumber};
use cairn_service::Projected;
use serde::Serialize;

// PRD: I5.
/// A journey as a turn opens: which it is, how current the read is, and its snapshot (its
/// facts and counts), which is already the essentials.
#[derive(Debug, Serialize)]
pub(crate) struct JourneyEssentials {
    pub journey_id: JourneyId,
    /// The base of a write that follows, until a tool answer names a newer one.
    pub revision: Revision,
    #[serde(flatten)]
    pub snapshot: Snapshot,
}

/// A route's draft as a turn opens.
#[derive(Debug, Serialize)]
pub(crate) struct RouteDraftEssentials {
    pub route: RouteId,
    pub name: Title,
    /// The route revision: the base of a route patch.
    pub revision: Revision,
    /// The published versions, newest first, in all.
    pub version_count: u32,
    /// The version the draft extends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<VersionNumber>,
    /// Nodes in the draft; absent when no draft is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_node_count: Option<u32>,
}

impl JourneyEssentials {
    /// The essentials of `journey`'s snapshot.
    pub(crate) fn of(journey: &JourneyId, read: Projected<Snapshot>) -> Self {
        Self {
            journey_id: journey.clone(),
            revision: read.revision,
            snapshot: read.value,
        }
    }
}

impl RouteDraftEssentials {
    /// The essentials of `route`, with its draft's node count when one is open.
    pub(crate) fn of(route: Route) -> Self {
        let version_count = u32::try_from(route.versions.len()).unwrap_or(u32::MAX);
        Self {
            route: route.header.id,
            name: route.header.name,
            revision: route.revision,
            version_count,
            extends: route.draft.as_ref().and_then(|draft| draft.extends),
            draft_node_count: route
                .draft
                .map(|draft| u32::try_from(draft.graph.nodes.len()).unwrap_or(u32::MAX)),
        }
    }
}
