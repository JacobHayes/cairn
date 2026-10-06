//! Every endpoint the API serves, by method and path (ARCHITECTURE, HTTP API). The router,
//! the OpenAPI document, and the Rust client all read these, so a path is spelled once.
//! Paths use axum's and OpenAPI's `{name}` placeholders.

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
    }
}

/// `GET /capabilities`: what this host offers.
pub static CAPABILITIES: Endpoint = endpoint(Method::GET, "/capabilities", "getCapabilities");
/// `POST /journeys/{id}/patches`: a journey patch, creating it at base revision 0 (A17).
pub static PATCH_JOURNEY: Endpoint =
    endpoint(Method::POST, "/journeys/{id}/patches", "patchJourney");
/// `POST /routes/{id}/patches`: a route patch, its draft included.
pub static PATCH_ROUTE: Endpoint = endpoint(Method::POST, "/routes/{id}/patches", "patchRoute");
/// `POST /deployment/patches`: a deployment patch (entities, merges).
pub static PATCH_DEPLOYMENT: Endpoint =
    endpoint(Method::POST, "/deployment/patches", "patchDeployment");
/// `GET /journeys`: the journey index (C16).
pub static JOURNEYS: Endpoint = endpoint(Method::GET, "/journeys", "listJourneys");
/// `GET /journeys/{id}`: a journey with its graph and state.
pub static JOURNEY: Endpoint = endpoint(Method::GET, "/journeys/{id}", "getJourney");
/// `GET /journeys/{id}/document`: the domain document the browser derives.
pub static DOCUMENT: Endpoint =
    endpoint(Method::GET, "/journeys/{id}/document", "getJourneyDocument");
/// `GET /routes/{id}`: a route with its draft.
pub static ROUTE: Endpoint = endpoint(Method::GET, "/routes/{id}", "getRoute");
/// `GET /routes/{id}/versions`: route detail, its versions and the journeys on each (C17).
pub static ROUTE_VERSIONS: Endpoint =
    endpoint(Method::GET, "/routes/{id}/versions", "getRouteDetail");
/// `GET /routes/{id}/versions/{version}`: one published version.
pub static ROUTE_VERSION: Endpoint = endpoint(
    Method::GET,
    "/routes/{id}/versions/{version}",
    "getRouteVersion",
);
/// `GET /deployment`: entities and aliases at the deployment revision.
pub static DEPLOYMENT: Endpoint = endpoint(Method::GET, "/deployment", "getDeployment");
/// `GET /entities/{key}`: an entity, through its alias when merged away (E6).
pub static ENTITY: Endpoint = endpoint(Method::GET, "/entities/{key}", "getEntity");
/// `GET /search`: text search across journeys.
pub static SEARCH: Endpoint = endpoint(Method::GET, "/search", "searchJourneys");
/// `GET /events`: the event history, filtered and paged (J5).
pub static EVENTS: Endpoint = endpoint(Method::GET, "/events", "listEvents");
/// `GET /events/stream`: revision ticks over SSE, current revisions first (H6).
pub static STREAM: Endpoint = endpoint(Method::GET, "/events/stream", "streamRevisions");
/// `GET /users/me`: the caller and their entities (H3).
pub static VIEWER: Endpoint = endpoint(Method::GET, "/users/me", "getViewer");
/// `GET /users/me/tokens`: the caller's agent tokens.
pub static TOKENS: Endpoint = endpoint(Method::GET, "/users/me/tokens", "listAgentTokens");
/// `POST /users/me/tokens`: mints an agent token (H2).
pub static MINT_TOKEN: Endpoint = endpoint(Method::POST, "/users/me/tokens", "mintAgentToken");
/// `DELETE /users/me/tokens/{agent}`: revokes one.
pub static REVOKE_TOKEN: Endpoint = endpoint(
    Method::DELETE,
    "/users/me/tokens/{agent}",
    "revokeAgentToken",
);

/// Every endpoint, in the order the OpenAPI document lists them.
pub static ALL: [&Endpoint; 19] = [
    &CAPABILITIES,
    &PATCH_JOURNEY,
    &PATCH_ROUTE,
    &PATCH_DEPLOYMENT,
    &JOURNEYS,
    &JOURNEY,
    &DOCUMENT,
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
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_fill_their_placeholders_in_order_and_encode_them() {
        assert_eq!(
            ROUTE_VERSION.path_with(&["r_a", "2"]),
            "/routes/r_a/versions/2"
        );
        assert_eq!(
            PATCH_JOURNEY.path_with(&["a b/c+"]),
            "/journeys/a%20b%2Fc%2B/patches"
        );
        assert_eq!(CAPABILITIES.path_with(&[]), "/capabilities");
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
