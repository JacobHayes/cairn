//! The Tailscale provider (ARCHITECTURE, Auth), in one of two modes.
//!
//! Direct: the listener is bound to a tailnet address, and each request's peer is looked
//! up with the local tailscaled's whois; `Tailscale-*` headers are never read. Proxy: an
//! authenticating proxy names the user in `Tailscale-User-*` headers. Either the proxy is
//! `tailscale serve` on this machine, in front of a listener only this machine reaches
//! (loopback or a unix socket), or it is on another machine and the provider lists the
//! addresses it connects from (`trusted_proxies`), on any listener. Proxy mode is a
//! security choice about this provider, enabled explicitly; a request carrying any
//! `Tailscale-*` header from a peer it does not trust is refused, so a forged header never
//! passes for anonymous or for a user
//! (decisions/2026-10-08-tailscale-proxy-mode-trusts-the-proxies-it-lists.md).
//!
//! A Tailscale login name was authenticated by the tailnet's identity provider, so it is
//! listed as a verified email when it has the shape of one (H3;
//! decisions/2026-10-06-a-tailscale-login-name-is-a-verified-email.md). A tagged node is
//! a machine, not a person: it signs no one in.

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use axum::body::Bytes;
use axum::http::header::HOST;
use axum::http::{HeaderMap, Request, StatusCode};
use cairn_schema::{Email, Identity, Limit, Slug, Title};
use http_body_util::{BodyExt, Empty, Limited};
use hyper_util::rt::TokioIo;
use ipnet::{IpNet, Ipv4Net};
use serde::Deserialize;

use crate::error::{ConfigError, Refusal};
use crate::limits::IDENTITY_CALL_DURATION_MAX;
use crate::provider::{AuthProvider, BoxFuture, Listener, Peer, Presented, Verdict};

/// How the provider learns who a request is from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TailscaleMode {
    /// Ask the local tailscaled, at this unix socket, who the peer is.
    Direct {
        /// tailscaled's local API socket.
        socket: PathBuf,
    },
    /// Trust the `Tailscale-User-*` headers of a proxy, from where it connects.
    Proxy {
        /// Where the proxy connects from.
        trusted: TrustedProxies,
    },
}

/// Where proxy mode trusts the identity headers from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrustedProxies {
    /// This machine (`tailscale serve`), on a listener only this machine reaches.
    ThisMachine,
    /// A proxy on another machine, connecting from one of these, on any listener. Never
    /// empty.
    Sources(Vec<ProxySource>),
}

/// An address or network a trusted proxy connects from: `10.10.10.2` or `10.10.10.0/24`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProxySource(IpNet);

impl ProxySource {
    /// Whether a peer at `address` connects from here. An IPv4 address mapped into IPv6
    /// is its IPv4 address.
    #[must_use]
    pub fn contains(self, address: IpAddr) -> bool {
        self.0.contains(&address.to_canonical())
    }
}

impl FromStr for ProxySource {
    type Err = String;

    /// An address (a network of one) or a network in CIDR notation; host bits in a
    /// network are dropped. An IPv4 address or network mapped into IPv6 is its IPv4 one,
    /// as a peer's address is.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if let Ok(address) = text.parse::<IpAddr>() {
            return Ok(Self(IpNet::from(address.to_canonical())));
        }
        text.parse::<IpNet>()
            .map(|network| Self(canonical(network.trunc())))
            .map_err(|_| format!("{text:?} is not an IP address or a network like 10.0.0.0/24"))
    }
}

/// `network` with an IPv4 network mapped into IPv6 (`::ffff:a.b.c.d/96` or longer) as that
/// IPv4 network.
fn canonical(network: IpNet) -> IpNet {
    if let IpNet::V6(v6) = network
        && let Some(v4) = v6.addr().to_ipv4_mapped()
        && let Some(prefix) = v6.prefix_len().checked_sub(96)
        && let Ok(v4) = Ipv4Net::new(v4, prefix)
    {
        return IpNet::V4(v4);
    }
    network
}

impl fmt::Display for ProxySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

/// The Tailscale provider's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TailscaleConfig {
    /// The provider's name.
    pub name: Slug,
    /// Direct or proxy.
    pub mode: TailscaleMode,
    /// H3: link a new identity to the user holding one of its verified emails.
    pub auto_link: bool,
}

/// The Tailscale provider.
#[derive(Debug)]
pub struct TailscaleProvider {
    config: TailscaleConfig,
}

