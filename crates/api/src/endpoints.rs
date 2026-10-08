//! Every endpoint the API serves, by method and path (ARCHITECTURE, HTTP API). The router,
//! the OpenAPI document, and the Rust client all read these, so a path is spelled once.
//! Paths use axum's and OpenAPI's `{name}` placeholders. Every endpoint but the health check
//! sits under [`PREFIX`], so the rest of the origin is the web app's
//! (decisions/2026-10-08-the-api-is-served-under-api-and-the-app-owns-the-rest.md).

use axum::http::Method;

/// One endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// The method.
    pub method: Method,
    /// The path, with `{name}` placeholders.
    pub path: &'static str,
    /// The OpenAPI operation id.
    pub operation: &'static str,
    /// Served outside the auth layer, to anyone: the health check alone.
    pub public: bool,
}

impl Endpoint {
    /// The path with each placeholder replaced, in order, by a percent-encoded segment.
    ///
    /// # Panics
    ///
    /// When the number of segments is not the number of placeholders.
    #[must_use]
    pub fn path_with(&self, segments: &[&str]) -> String {
        let mut path = String::with_capacity(self.path.len());
        let mut remaining = segments.iter();
        for part in self.path.split('/').skip(1) {
            path.push('/');
            if part.starts_with('{') && part.ends_with('}') {
                let segment = remaining.next();
                assert!(
                    segment.is_some(),
                    "{}: a segment per placeholder",
                    self.path
                );
                encode_segment(segment.map_or("", |text| text), &mut path);
            } else {
                path.push_str(part);
            }
        }
        assert!(
            remaining.next().is_none(),
            "{}: no extra segments",
            self.path
        );
        path
    }
}

/// The path every endpoint but [`HEALTH`] sits under, MCP's and auth's routes too: the
/// server's part of the origin. The web app owns every other path but `/.well-known/` and
/// [`HEALTH`]'s, which standards and the deployment's probes fix.
pub const PREFIX: &str = "/api";

/// The path prefixes the server keeps whole: [`PREFIX`], and `/.well-known`, whose paths
/// RFC 8615 fixes (OAuth's metadata). A path under one that no endpoint serves is answered
/// as no endpoint, never with the app's page; with [`HEALTH`]'s path they are everything on
/// the origin that is not the app's.
pub const RESERVED: [&str; 2] = [PREFIX, "/.well-known"];

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Percent-encodes everything in `segment` but RFC 3986's unreserved characters.
fn encode_segment(segment: &str, into: &mut String) {
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            into.push(char::from(byte));
        } else {
            into.push('%');
            into.push(char::from(HEX[usize::from(byte >> 4)]));
            into.push(char::from(HEX[usize::from(byte & 0xF)]));
        }
    }
}

const fn endpoint(method: Method, path: &'static str, operation: &'static str) -> Endpoint {
    Endpoint {
        method,
        path,
        operation,
        public: false,
    }
}

/// `GET /healthz`: whether the server is serving, answered without credentials so a proxy's
/// health check can ask it.
pub static HEALTH: Endpoint = Endpoint {
    method: Method::GET,
    path: "/healthz",
    operation: "getHealth",
    public: true,
};
/// `GET /api/capabilities`: what this host offers.
pub static CAPABILITIES: Endpoint = endpoint(Method::GET, "/api/capabilities", "getCapabilities");
/// `POST /api/journeys/{id}/patches`: a journey patch, creating it at base revision 0 (A17).
pub static PATCH_JOURNEY: Endpoint =
    endpoint(Method::POST, "/api/journeys/{id}/patches", "patchJourney");
/// `POST /api/routes/{id}/patches`: a route patch, its draft included.
pub static PATCH_ROUTE: Endpoint = endpoint(Method::POST, "/api/routes/{id}/patches", "patchRoute");
/// `POST /api/deployment/patches`: a deployment patch (entities, merges).
pub static PATCH_DEPLOYMENT: Endpoint =
    endpoint(Method::POST, "/api/deployment/patches", "patchDeployment");
