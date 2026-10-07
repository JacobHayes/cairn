//! The Tailscale provider (ARCHITECTURE, Auth), in one of two modes.
//!
//! Direct: the listener is bound to a tailnet address, and each request's peer is looked
//! up with the local tailscaled's whois. Proxy: `tailscale serve` (or another
//! authenticating proxy) in front of a listener only it can reach, loopback or a unix
//! socket, names the user in `Tailscale-User-*` headers. Proxy mode is a security choice
//! about this provider, enabled explicitly; it refuses to start on any other listener and
//! refuses the headers from any peer but this machine.
//!
//! A Tailscale login name was authenticated by the tailnet's identity provider, so it is
//! listed as a verified email when it has the shape of one (H3;
//! decisions/2026-10-06-a-tailscale-login-name-is-a-verified-email.md). A tagged node is
//! a machine, not a person: it signs no one in.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use axum::body::Bytes;
use axum::http::header::HOST;
use axum::http::{HeaderMap, Request, StatusCode};
use cairn_schema::{Email, Identity, Limit, Slug, Title};
use http_body_util::{BodyExt, Empty, Limited};
use hyper_util::rt::TokioIo;
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
    /// Trust the `Tailscale-User-*` headers of the proxy in front of a local listener.
    Proxy,
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

impl TailscaleProvider {
    /// The provider for a server on `listener`.
    ///
    /// # Errors
    ///
    /// Proxy mode on a listener other machines reach, or direct mode on a listener that is
    /// not bound to a tailnet address.
    pub fn new(config: TailscaleConfig, listener: Listener) -> Result<Self, ConfigError> {
        let refuse = |reason: &str| ConfigError {
            provider: config.name.to_string(),
            reason: reason.to_owned(),
        };
        match (&config.mode, listener) {
            (TailscaleMode::Proxy, listener) if !listener.is_local() => Err(refuse(
                "proxy mode needs a listener only the proxy reaches: loopback or a unix socket",
            )),
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

    /// Proxy mode: the headers name the user, from this machine only.
    fn proxied(&self, request: Presented<'_>) -> Verdict {
        let Some(login) = header(request.headers, LOGIN_HEADER) else {
            return Verdict::Absent;
        };
        if !request.peer.is_local() {
            return Verdict::Refused(Refusal::Peer);
        }
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
            TailscaleMode::Proxy => {
                let verdict = self.proxied(request);
                Box::pin(async move { verdict })
            }
            TailscaleMode::Direct { socket } => Box::pin(self.looked_up(socket, request.peer)),
        }
    }

    fn auto_link(&self) -> bool {
        self.config.auto_link
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