/// The header naming the user's login, set by `tailscale serve`.
const LOGIN_HEADER: &str = "tailscale-user-login";
/// The header naming the user's display name.
const NAME_HEADER: &str = "tailscale-user-name";
/// The header listing a tagged node's tags: a machine, which signs no one in.
const TAGS_HEADER: &str = "tailscale-node-tags";
/// What every identity header's name starts with.
const HEADER_PREFIX: &str = "tailscale-";

impl TailscaleProvider {
    /// The provider for a server on `listener`.
    ///
    /// # Errors
    ///
    /// Proxy mode trusting this machine on a listener other machines reach, or trusting an
    /// empty list; direct mode on a listener that is not bound to a tailnet address.
    pub fn new(config: TailscaleConfig, listener: Listener) -> Result<Self, ConfigError> {
        let refuse = |reason: &str| ConfigError {
            provider: config.name.to_string(),
            reason: reason.to_owned(),
        };
        match (&config.mode, listener) {
            (
                TailscaleMode::Proxy {
                    trusted: TrustedProxies::ThisMachine,
                },
                listener,
            ) if !listener.is_local() => Err(refuse(
                "proxy mode needs a listener only the proxy reaches (loopback or a unix \
                 socket), or the addresses a proxy on another machine connects from \
                 (trusted_proxies)",
            )),
            (
                TailscaleMode::Proxy {
                    trusted: TrustedProxies::Sources(sources),
                },
                _,
            ) if sources.is_empty() => Err(refuse("trusted_proxies lists no address")),
            (TailscaleMode::Direct { .. }, Listener::Tcp(address)) if !is_tailnet(address.ip()) => {
                Err(refuse(
                    "direct mode needs a listener bound to a tailnet address",
                ))
            }
            (TailscaleMode::Direct { .. }, Listener::Unix) => Err(refuse(
                "direct mode needs a TCP listener on a tailnet address",
            )),
            _ => Ok(Self { config }),
        }
    }

    /// Proxy mode: the headers name the user, from a trusted peer only. A request carrying
    /// any `Tailscale-*` header from another peer is refused; one from a trusted peer that
    /// names a tagged node, or no user, signs no one in.
    fn proxied(&self, trusted: &TrustedProxies, request: Presented<'_>) -> Verdict {
        let carries = request
            .headers
            .keys()
            .any(|name| name.as_str().starts_with(HEADER_PREFIX));
        if !carries {
            return Verdict::Absent;
        }
        if !trusts(trusted, request.peer) {
            return Verdict::Refused(Refusal::Peer);
        }
        if header(request.headers, TAGS_HEADER).is_some() {
            return Verdict::Absent;
        }
        let Some(login) = header(request.headers, LOGIN_HEADER) else {
            return Verdict::Absent;
        };
        // tailscale serve encodes a display name that is not ASCII (RFC 2047); the login
        // stands in for it rather than a hand-written decoder.
        let name = header(request.headers, NAME_HEADER).filter(|name| !name.starts_with("=?"));
        self.identity(login, name)
    }

    /// Direct mode: tailscaled's whois of the peer.
    async fn looked_up(&self, socket: &std::path::Path, peer: Peer) -> Verdict {
        let Peer::Tcp(address) = peer else {
            return Verdict::Refused(Refusal::Peer);
        };
        let looked_up = tokio::time::timeout(IDENTITY_CALL_DURATION_MAX, whois(socket, address));
        match looked_up.await {
            Ok(Ok(Some(whois))) if whois.node.tags.as_ref().is_none_or(Vec::is_empty) => {
                let profile = whois.user_profile;
                self.identity(&profile.login_name, profile.display_name.as_deref())
            }
            Ok(Ok(Some(_))) => Verdict::Absent,
            Ok(Ok(None)) => Verdict::Refused(Refusal::Peer),
            Ok(Err(reason)) => Verdict::Refused(Refusal::Unavailable(format!("whois: {reason}"))),
            Err(_) => Verdict::Refused(Refusal::Unavailable("whois timed out".to_owned())),
        }
    }

    fn identity(&self, login: &str, name: Option<&str>) -> Verdict {
        let Ok(subject) = login.parse::<Title>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        let display = name
            .and_then(|name| name.parse::<Title>().ok())
            .unwrap_or_else(|| subject.clone());
        let verified_emails = login.parse::<Email>().into_iter().collect();
        Verdict::Identity(Identity {
            provider: self.config.name.clone(),
            subject,
            display,
            verified_emails,
        })
    }
}

impl AuthProvider for TailscaleProvider {
    fn name(&self) -> &Slug {
        &self.config.name
    }

