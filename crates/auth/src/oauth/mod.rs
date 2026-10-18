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
//!
//! The pieces the IAP relay shares with it (the metadata documents, registration, PKCE
//! shapes, and the answers) are this module's children; consent, stored codes, and token
//! minting are the built-in server's alone.

pub mod client;
mod flow;
pub(crate) mod metadata;
pub(crate) mod pkce;
pub(crate) mod reply;

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::routing::{get, post};
use cairn_schema::Slug;
use url::Url;

use crate::accounts::Accounts;
use crate::error::Refusal;
use crate::layer::Authenticate;
use crate::provider::{AuthProvider, Presented, Verdict};
use crate::secret::SECRET_PREFIX;
use metadata::Metadata;

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
pub struct OAuthServer {
    inner: Arc<Server>,
}

struct Server {
    config: OAuthConfig,
    accounts: Accounts,
    metadata: Arc<Metadata>,
}

impl OAuthServer {
    /// The provider for a deployment at the public `base` URL.
    #[must_use]
    pub fn new(config: OAuthConfig, accounts: Accounts, base: &Url) -> Self {
        let inner = Server {
            config,
            accounts,
            metadata: Arc::new(Metadata::new(base, &client::BUILT_IN)),
        };
        Self {
            inner: Arc::new(inner),
        }
    }
}

impl Server {
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
}

#[async_trait]
impl AuthProvider for OAuthServer {
    fn name(&self) -> &Slug {
        &self.inner.config.name
    }

    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        self.inner.verdict(request).await
    }

    /// Every bearer token with Cairn's prefix is this provider's: a session token or a code
    /// presented as one is refused, not passed on.
    fn claims_bearer(&self, token: &str) -> bool {
        token.starts_with(SECRET_PREFIX)
    }

    /// I2, RFC 9728: where an MCP client finds how to get a token for `path`.
    fn challenge(&self, path: &str) -> Option<String> {
        Some(self.inner.metadata.challenge(path))
    }

    fn router(&self, authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        let state = flow::Flow {
            server: Arc::clone(&self.inner),
            authenticator,
        };
        let router = Router::new()
            .route(metadata::REGISTER, post(flow::register))
            .route(
                metadata::AUTHORIZE,
                get(flow::authorize).post(flow::consent),
            )
            .route(metadata::TOKEN, post(flow::token))
            .with_state(Arc::new(state));
        Some(router.merge(self.inner.metadata.router()))
    }
}
