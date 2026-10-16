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

/// What a server registers: which redirect URIs, and which grants it offers.
#[derive(Debug)]
pub struct Policy {
    /// The redirect URIs it allows.
    pub redirects: Redirects,
    /// The grant types it offers, `authorization_code` first; a registration must ask for
    /// that one, and is answered with all of them.
    pub grant_types: &'static [&'static str],
}

/// The redirect URIs a server allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redirects {
    /// https, or http to a loopback address, each matched exactly.
    HttpsOrLoopback,
    /// http to a loopback address only, matched on any port (RFC 8252 section 7.3): codes
    /// go only to the machine of the person signing in.
    Loopback,
}

/// The built-in server: any https or loopback client, codes only (its tokens last until
/// revoked, so there is nothing to refresh).
pub const BUILT_IN: Policy = Policy {
    redirects: Redirects::HttpsOrLoopback,
    grant_types: &["authorization_code"],
};

/// The IAP relay: local clients only, which refresh Google's hour-long ID token.
pub const RELAY: Policy = Policy {
    redirects: Redirects::Loopback,
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

/// Why a registration or a client id is not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// A redirect URI is not one the policy allows, has a fragment or a response
    /// parameter, or does not parse.
    RedirectUri(String),
    /// The metadata is not something this server registers.
    Metadata(String),
}

impl ClientError {
    /// The RFC 7591 error code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            ClientError::RedirectUri(_) => "invalid_redirect_uri",
            ClientError::Metadata(_) => "invalid_client_metadata",
        }
    }

    /// What is wrong.
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            ClientError::RedirectUri(reason) | ClientError::Metadata(reason) => reason,
        }
    }
}

/// An RFC 7591 registration request: the fields this server reads. Others are ignored, as
/// the RFC allows.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Registration {
    /// Where codes may be sent.
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    /// The client's name.
    pub client_name: Option<String>,
    /// Must be `none` if given: public clients only.
    pub token_endpoint_auth_method: Option<String>,
    /// Must include `authorization_code` if given; the server answers with the grants it
    /// offers, which RFC 7591 section 3.2.1 lets it do.
    pub grant_types: Option<Vec<String>>,
    /// Must include only `code` if given.
    pub response_types: Option<Vec<String>>,
}