/// `POST /api/journeys/{id}/proposals`: a proposal for a journey, which may not exist yet (I6).
pub static PROPOSE_JOURNEY: Endpoint = endpoint(
    Method::POST,
    "/api/journeys/{id}/proposals",
    "proposeToJourney",
);
/// `POST /api/routes/{id}/proposals`: a proposal for a route, which may not exist yet (I6).
pub static PROPOSE_ROUTE: Endpoint =
    endpoint(Method::POST, "/api/routes/{id}/proposals", "proposeToRoute");
/// `POST /api/deployment/proposals`: a proposal for the deployment (I6).
pub static PROPOSE_DEPLOYMENT: Endpoint = endpoint(
    Method::POST,
    "/api/deployment/proposals",
    "proposeToDeployment",
);
/// `GET /api/proposals/{id}`: a proposal by its client-generated id (I6).
pub static PROPOSAL: Endpoint = endpoint(Method::GET, "/api/proposals/{id}", "getProposal");
/// `PATCH /api/proposals/{id}`: replaces a proposal's content against its editing revision.
pub static EDIT_PROPOSAL: Endpoint = endpoint(Method::PATCH, "/api/proposals/{id}", "editProposal");
/// `POST /api/proposals/{id}/preview`: what applying it now would do (C14, D7).
pub static PREVIEW_PROPOSAL: Endpoint = endpoint(
    Method::POST,
    "/api/proposals/{id}/preview",
    "previewProposal",
);
/// `POST /api/proposals/{id}/apply`: applies it, the caller confirming (H2, I6).
pub static APPLY_PROPOSAL: Endpoint =
    endpoint(Method::POST, "/api/proposals/{id}/apply", "applyProposal");
/// `POST /api/proposals/{id}/discard`: discards it.
pub static DISCARD_PROPOSAL: Endpoint = endpoint(
    Method::POST,
    "/api/proposals/{id}/discard",
    "discardProposal",
);
/// `POST /api/proposals/{id}/refresh`: drafts it again against its destination as it stands (I6).
pub static REFRESH_PROPOSAL: Endpoint = endpoint(
    Method::POST,
    "/api/proposals/{id}/refresh",
    "refreshProposal",
);
/// `POST /api/journeys/{id}/upgrade`: proposes upgrading it to a newer route version (B7).
pub static UPGRADE: Endpoint =
    endpoint(Method::POST, "/api/journeys/{id}/upgrade", "proposeUpgrade");
/// `POST /api/journeys/{id}/save-as-route`: proposes saving its structure as a route draft (B8).
pub static SAVE_AS_ROUTE: Endpoint = endpoint(
    Method::POST,
    "/api/journeys/{id}/save-as-route",
    "proposeSaveAsRoute",
);
/// `POST /api/journeys/{id}/relink`: proposes re-linking it to a published version (B9).
pub static RELINK: Endpoint = endpoint(Method::POST, "/api/journeys/{id}/relink", "proposeRelink");
/// `POST /api/routes/{id}/import`: imports a route file as a new route or draft (A13).
pub static IMPORT_ROUTE: Endpoint =
    endpoint(Method::POST, "/api/routes/{id}/import", "importRoute");
