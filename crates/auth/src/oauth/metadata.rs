//! Where an MCP client finds how to get a token (I2), the same for both OAuth servers:
//! protected resource metadata (RFC 9728), authorization server metadata (RFC 8414), and
//! the endpoints' paths. One server holds them in a deployment: the origin's well-known
//! paths can name only one.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::routing::get;
use serde_json::{Value, json};
use url::Url;

use super::client::Policy;

/// RFC 9728's well-known path, unsuffixed for the deployment as a whole.
pub(crate) const PROTECTED_RESOURCE: &str = "/.well-known/oauth-protected-resource";
/// RFC 8414's well-known path.
pub(crate) const AUTHORIZATION_SERVER: &str = "/.well-known/oauth-authorization-server";
/// Dynamic client registration.
pub(crate) const REGISTER: &str = "/api/oauth/register";
/// The authorization endpoint.
pub(crate) const AUTHORIZE: &str = "/api/oauth/authorize";
/// The token endpoint.
pub(crate) const TOKEN: &str = "/api/oauth/token";

/// A server's metadata: its issuer and what it offers.
#[derive(Debug)]
pub(crate) struct Metadata {
    /// The issuer identifier: the public base URL without its trailing slash.
    issuer: String,
    policy: &'static Policy,
}

impl Metadata {
    /// The metadata of a server at the public `base` URL registering under `policy`.
    pub(crate) fn new(base: &Url, policy: &'static Policy) -> Self {
        let issuer = base.as_str().trim_end_matches('/').to_owned();
        Self { issuer, policy }
    }

    /// The issuer: what every authorization response names as `iss` (RFC 9207).
    pub(crate) fn issuer(&self) -> &str {
        &self.issuer
    }

    /// RFC 8707: a resource is this deployment (any path on its origin, as the protected
    /// resource metadata serves every path), never a query or a fragment.
    pub(crate) fn is_own_resource(&self, resource: &str) -> bool {
        let own = Url::parse(&self.issuer).ok().map(|url| url.origin());
        Url::parse(resource).is_ok_and(|url| {
            Some(url.origin()) == own && url.query().is_none() && url.fragment().is_none()
        })
    }

    /// The absolute URL of `path` on this deployment.
    pub(crate) fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.issuer)
    }

    /// RFC 9728's challenge parameter: where a client finds how to get a token for
    /// `path`; the root is the deployment itself, served without a suffix.
    pub(crate) fn challenge(&self, path: &str) -> String {
        let suffix = if path == "/" { "" } else { path };
        format!(
            "resource_metadata=\"{}{suffix}\"",
            self.endpoint(PROTECTED_RESOURCE)
        )
    }

    /// The well-known routes.
    pub(crate) fn router(self: &Arc<Self>) -> Router {
        let suffixed = format!("{PROTECTED_RESOURCE}/{{*resource}}");
        Router::new()
            .route(PROTECTED_RESOURCE, get(resource_root))
            .route(&suffixed, get(resource))
            .route(AUTHORIZATION_SERVER, get(server))
            .with_state(Arc::clone(self))
    }

    fn resource(&self, resource: &str) -> Json<Value> {
        Json(json!({
            "resource": resource,
            "authorization_servers": [self.issuer],
            "bearer_methods_supported": ["header"],
        }))
    }
}

/// RFC 9728: the deployment as one protected resource.
async fn resource_root(State(metadata): State<Arc<Metadata>>) -> Json<Value> {
    metadata.resource(&metadata.issuer)
}

/// RFC 9728's path-suffixed form: the resource at `resource` under the deployment.
async fn resource(
    State(metadata): State<Arc<Metadata>>,
    Path(resource): Path<String>,
) -> Json<Value> {
    metadata.resource(&metadata.endpoint(&format!("/{resource}")))
}

/// RFC 8414: what this server supports, which is one flow and its grants.
async fn server(State(metadata): State<Arc<Metadata>>) -> Json<Value> {
    Json(json!({
        "issuer": metadata.issuer,
        "authorization_endpoint": metadata.endpoint(AUTHORIZE),
        "token_endpoint": metadata.endpoint(TOKEN),
        "registration_endpoint": metadata.endpoint(REGISTER),
        "response_types_supported": ["code"],
        "grant_types_supported": metadata.policy.grant_types,
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "authorization_response_iss_parameter_supported": true,
    }))
}