impl Client {
    /// The client a registration describes, under `policy`.
    ///
    /// # Errors
    ///
    /// The registration asks for what this server does not do, or a redirect URI is not
    /// allowed.
    pub fn register(registration: Registration, policy: &Policy) -> Result<Self, ClientError> {
        let metadata = |reason: &str| Err(ClientError::Metadata(reason.to_owned()));
        if registration
            .token_endpoint_auth_method
            .as_deref()
            .is_some_and(|method| method != "none")
        {
            return metadata("only public clients (token_endpoint_auth_method none) register");
        }
        let codes = registration
            .grant_types
            .as_ref()
            .is_none_or(|grants| grants.iter().any(|grant| grant == "authorization_code"));
        if !codes {
            return metadata("a client registers for the authorization_code grant");
        }
        let mut responses = registration.response_types.iter().flatten();
        if !responses.all(|value| value == "code") {
            return metadata("the only response type is code");
        }
        let name = match registration.client_name.as_deref().map(str::parse::<Title>) {
            None => None,
            Some(Ok(name)) => Some(name),
            Some(Err(error)) => return metadata(&format!("client_name: {error}")),
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
        let unknown = || ClientError::Metadata("unknown client_id".to_owned());
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
        let registered = self.redirect_uris.iter();
        match policy.redirects {
            Redirects::HttpsOrLoopback => registered.into_iter().any(|known| known == uri),
            Redirects::Loopback => {
                let Some(asked) = without_port(uri) else {
                    return false;
                };
                registered
                    .filter_map(|known| without_port(known))
                    .any(|known| known == asked)
            }
        }
    }

    /// At least one redirect URI, each allowed, and the whole within the body limit (the
    /// nearest named limit for a document of free text;
    /// decisions/2026-10-06-values-the-limits-table-does-not-name-take-the-nearest-named.md).
    fn check(&self, policy: &Policy) -> Result<(), ClientError> {
        if self.redirect_uris.is_empty() {
            return Err(ClientError::RedirectUri(
                "at least one redirect URI".to_owned(),
            ));
        }
        for uri in &self.redirect_uris {
            check_redirect_uri(uri, policy.redirects)?;
        }
        let size = serde_json::to_vec(self).map_or(usize::MAX, |json| json.len());
        Limit::BodyBytes
            .check(size)
            .map_err(|error| ClientError::Metadata(error.to_string()))
    }
}

/// A loopback redirect URI without its port, for RFC 8252's any-port match.
fn without_port(uri: &str) -> Option<String> {
    let mut url = Url::parse(uri).ok()?;
    url.set_port(None).ok()?;
    Some(url.into())
}

/// A redirect URI the policy allows: https or http to a loopback address (RFC 8252's
/// native clients), or loopback only; never with a fragment, nor with a parameter the
/// authorization response sets.
fn check_redirect_uri(uri: &str, redirects: Redirects) -> Result<(), ClientError> {
    let refuse = |reason: &str| Err(ClientError::RedirectUri(format!("{uri}: {reason}")));
    let Ok(url) = Url::parse(uri) else {
        return refuse("not a URL");
    };
    if url.fragment().is_some() {
        return refuse("a redirect URI has no fragment");
    }
    if url
        .query_pairs()
        .any(|(name, _)| RESPONSE_PARAMETERS.contains(&name.as_ref()))
    {
        return refuse("a redirect URI does not set a parameter of the answer");
    }
    match redirects {
        Redirects::HttpsOrLoopback if !is_trusted_origin(&url) => {
            refuse("only https, or http to a loopback address")
        }
        Redirects::Loopback if url.scheme() != "http" || !is_trusted_origin(&url) => {
            refuse("only http to a loopback address")
        }
        Redirects::HttpsOrLoopback | Redirects::Loopback => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration(uris: &[&str]) -> Registration {
        Registration {
            redirect_uris: uris.iter().map(|uri| (*uri).to_owned()).collect(),
            client_name: Some("An MCP client".to_owned()),
            ..Registration::default()
        }
    }

    /// I2: what each server registers. The built-in server takes https or loopback; the
    /// relay loopback alone; neither a fragment nor a parameter its answer sets.
    #[test]
    fn registration_accepts_only_https_or_loopback_redirect_uris() {
        // (URI, built-in, relay)
        let cases = [
            ("https://client.example/callback", true, false),
            ("http://127.0.0.1:33418/callback", true, true),
            ("http://[::1]:33418/callback", true, true),
            ("http://localhost:33418/callback", true, true),
            ("http://localhost:33418/callback?next=x", true, true),
            ("https://127.0.0.1:33418/callback", true, false),
            ("http://client.example/callback", false, false),
            ("http://10.0.0.2/callback", false, false),
            ("https://client.example/callback#fragment", false, false),
            ("http://localhost:1/cb?iss=x", false, false),
            ("http://localhost:1/cb?code=x", false, false),
            ("http://localhost:1/cb?state=x", false, false),
            ("http://localhost:1/cb?error=x", false, false),
            ("custom-scheme://callback", false, false),
            ("not a url", false, false),
        ];
        for (uri, built_in, relay) in cases {
            for (policy, accepted) in [(&BUILT_IN, built_in), (&RELAY, relay)] {
                let registered = Client::register(registration(&[uri]), policy);
                assert_eq!(
                    registered.is_ok(),
                    accepted,
                    "{uri} {policy:?}: {registered:?}"
                );
            }
        }
        assert!(Client::register(registration(&[]), &BUILT_IN).is_err());
        assert!(Client::register(registration(&[]), &RELAY).is_err());
    }

    #[test]
    fn registration_accepts_only_public_code_clients() {
        let cases = [
            (Some("client_secret_basic"), None, None),
            (None, Some(vec!["client_credentials".to_owned()]), None),
            (None, None, Some(vec!["token".to_owned()])),
        ];
        for (method, grants, responses) in cases {
            let refused = Client::register(
                Registration {
                    token_endpoint_auth_method: method.map(str::to_owned),
                    grant_types: grants.clone(),
                    response_types: responses.clone(),
                    ..registration(&["https://client.example/callback"])
                },
                &BUILT_IN,
            );
            assert!(
                matches!(refused, Err(ClientError::Metadata(_))),
                "{method:?} {grants:?} {responses:?}"
            );
        }
    }

    /// I2: a client asking for refresh beside codes, as MCP clients do, registers with
    /// either server; one not asking for codes registers with neither.
    #[test]
    fn registration_keeps_only_the_grant_types_offered() {
        let grants = |names: &[&str]| Registration {
            grant_types: Some(names.iter().map(|name| (*name).to_owned()).collect()),
            ..registration(&["http://localhost:33418/callback"])
        };
        for policy in [&BUILT_IN, &RELAY] {
            let both = grants(&["authorization_code", "refresh_token"]);
            assert!(Client::register(both, policy).is_ok(), "{policy:?}");
            for refused in [&["refresh_token"][..], &["client_credentials"], &[]] {
                let registered = Client::register(grants(refused), policy);
                assert!(registered.is_err(), "{refused:?} {policy:?}");
            }
        }
    }

    #[test]
    fn a_client_id_names_its_registration_and_redirects_match_exactly() {
        let uris = ["https://client.example/callback", "http://127.0.0.1:9/cb"];
        let client = Client::register(registration(&uris), &BUILT_IN).unwrap();
        assert_eq!(Client::from_id(&client.id(), &BUILT_IN), Ok(client.clone()));
        assert!(client.redirects_to("http://127.0.0.1:9/cb", &BUILT_IN));
        for other in [
            "http://127.0.0.1:10/cb",
            "https://client.example/callback/",
            "https://client.example/callback?x=1",
        ] {
            assert!(!client.redirects_to(other, &BUILT_IN), "{other}");
        }
        let forged = Client {
            name: None,
            redirect_uris: vec!["http://attacker.example/".to_owned()],
        };
        assert!(Client::from_id(&forged.id(), &BUILT_IN).is_err());
        assert!(Client::from_id("client_!!", &BUILT_IN).is_err());
        assert!(Client::from_id("someone-else", &BUILT_IN).is_err());
        let preset = Client {
            name: None,
            redirect_uris: vec!["http://127.0.0.1:9/cb?iss=x".to_owned()],
        };
        assert!(Client::from_id(&preset.id(), &BUILT_IN).is_err());
    }

    /// RFC 8252 section 7.3: a relay client's loopback redirect matches on any port, and
    /// on nothing else; an id minted with an https redirect is not the relay's.
    #[test]
    fn a_relay_loopback_redirect_matches_on_any_port() {
        let registered = "http://localhost:59411/callback";
        let client = Client::register(registration(&[registered]), &RELAY).unwrap();
        let client = Client::from_id(&client.id(), &RELAY).unwrap();
        let cases = [
            ("http://localhost:59411/callback", true),
            ("http://localhost:60000/callback", true),
            ("http://localhost/callback", true),
            ("http://localhost:60000/other", false),
            ("http://localhost:60000/callback?x=1", false),
            ("http://127.0.0.1:59411/callback", false),
            ("https://localhost:59411/callback", false),
            ("not a url", false),
        ];
        for (uri, matches) in cases {
            assert_eq!(client.redirects_to(uri, &RELAY), matches, "{uri}");
        }
        assert!(!client.redirects_to("http://localhost:60000/callback", &BUILT_IN));
        let web = Client::register(registration(&["https://client.example/cb"]), &BUILT_IN);
        assert!(Client::from_id(&web.unwrap().id(), &RELAY).is_err());
    }
}
