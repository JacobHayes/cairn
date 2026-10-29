//! Dynamic client registration (RFC 7591) for MCP clients (I2), shared by the built-in
//! server and the IAP relay: an open endpoint, public clients only (no client secret; PKCE
//! protects the code), and redirect URIs each server's [`Policy`] allows.
//!
//! A registered client is not stored: its id is its registration (name and redirect URIs)
//! encoded, and every use decodes and checks it again under the server's policy.
//! Registration is open to anyone, so a stored record would vouch for nothing a forged id
//! could not claim; the checks on the redirect URIs are what protect a user, and they run
//! on every authorization.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cairn_schema::{Limit, Title};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::outbound::is_trusted_origin;

/// The prefix of every client id this server issues.
const CLIENT_ID_PREFIX: &str = "client_";

/// The parameters an authorization response carries, which a registered redirect URI may
/// not set itself: the answer appends them, so a URI holding one would deliver two.
const RESPONSE_PARAMETERS: [&str; 6] = [
    "code",
    "state",
    "iss",
    "error",
    "error_description",
    "error_uri",
];

/// What a server registers.
#[derive(Debug)]
pub struct Policy {
    /// Redirects to a loopback address only, matched on any port (RFC 8252 section 7.3);
    /// otherwise https or loopback, matched exactly.
    pub loopback_only: bool,
    /// The grant types it offers, `authorization_code` first.
    pub grant_types: &'static [&'static str],
}

/// The built-in server: any https or loopback client, codes only (its tokens last until
/// revoked, so there is nothing to refresh).
pub const BUILT_IN: Policy = Policy {
    loopback_only: false,
    grant_types: &["authorization_code"],
};

/// The IAP relay: local clients only, so a code goes only to the machine of the person
/// signing in, which refresh Google's hour-long ID token.
pub const RELAY: Policy = Policy {
    loopback_only: true,
    grant_types: &["authorization_code", "refresh_token"],
};

/// A registered client, as its id encodes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Client {
    /// The name the client registered, shown when a user is asked to allow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Title>,
    /// Where codes may be sent.
    pub redirect_uris: Vec<String>,
}

/// Why a registration or a client id is not accepted, as RFC 7591 names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientError {
    /// The error code.
    pub code: &'static str,
    /// What is wrong.
    pub description: String,
}

/// An RFC 7591 registration request: the fields this server reads. Others, such as the
/// grants and response types asked for, are ignored, as the RFC allows: the answer says
/// what the server offers.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Registration {
    /// Where codes may be sent.
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    /// The client's name.
    pub client_name: Option<String>,
}

impl Client {
    /// The client a registration describes, under `policy`.
    ///
    /// # Errors
    ///
    /// The name is not a title, or a redirect URI is not allowed.
    pub fn register(registration: Registration, policy: &Policy) -> Result<Self, ClientError> {
        let name = match registration.client_name.as_deref().map(str::parse::<Title>) {
            None => None,
            Some(Ok(name)) => Some(name),
            Some(Err(error)) => {
                return Err(client_error(
                    "invalid_client_metadata",
                    format!("client_name: {error}"),
                ));
            }
        };
        let client = Client {
            name,
            redirect_uris: registration.redirect_uris,
        };
        client.check(policy)?;
        Ok(client)
    }

    /// The client an id names, checked as at registration under `policy`.
    ///
    /// # Errors
    ///
    /// The id is not one this server issues, or what it encodes is not allowed.
    pub fn from_id(id: &str, policy: &Policy) -> Result<Self, ClientError> {
        let unknown = || client_error("invalid_client", "unknown client_id".to_owned());
        let encoded = id.strip_prefix(CLIENT_ID_PREFIX).ok_or_else(unknown)?;
        let json = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| unknown())?;
        let client: Client = serde_json::from_slice(&json).map_err(|_| unknown())?;
        client.check(policy)?;
        Ok(client)
    }

    /// The client's id: its registration, encoded.
    #[must_use]
    pub fn id(&self) -> String {
        let json = match serde_json::to_vec(self) {
            Ok(json) => json,
            Err(error) => unreachable!("a client serializes: {error}"),
        };
        format!("{CLIENT_ID_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
    }

    /// Whether `uri` is one of the client's redirect URIs: compared exactly, or for a
    /// loopback-only policy, exactly but for the port.
    #[must_use]
    pub fn redirects_to(&self, uri: &str, policy: &Policy) -> bool {
        let key = |uri: &str| {
            if !policy.loopback_only {
                return Some(uri.to_owned());
            }
            let mut url = Url::parse(uri).ok()?;
            url.set_port(None).ok()?;
            Some(url.into())
        };
        let asked = key(uri);
        asked.is_some() && self.redirect_uris.iter().any(|known| key(known) == asked)
    }

    /// At least one redirect URI, each allowed, and the whole within the body limit.
    fn check(&self, policy: &Policy) -> Result<(), ClientError> {
        let redirect = |reason: String| client_error("invalid_redirect_uri", reason);
        if self.redirect_uris.is_empty() {
            return Err(redirect("at least one redirect URI".to_owned()));
        }
        for uri in &self.redirect_uris {
            let refuse = |reason: &str| redirect(format!("{uri}: {reason}"));
            let Ok(url) = Url::parse(uri) else {
                return Err(refuse("not a URL"));
            };
            if url.fragment().is_some() {
                return Err(refuse("a redirect URI has no fragment"));
            }
            let sets_answer = url
                .query_pairs()
                .any(|(name, _)| RESPONSE_PARAMETERS.contains(&name.as_ref()));
            if sets_answer {
                return Err(refuse(
                    "a redirect URI does not set a parameter of the answer",
                ));
            }
            if !is_trusted_origin(&url) || (policy.loopback_only && url.scheme() != "http") {
                return Err(refuse("only https, or http to a loopback address"));
            }
        }
        let size = serde_json::to_vec(self).map_or(usize::MAX, |json| json.len());
        Limit::BodyBytes
            .check(size)
            .map_err(|error| client_error("invalid_client_metadata", error.to_string()))
    }
}

fn client_error(code: &'static str, description: String) -> ClientError {
    ClientError { code, description }
}
