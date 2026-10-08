//! Cairn's auth (ARCHITECTURE, Auth): one [`AuthProvider`] trait and its providers, users
//! and their identities, agent tokens, and the auth log, all stored through the store's
//! records outside every domain, plus the axum layer the HTTP API and MCP endpoint sit
//! behind ([`Auth::protect`]). No RBAC: every authenticated user reads and writes
//! everything in a deployment (H4), so authentication is all there is.

#![forbid(unsafe_code)]

pub mod accounts;
pub mod clock;
pub mod cookies;
pub mod dev;
pub mod error;
pub mod gcp_iap;
pub mod layer;
pub mod lifetimes;
pub mod limits;
pub mod oauth;
pub mod oidc;
mod outbound;
pub mod provider;
pub mod secret;
pub mod tailscale;
pub mod tokens;

pub use accounts::Accounts;
pub use clock::Clock;
pub use dev::{DevConfig, DevProvider};
pub use error::{AuthError, ConfigError, Refusal};
pub use gcp_iap::{GcpIapConfig, GcpIapProvider};
pub use layer::{Auth, Authenticate};
pub use oauth::{OAuthConfig, OAuthServer};
pub use oidc::{OidcConfig, OidcProvider};
pub use provider::{AuthProvider, BoxFuture, Listener, Peer, Presented, Verdict};
pub use secret::Secret;
pub use tailscale::{
    ProxySource, TailscaleConfig, TailscaleMode, TailscaleProvider, TrustedProxies,
};
pub use tokens::{AgentToken, MintedToken};