/// `GET /api/routes/{id}/export`: a version or the draft as a route file (A13).
pub static EXPORT_ROUTE: Endpoint = endpoint(Method::GET, "/api/routes/{id}/export", "exportRoute");
/// `GET /api/journeys`: the journey index (C16).
pub static JOURNEYS: Endpoint = endpoint(Method::GET, "/api/journeys", "listJourneys");
/// `GET /api/journeys/{id}`: a journey with its graph and state.
pub static JOURNEY: Endpoint = endpoint(Method::GET, "/api/journeys/{id}", "getJourney");
/// `GET /api/journeys/{id}/document`: the domain document the browser derives.
pub static DOCUMENT: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/document",
    "getJourneyDocument",
);
/// `GET /api/journeys/{id}/snapshot`: the bounded agent snapshot, scoped and paged (I3).
pub static SNAPSHOT: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/snapshot",
    "getJourneySnapshot",
);
/// `GET /api/journeys/{id}/level`: one canvas level (C2).
pub static LEVEL: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/level", "getJourneyLevel");
/// `GET /api/journeys/{id}/trace/{key}`: what is upstream and downstream of a node (C7).
pub static TRACE: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/trace/{key}", "traceNode");
/// `GET /api/journeys/{id}/derived`: every derived value (D3), as the browser derives it.
pub static DERIVED: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/derived",
    "getJourneyDerived",
);
/// `GET /api/journeys/{id}/decisions`: the decision view (C12).
pub static DECISIONS: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/decisions",
    "getDecisionView",
);
/// `GET /api/journeys/{id}/timeline`: the timeline (C13).
pub static TIMELINE: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/timeline", "getTimeline");
/// `GET /api/journeys/{id}/summary`: the status summary (C18).
pub static SUMMARY: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/summary",
    "getStatusSummary",
);
/// `GET /api/journeys/{id}/next`: the ranked acting frontier (C10).
pub static NEXT: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/next", "getNext");
/// `GET /api/journeys/{id}/nodes`: the nodes a list query matches, paged (C9).
pub static NODES: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/nodes", "listNodes");
/// `GET /api/journeys/{id}/mine`: the nodes the caller participates in (E4).
pub static MINE: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/mine", "getMine");
/// `GET /api/journeys/{id}/nodes/{key}`: one node in full, explanations capped (C8).
pub static NODE: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/nodes/{key}", "getNode");
/// `GET /api/journeys/{id}/nodes/{key}/explanations/{field}`: a page of one explanation list.
pub static EXPLANATIONS: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/nodes/{key}/explanations/{field}",
    "listExplanations",
);
/// `GET /api/journeys/{id}/history`: the journey's events, or a node's, grouped by patch (J4).
pub static HISTORY: Endpoint = endpoint(Method::GET, "/api/journeys/{id}/history", "getHistory");
/// `GET /api/routes`: the route index (I2).
pub static ROUTES: Endpoint = endpoint(Method::GET, "/api/routes", "listRoutes");
/// `GET /api/routes/{id}`: a route with its draft.
pub static ROUTE: Endpoint = endpoint(Method::GET, "/api/routes/{id}", "getRoute");
/// `GET /api/routes/{id}/versions`: route detail, its versions and the journeys on each (C17).
pub static ROUTE_VERSIONS: Endpoint =
    endpoint(Method::GET, "/api/routes/{id}/versions", "getRouteDetail");
/// `GET /api/routes/{id}/versions/{version}`: one published version.
pub static ROUTE_VERSION: Endpoint = endpoint(
    Method::GET,
    "/api/routes/{id}/versions/{version}",
    "getRouteVersion",
);
/// `GET /api/deployment`: entities and aliases at the deployment revision.
pub static DEPLOYMENT: Endpoint = endpoint(Method::GET, "/api/deployment", "getDeployment");
/// `GET /api/entities/{key}`: an entity, through its alias when merged away (E6).
pub static ENTITY: Endpoint = endpoint(Method::GET, "/api/entities/{key}", "getEntity");
/// `GET /api/search`: text search across journeys.
pub static SEARCH: Endpoint = endpoint(Method::GET, "/api/search", "searchJourneys");
/// `GET /api/events`: the event history, filtered and paged (J5).
pub static EVENTS: Endpoint = endpoint(Method::GET, "/api/events", "listEvents");
/// `GET /api/events/stream`: revision ticks over SSE, current revisions first (H6).
pub static STREAM: Endpoint = endpoint(Method::GET, "/api/events/stream", "streamRevisions");
/// `GET /api/users/me`: the caller and their entities (H3).
pub static VIEWER: Endpoint = endpoint(Method::GET, "/api/users/me", "getViewer");
/// `GET /api/users/me/tokens`: the caller's agent tokens.
pub static TOKENS: Endpoint = endpoint(Method::GET, "/api/users/me/tokens", "listAgentTokens");
/// `POST /api/users/me/tokens`: mints an agent token (H2).
pub static MINT_TOKEN: Endpoint = endpoint(Method::POST, "/api/users/me/tokens", "mintAgentToken");
/// `DELETE /api/users/me/tokens/{agent}`: revokes one.
pub static REVOKE_TOKEN: Endpoint = endpoint(
    Method::DELETE,
    "/api/users/me/tokens/{agent}",
    "revokeAgentToken",
);