    fn authenticate<'a>(&'a self, request: Presented<'a>) -> BoxFuture<'a, Verdict> {
        match &self.config.mode {
            TailscaleMode::Proxy { trusted } => {
                let verdict = self.proxied(trusted, request);
                Box::pin(async move { verdict })
            }
            TailscaleMode::Direct { socket } => Box::pin(self.looked_up(socket, request.peer)),
        }
    }

    fn auto_link(&self) -> bool {
        self.config.auto_link
    }
}

/// Whether proxy mode trusts the identity headers from `peer`.
fn trusts(trusted: &TrustedProxies, peer: Peer) -> bool {
    match (trusted, peer) {
        (TrustedProxies::ThisMachine, peer) => peer.is_local(),
        (TrustedProxies::Sources(sources), Peer::Tcp(address)) => {
            sources.iter().any(|source| source.contains(address.ip()))
        }
        (TrustedProxies::Sources(_), Peer::Unix | Peer::Unknown) => false,
    }
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    (!value.is_empty()).then_some(value)
}

/// A tailnet address: Tailscale's IPv4 range (100.64.0.0/10) or its IPv6 prefix
/// (`fd7a:115c:a1e0::/48`).
fn is_tailnet(address: IpAddr) -> bool {
    match address.to_canonical() {
        IpAddr::V4(address) => {
            let [first, second, ..] = address.octets();
            first == 100 && (64..128).contains(&second)
        }
        IpAddr::V6(address) => address.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

/// What Cairn reads of a whois answer; the rest of tailscaled's node record is ignored,
/// so a field Cairn never reads cannot break sign-in when tailscaled changes it.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Whois {
    node: WhoisNode,
    user_profile: WhoisProfile,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisNode {
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisProfile {
    login_name: String,
    display_name: Option<String>,
}

/// tailscaled's whois of `peer` over its local API socket: none when it does not know the
/// address.
async fn whois(socket: &std::path::Path, peer: SocketAddr) -> Result<Option<Whois>, String> {
    let stream = tokio::net::UnixStream::connect(socket)
        .await
        .map_err(|error| error.to_string())?;
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|error| error.to_string())?;
    tokio::spawn(connection);
    let request = Request::get(format!("/localapi/v0/whois?addr={peer}"))
        .header(HOST, "local-tailscaled.sock")
        .body(Empty::<Bytes>::new())
        .map_err(|error| error.to_string())?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|error| error.to_string())?;
    match response.status() {
        StatusCode::OK => {}
        StatusCode::NOT_FOUND => return Ok(None),
        status => return Err(format!("tailscaled answered {status}")),
    }
    // A whois answer is a few kilobytes; the body limit bounds a broken one.
    let bound = usize::try_from(Limit::BodyBytes.max()).unwrap_or(usize::MAX);
    let body = Limited::new(response.into_body(), bound).collect().await;
    let body = body.map_err(|error| error.to_string())?.to_bytes();
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proxy_source_is_an_address_or_a_network() {
        let cases = [
            ("10.10.10.2", "10.10.10.2", true),
            ("10.10.10.2", "10.10.10.3", false),
            ("10.10.10.0/24", "10.10.10.200", true),
            ("10.10.10.9/24", "10.10.10.200", true),
            ("10.10.10.0/24", "10.10.11.1", false),
            ("10.10.10.2", "::ffff:10.10.10.2", true),
            ("::ffff:10.10.10.2", "10.10.10.2", true),
            ("::ffff:10.10.10.2/128", "10.10.10.2", true),
            ("::ffff:10.10.10.0/120", "::ffff:10.10.10.9", true),
            ("::ffff:10.10.10.0/120", "10.10.11.1", false),
            ("fd00::/8", "fd00::1", true),
            ("fd00::/8", "10.10.10.2", false),
        ];
        for (source, peer, contains) in cases {
            let source: ProxySource = source.parse().unwrap();
            let peer: IpAddr = peer.parse().unwrap();
            assert_eq!(source.contains(peer), contains, "{source} {peer}");
        }
        for malformed in [
            "",
            "10.10.10",
            "10.10.10.0/33",
            "proxy.example",
            "10.0.0.1:80",
        ] {
            assert!(malformed.parse::<ProxySource>().is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn tailnet_addresses_are_tailscales_ranges() {
        let cases = [
            ("100.64.0.1", true),
            ("100.101.102.103", true),
            ("100.127.255.255", true),
            ("100.128.0.1", false),
            ("100.63.0.1", false),
            ("127.0.0.1", false),
            ("fd7a:115c:a1e0::1", true),
            ("fd7a:115c:a1e1::1", false),
            ("::ffff:100.100.1.1", true),
        ];
        for (address, tailnet) in cases {
            assert_eq!(is_tailnet(address.parse().unwrap()), tailnet, "{address}");
        }
    }
}
