//! The `Host` allowlist in front of the whole router (DECISIONS.md, 4.3 review round 1:
//! DNS-rebinding protection belongs to the listener). A page whose name an attacker rebinds
//! to the server's address makes the browser send that name as the request's `Host`, so the
//! dev provider without a token and Tailscale in direct mode, which trust the peer, would
//! otherwise answer it as their user. Every request whose host is not the public URL's, or,
//! when the listener is bound to loopback, a loopback name, is refused before the auth layer,
//! on the API, `/mcp`, auth's own routes, the UI, and `/metrics` alike.
//!
//! The name decides, not the port: a rebinding page can choose any port but never a name
//! other than its own, while proxies in front of Cairn differ in whether they forward the
//! port (DECISIONS.md, 4.7).

use std::collections::BTreeSet;
use std::net::SocketAddr;

use axum::Router;
use axum::extract::Request;
use axum::http::header::{CONTENT_TYPE, HOST};
use axum::http::uri::Authority;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use url::Url;

/// The names a loopback-bound listener also answers to.
const LOOPBACK_NAMES: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

/// The host names the server answers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowedHosts(BTreeSet<String>);

impl AllowedHosts {
    /// The public URL's host, and the loopback names when `listen` is a loopback address.
    ///
    /// # Panics
    ///
    /// When `public_url` has no host, which configuration never accepts.
    #[must_use]
    pub fn new(public_url: &Url, listen: SocketAddr) -> Self {
        let Some(public) = public_url.host_str() else {
            unreachable!("a public URL has a host: configuration checks it");
        };
        let mut names = BTreeSet::from([normalized(public)]);
        // An IPv4-mapped loopback (`::ffff:127.0.0.1`) is loopback too, as the auth
        // providers judge it.
        if listen.ip().to_canonical().is_loopback() {
            names.extend(LOOPBACK_NAMES.map(str::to_owned));
        }
        Self(names)
    }

    /// The names, normalized.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    /// Whether a request naming `authority` (its `Host`, or its target's authority) is
    /// answered.
    #[must_use]
    pub fn allows(&self, authority: &str) -> bool {
        // A Host is a name and a port; one carrying credentials is malformed (RFC 9110,
        // 7.2), never a name to look up.
        !authority.contains('@')
            && authority.parse::<Authority>().is_ok_and(|authority| {
                let port_valid = authority.as_str().len() == authority.host().len()
                    || authority.port_u16().is_some();
                port_valid && self.0.contains(&normalized(authority.host()))
            })
    }

    /// `router` behind the allowlist.
    pub fn guard(&self, router: Router) -> Router {
        let allowed = self.clone();
        router.layer(middleware::from_fn(move |request: Request, next: Next| {
            let allowed = allowed.clone();
            async move { allowed.admit(request, next).await }
        }))
    }

    async fn admit(&self, request: Request, next: Next) -> Response {
        // HTTP/1.1: a target in absolute form names the authority the client meant, and
        // the server must use it over the Host header (RFC 9112, 3.2.2).
        let named = match request.uri().authority() {
            Some(authority) => Some(authority.as_str().to_owned()),
            None => request
                .headers()
                .get(HOST)
                .and_then(|host| host.to_str().ok())
                .map(str::to_owned),
        };
        match named {
            Some(authority) if self.allows(&authority) => next.run(request).await,
            named => {
                tracing::warn!(
                    host = named.as_deref().unwrap_or(""),
                    "refused a request for a host this server does not serve"
                );
                misdirected()
            }
        }
    }
}

/// A host name compared as DNS does: case-insensitive, a trailing dot ignored.
fn normalized(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// 421 Misdirected Request: this server does not answer for that name.
fn misdirected() -> Response {
    (
        StatusCode::MISDIRECTED_REQUEST,
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        "this server does not answer for the host this request names\n",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(public_url: &str, listen: &str) -> AllowedHosts {
        AllowedHosts::new(&public_url.parse().unwrap(), listen.parse().unwrap())
    }

    #[test]
    fn the_public_name_is_allowed_on_any_port_and_in_any_case() {
        let hosts = allowed("https://Cairn.Example.com/", "0.0.0.0:8080");
        for host in [
            "cairn.example.com",
            "CAIRN.example.com:443",
            "cairn.example.com.:8080",
        ] {
            assert!(hosts.allows(host), "{host}");
        }
    }

    #[test]
    fn another_name_is_refused_however_it_resembles_the_public_one() {
        let hosts = allowed("https://cairn.example.com/", "0.0.0.0:8080");
        for host in [
            "attacker.example",
            "cairn.example.com.attacker.example",
            "localhost:8080",
            "127.0.0.1:8080",
            "",
            "cairn.example.com:notaport",
            "user@cairn.example.com",
        ] {
            assert!(!hosts.allows(host), "{host:?}");
        }
    }

    #[test]
    fn loopback_names_are_allowed_only_on_a_loopback_listener() {
        let loopback = allowed("https://cairn.example.com/", "127.0.0.1:8080");
        let public = allowed("https://cairn.example.com/", "100.64.0.7:8080");
        for host in [
            "localhost:8080",
            "127.0.0.1:5173",
            "[::1]:8080",
            "LOCALHOST",
        ] {
            assert!(loopback.allows(host), "{host}");
            assert!(!public.allows(host), "{host}");
        }
        for listen in ["[::1]:8080", "[::ffff:127.0.0.1]:8080"] {
            let v6 = allowed("https://cairn.example.com/", listen);
            assert!(v6.allows("[::1]:8080"), "{listen}");
            assert!(v6.allows("localhost:8080"), "{listen}");
        }
    }
}
