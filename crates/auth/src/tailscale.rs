//! The Tailscale provider (ARCHITECTURE, Auth): an authenticating proxy (`tailscale serve`,
//! or a gateway on the tailnet that asks tailscaled who the peer is) names the user in
//! `Tailscale-User-*` headers. The provider lists the addresses the proxy connects from
//! (`trusted_proxies`); a request carrying any `Tailscale-*` header from a peer it does not
//! list is refused, so a forged header never passes for anonymous or for a user. A
//! request with none is anonymous
//! (decisions/2026-10-08-tailscale-proxy-mode-trusts-the-proxies-it-lists.md).
//!
//! A Tailscale login name was authenticated by the tailnet's sign-in provider, so it is
//! listed as a verified email when it has the shape of one (H3;
//! decisions/2026-10-06-a-tailscale-login-name-is-a-verified-email.md). A tagged node is
//! a machine, not a person: it signs no one in.

use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

use async_trait::async_trait;
use axum::http::HeaderMap;
use cairn_schema::{Email, Login, Slug, Title};
use ipnet::IpNet;

use crate::error::{ConfigError, Refusal};
use crate::provider::{AuthProvider, Presented, Verdict};

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

/// `network` as the IPv4 network it maps, when it is an IPv4-mapped IPv6 one, so it
/// compares with peers as `contains` canonicalizes them.
fn canonical(network: IpNet) -> IpNet {
    match network {
        IpNet::V6(v6) if v6.prefix_len() >= 96 => match v6.addr().to_canonical() {
            IpAddr::V4(address) => {
                IpNet::new(IpAddr::V4(address), v6.prefix_len() - 96).unwrap_or(network)
            }
            IpAddr::V6(_) => network,
        },
        _ => network,
    }
}

impl FromStr for ProxySource {
    type Err = String;

    /// An address (a network of one) or a network in CIDR notation; host bits in a
    /// network are dropped.
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
    /// Where the proxy connects from: the only peers whose login headers are trusted.
    pub trusted_proxies: Vec<ProxySource>,
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
/// What every login header's name starts with.
const HEADER_PREFIX: &str = "tailscale-";

impl TailscaleProvider {
    /// The provider.
    ///
    /// # Errors
    ///
    /// It trusts no proxy.
    pub fn new(config: TailscaleConfig) -> Result<Self, ConfigError> {
        if config.trusted_proxies.is_empty() {
            return Err(ConfigError {
                provider: config.name.to_string(),
                reason: "trusted_proxies lists no address".to_owned(),
            });
        }
        Ok(Self { config })
    }

    fn trusts(&self, request: Presented<'_>) -> bool {
        request.peer.is_some_and(|peer| {
            let sources = self.config.trusted_proxies.iter();
            sources.into_iter().any(|source| source.contains(peer.ip()))
        })
    }
}

#[async_trait]
impl AuthProvider for TailscaleProvider {
    /// The headers name the user, from a trusted peer only. A request carrying any
    /// `Tailscale-*` header from another peer is refused; one from a trusted peer that
    /// names a tagged node, or no user, signs no one in.
    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        let carries = request
            .headers
            .keys()
            .any(|name| name.as_str().starts_with(HEADER_PREFIX));
        if !carries {
            return Verdict::Absent;
        }
        if !self.trusts(request) {
            return Verdict::Refused(Refusal::Peer);
        }
        if header(request.headers, TAGS_HEADER).is_some() {
            return Verdict::Absent;
        }
        let Some(login) = header(request.headers, LOGIN_HEADER) else {
            return Verdict::Absent;
        };
        let Ok(subject) = login.parse::<Title>() else {
            return Verdict::Refused(Refusal::Credential);
        };
        // tailscale serve encodes a display name that is not ASCII (RFC 2047); the login
        // stands in for it rather than a hand-written decoder.
        let name = header(request.headers, NAME_HEADER)
            .filter(|name| !name.starts_with("=?"))
            .and_then(|name| name.parse::<Title>().ok());
        Verdict::Login(Login {
            provider: self.config.name.clone(),
            subject,
            name,
            verified_emails: login.parse::<Email>().into_iter().collect(),
        })
    }
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    (!value.is_empty()).then_some(value)
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
}
