//! The built-in OAuth provider (ARCHITECTURE, Auth): Cairn's own authorization server, so
//! an MCP client connects with a sign-in click and no identity-provider setup (I2). It
//! serves protected resource metadata (RFC 9728) and authorization server metadata (RFC
//! 8414), dynamic client registration (RFC 7591), the authorization-code flow with PKCE
//! S256 only, behind a consent page, and a token endpoint that mints an agent token for
//! the client. A browser without a session is sent to sign in with the provider the
//! configuration names (federating sign-in to OIDC). No scopes: a token is an identity.
//!
//! The same provider accepts the agent tokens Cairn mints, from this flow or from the UI
//! for scripts, as bearer tokens.

pub mod client;
mod flow;

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::response::Json;
use axum::routing::{get, post};
use cairn_schema::Slug;
use cairn_store::AuthStore;
use serde_json::{Value, json};
use url::Url;

use crate::accounts::Accounts;
use crate::error::Refusal;
use crate::layer::Authenticate;
use crate::provider::{AuthProvider, BoxFuture, Presented, Verdict};
use crate::secret::SECRET_PREFIX;

/// The built-in OAuth provider's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthConfig {
    /// The provider's name.
    pub name: Slug,
    /// The provider a browser without a session signs in with before it is asked to allow
    /// a client: usually the OIDC provider.
    pub sign_in_with: Option<Slug>,
}

/// The built-in OAuth provider.
pub struct OAuthServer<S> {
    inner: Arc<Server<S>>,
}

struct Server<S> {
    config: OAuthConfig,
    accounts: Accounts<S>,
    /// The issuer identifier: the public base URL without its trailing slash.
    issuer: String,
}

impl<S: AuthStore> OAuthServer<S> {
    /// The provider for a deployment at the public `base` URL.
    #[must_use]
    pub fn new(config: OAuthConfig, accounts: Accounts<S>, base: &Url) -> Self {
        let issuer = base.as_str().trim_end_matches('/').to_owned();
        let inner = Server {
            config,
            accounts,
            issuer,
        };
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl<S: AuthStore> Server<S> {
    /// H2: a bearer token Cairn issued names its agent and user, or is refused.
    async fn verdict(&self, request: Presented<'_>) -> Verdict {
        let claimed = request
            .bearer()
            .filter(|token| token.starts_with(SECRET_PREFIX));
        let Some(token) = claimed else {
            return Verdict::Absent;
        };
        match self.accounts.token_actor(token).await {
            Ok(Some(actor)) => Verdict::Actor(actor),
            Ok(None) => Verdict::Refused(Refusal::Credential),
            Err(error) => Verdict::Refused(error.into()),
        }
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.issuer)
    }
}

impl<S: AuthStore + 'static> AuthProvider for OAuthServer<S> {
    fn name(&self) -> &Slug {
        &self.inner.config.name
    }

    fn authenticate<'a>(&'a self, request: Presented<'a>) -> BoxFuture<'a, Verdict> {
        Box::pin(self.inner.verdict(request))
    }

    /// Every bearer token with Cairn's prefix is this provider's: a session token or a code
    /// presented as one is refused, not passed on.
    fn claims_bearer(&self, token: &str) -> bool {
        token.starts_with(SECRET_PREFIX)
    }

    /// I2, RFC 9728: where an MCP client finds how to get a token for `path`; the root is
    /// the deployment itself, served without a suffix.
    fn challenge(&self, path: &str) -> Option<String> {
        let metadata = self.inner.endpoint("/.well-known/oauth-protected-resource");
        let suffix = if path == "/" { "" } else { path };
        Some(format!("resource_metadata=\"{metadata}{suffix}\""))
    }

    fn router(&self, authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        let state = flow::Flow {
            server: Arc::clone(&self.inner),
            authenticator,
        };
        let router = Router::new()
            .route(
                "/.well-known/oauth-protected-resource",
                get(resource_root::<S>),
            )
            .route(
                "/.well-known/oauth-protected-resource/{*resource}",
                get(resource::<S>),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(metadata::<S>),
            )
            .route("/api/oauth/register", post(flow::register))
            .route(
                "/api/oauth/authorize",
                get(flow::authorize::<S>).post(flow::consent::<S>),
            )
            .route("/api/oauth/token", post(flow::token::<S>))
            .with_state(Arc::new(state));
        Some(router)
    }
}

/// RFC 9728: the deployment as one protected resource, served by this server.
async fn resource_root<S: AuthStore>(State(flow): State<Arc<flow::Flow<S>>>) -> Json<Value> {
    resource_metadata(&flow.server, &flow.server.issuer)
}

/// RFC 9728's path-suffixed form: the resource at `resource` under the deployment.
async fn resource<S: AuthStore>(
    State(flow): State<Arc<flow::Flow<S>>>,
    Path(resource): Path<String>,
) -> Json<Value> {
    resource_metadata(&flow.server, &flow.server.endpoint(&format!("/{resource}")))
}

fn resource_metadata<S>(server: &Server<S>, resource: &str) -> Json<Value> {
    Json(json!({
        "resource": resource,
        "authorization_servers": [server.issuer],
        "bearer_methods_supported": ["header"],
    }))
}

/// RFC 8414: what this server supports, which is one flow.
async fn metadata<S: AuthStore>(State(flow): State<Arc<flow::Flow<S>>>) -> Json<Value> {
    let server = &flow.server;
    Json(json!({
        "issuer": server.issuer,
        "authorization_endpoint": server.endpoint("/api/oauth/authorize"),
        "token_endpoint": server.endpoint("/api/oauth/token"),
        "registration_endpoint": server.endpoint("/api/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "authorization_response_iss_parameter_supported": true,
    }))
}
