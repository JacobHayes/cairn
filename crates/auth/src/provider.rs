//! The `AuthProvider` trait (ARCHITECTURE, Auth): given a request, a provider answers with
//! the login it vouches for, an actor Cairn already knows, nothing, or a refusal. Providers
//! are configured independently and run together; each may serve its own routes (a login
//! and its callback, the OAuth endpoints).

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use cairn_schema::{Actor, Login};

use crate::error::Refusal;
use crate::layer::Authenticate;

/// What one provider makes of one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The request carries nothing this provider reads.
    Absent,
    /// The provider vouches for this login; Cairn resolves it to a user.
    Login(Login),
    /// A credential Cairn issued itself (an agent token) names this actor directly.
    Actor(Actor),
    /// The request carries this provider's credential and it is not good. The request is
    /// refused rather than offered to another provider.
    Refused(Refusal),
}

/// What a provider sees of a request: its headers and its peer.
#[derive(Clone, Copy, Debug)]
pub struct Presented<'a> {
    /// The request's headers.
    pub headers: &'a HeaderMap,
    /// The connection's peer; none when the server did not record it, which is never local.
    pub peer: Option<SocketAddr>,
}

impl Presented<'_> {
    /// The bearer token in the `Authorization` header, if it holds one.
    #[must_use]
    pub fn bearer(&self) -> Option<&str> {
        let value = self.headers.get(AUTHORIZATION)?.to_str().ok()?;
        let (scheme, token) = value.split_once(' ')?;
        let token = token.trim();
        (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
    }
}

/// Whether `address` is this machine: a loopback address, including an IPv4 loopback
/// address mapped into IPv6.
#[must_use]
pub fn is_loopback(address: IpAddr) -> bool {
    address.to_canonical().is_loopback()
}

/// One way of authenticating (ARCHITECTURE, Auth).
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// What this provider makes of a request. A provider refuses only a credential of its
    /// own; one it does not read is [`Verdict::Absent`].
    async fn authenticate(&self, request: Presented<'_>) -> Verdict;

    /// Whether `token`, presented as a bearer token, is this provider's to check. A bearer
    /// token no provider claims is refused, so a mistyped token never passes unnoticed.
    fn claims_bearer(&self, token: &str) -> bool {
        let _ = token;
        false
    }

    /// The parameters this provider adds to the `WWW-Authenticate: Bearer` challenge of an
    /// unauthenticated request to `path` (the built-in OAuth server's resource metadata).
    fn challenge(&self, path: &str) -> Option<String> {
        let _ = path;
        None
    }

    /// The provider's own routes, at the paths they are served on: a login and its
    /// callback, the OAuth endpoints. They sit outside the auth layer; `authenticator`
    /// tells them who a browser is, as the layer would.
    fn router(&self, authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        let _ = authenticator;
        None
    }
}