/// `POST /api/journeys/{id}/assistant`: one assistant turn about a journey (I5), when the host
/// offers the assistant.
pub static ASSISTANT_JOURNEY: Endpoint = endpoint(
    Method::POST,
    "/api/journeys/{id}/assistant",
    "converseAboutJourney",
);
/// `POST /api/routes/{id}/draft/assistant`: one assistant turn about a route's draft (I5, A12),
/// when the host offers the assistant.
pub static ASSISTANT_ROUTE_DRAFT: Endpoint = endpoint(
    Method::POST,
    "/api/routes/{id}/draft/assistant",
    "converseAboutRouteDraft",
);
/// `GET /api/journeys/{id}/assistant`: the caller's conversation about a journey (I5), when the
/// host offers the assistant.
pub static ASSISTANT_JOURNEY_CONVERSATION: Endpoint = endpoint(
    Method::GET,
    "/api/journeys/{id}/assistant",
    "conversationAboutJourney",
);
/// `GET /api/routes/{id}/draft/assistant`: the caller's conversation about a route's draft (I5),
/// when the host offers the assistant.
pub static ASSISTANT_ROUTE_DRAFT_CONVERSATION: Endpoint = endpoint(
    Method::GET,
    "/api/routes/{id}/draft/assistant",
    "conversationAboutRouteDraft",
);

/// Every endpoint, in the order the OpenAPI document lists them.
pub static ALL: [&Endpoint; 52] = [
    &HEALTH,
    &CAPABILITIES,
    &PATCH_JOURNEY,
    &PATCH_ROUTE,
    &PATCH_DEPLOYMENT,
    &PROPOSE_JOURNEY,
    &PROPOSE_ROUTE,
    &PROPOSE_DEPLOYMENT,
    &PROPOSAL,
    &EDIT_PROPOSAL,
    &PREVIEW_PROPOSAL,
    &APPLY_PROPOSAL,
    &DISCARD_PROPOSAL,
    &REFRESH_PROPOSAL,
    &UPGRADE,
    &SAVE_AS_ROUTE,
    &RELINK,
    &IMPORT_ROUTE,
    &EXPORT_ROUTE,
    &JOURNEYS,
    &JOURNEY,
    &DOCUMENT,
    &SNAPSHOT,
    &LEVEL,
    &TRACE,
    &DERIVED,
    &DECISIONS,
    &TIMELINE,
    &SUMMARY,
    &NEXT,
    &NODES,
    &MINE,
    &NODE,
    &EXPLANATIONS,
    &HISTORY,
    &ROUTES,
    &ROUTE,
    &ROUTE_VERSIONS,
    &ROUTE_VERSION,
    &DEPLOYMENT,
    &ENTITY,
    &SEARCH,
    &EVENTS,
    &STREAM,
    &VIEWER,
    &TOKENS,
    &MINT_TOKEN,
    &REVOKE_TOKEN,
    &ASSISTANT_JOURNEY,
    &ASSISTANT_ROUTE_DRAFT,
    &ASSISTANT_JOURNEY_CONVERSATION,
    &ASSISTANT_ROUTE_DRAFT_CONVERSATION,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_fill_their_placeholders_in_order_and_encode_them() {
        assert_eq!(
            ROUTE_VERSION.path_with(&["r_a", "2"]),
            "/api/routes/r_a/versions/2"
        );
        assert_eq!(
            PATCH_JOURNEY.path_with(&["a b/c+"]),
            "/api/journeys/a%20b%2Fc%2B/patches"
        );
        assert_eq!(CAPABILITIES.path_with(&[]), "/api/capabilities");
    }

    #[test]
    fn every_endpoint_but_the_health_check_is_under_the_prefix() {
        for endpoint in ALL {
            let under = endpoint
                .path
                .strip_prefix(PREFIX)
                .is_some_and(|rest| rest.starts_with('/'));
            assert_eq!(under, !endpoint.public, "{}", endpoint.path);
        }
    }

    #[test]
    fn operations_and_routes_are_unique() {
        let operations: std::collections::BTreeSet<_> =
            ALL.iter().map(|endpoint| endpoint.operation).collect();
        let routes: std::collections::BTreeSet<_> = ALL
            .iter()
            .map(|endpoint| (endpoint.method.as_str(), endpoint.path))
            .collect();
        assert_eq!(operations.len(), ALL.len());
        assert_eq!(routes.len(), ALL.len());
    }
}
