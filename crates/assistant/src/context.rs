//! What a turn leads its user message with: the essentials of its target, not a full read.
//! The assistant reads the rest with its tools when the message needs it.

use cairn_schema::{
    JourneyId, JourneyStatus, Revision, Route, RouteId, Snapshot, SnapshotCounts, Title,
    VersionNumber,
};
use cairn_service::Projected;
use serde::Serialize;

// PRD: I5.
/// A journey as a turn opens: who it is, how current the read is, and the snapshot's counts.
#[derive(Debug, Serialize)]
pub(crate) struct JourneyEssentials {
    pub journey: JourneyId,
    pub name: Title,
    pub status: JourneyStatus,
    /// The base of a write that follows, until a tool answer names a newer one.
    pub revision: Revision,
    /// Completing it is suggested: the person's call.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub completion_suggested: bool,
    pub counts: SnapshotCounts,
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
        let snapshot = read.value;
        Self {
            journey: journey.clone(),
            name: snapshot.journey.name,
            status: snapshot.journey.status,
            revision: read.revision,
            completion_suggested: snapshot.completion_suggested,
            counts: snapshot.counts,
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
