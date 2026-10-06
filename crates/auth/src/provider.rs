//! The `AuthProvider` trait (ARCHITECTURE, Auth): given a request, a provider answers with
//! the identity it vouches for, an actor Cairn already knows, nothing, or a refusal.
//! Providers are configured independently and run together; each may serve its own routes
//! (a login and its callback, the OAuth endpoints).

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use cairn_schema::{Actor, Identity, Slug};

use crate::error::Refusal;
use crate::layer::Authenticate;

/// A boxed future, so that providers stay trait objects (PRACTICES, Not adopted: the auth
/// provider is a trait object by design).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What one provider makes of one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The request carries nothing this provider reads.
    Absent,
    /// The provider vouches for this identity; Cairn resolves it to a user.
    Identity(Identity),
    /// A credential Cairn issued itself (an agent token) names this actor directly.
    Actor(Actor),
    /// The request carries this provider's credential and it is not good. The request is
    /// refused rather than offered to another provider.
    Refused(Refusal),
}

/// Where a listener accepts connections: what decides whether the local-only providers
/// (dev without its override, Tailscale's proxy mode) may run at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    /// A TCP listener at this address.
    Tcp(SocketAddr),
    /// A unix socket, reachable only from this machine.
    Unix,
}

impl Listener {
    /// Whether only this machine can connect: a loopback address or a unix socket.
    #[must_use]
    pub fn is_local(self) -> bool {
        match self {
            Listener::Tcp(address) => is_loopback(address.ip()),
            Listener::Unix => true,
        }
    }
}

/// Who is on the other end of a request's connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Peer {
    /// A TCP peer at this address.
    Tcp(SocketAddr),
    /// A peer on a unix socket: this machine.
    Unix,
    /// Unknown: a TCP listener whose server did not record peers. Never local.
    Unknown,
}

impl Peer {
    /// Whether the peer is this machine.
    #[must_use]
    pub fn is_local(self) -> bool {
        match self {
            Peer::Tcp(address) => is_loopback(address.ip()),
            Peer::Unix => true,
            Peer::Unknown => false,
        }
    }
}

/// A loopback address, including an IPv4 loopback address mapped into IPv6.
fn is_loopback(address: IpAddr) -> bool {
    address.to_canonical().is_loopback()
}

/// What a provider sees of a request: its headers and its peer.
#[derive(Clone, Copy, Debug)]
pub struct Presented<'a> {
    /// The request's headers.
    pub headers: &'a HeaderMap,
    /// The connection's peer.
    pub peer: Peer,
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

/// One way of authenticating (ARCHITECTURE, Auth).
pub trait AuthProvider: Send + Sync {
    /// The provider's configured name: the `provider` of every identity it vouches for.
    fn name(&self) -> &Slug;

    /// What this provider makes of a request. A provider refuses only a credential of its
    /// own; one it does not read is [`Verdict::Absent`].
    fn authenticate<'a>(&'a self, request: Presented<'a>) -> BoxFuture<'a, Verdict>;

    /// Whether `token`, presented as a bearer token, is this provider's to check. A bearer
    /// token no provider claims is refused, so a mistyped token never passes unnoticed.
    fn claims_bearer(&self, token: &str) -> bool {
        let _ = token;
        false
    }

    /// H3: whether a new identity from this provider links to the user already holding one
    /// of its verified emails, a per-provider security choice (ARCHITECTURE, Auth).
    fn auto_link(&self) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_means_loopback_or_a_unix_socket() {
        let cases = [
            ("127.0.0.1:8080", true),
            ("127.3.4.5:8080", true),
            ("[::1]:8080", true),
            ("[::ffff:127.0.0.1]:8080", true),
            ("0.0.0.0:8080", false),
            ("192.168.1.4:8080", false),
            ("[::]:8080", false),
            ("100.101.102.103:443", false),
        ];
        for (address, local) in cases {
            let address: SocketAddr = address.parse().unwrap();
            assert_eq!(Listener::Tcp(address).is_local(), local, "{address}");
            assert_eq!(Peer::Tcp(address).is_local(), local, "{address}");
        }
        assert!(Listener::Unix.is_local());
        assert!(Peer::Unix.is_local());
        assert!(!Peer::Unknown.is_local());
    }

    #[test]
    fn a_bearer_token_is_read_from_the_authorization_header() {
        let cases = [
            (Some("Bearer abc"), Some("abc")),
            (Some("bearer  abc "), Some("abc")),
            (Some("Basic abc"), None),
            (Some("Bearer "), None),
            (Some("Bearer"), None),
            (None, None),
        ];
        for (header, token) in cases {
            let mut headers = HeaderMap::new();
            if let Some(header) = header {
                headers.insert(AUTHORIZATION, header.parse().unwrap());
            }
            let request = Presented {
                headers: &headers,
                peer: Peer::Unix,
            };
            assert_eq!(request.bearer(), token, "{header:?}");
        }
    }
}
