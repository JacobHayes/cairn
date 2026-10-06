//! Dynamic client registration (RFC 7591) for MCP clients (I2): an open endpoint, public
//! clients only (no client secret; PKCE protects the code), and redirect URIs that are
//! https or loopback http, matched exactly.
//!
//! A registered client is not stored: its id is its registration (name and redirect URIs)
//! encoded, and every use decodes and checks it again. Registration is open to anyone, so
//! a stored record would vouch for nothing a forged id could not claim; the checks on the
//! redirect URIs are what protect a user, and they run on every authorization.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cairn_schema::{Limit, Title};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::outbound::is_trusted_origin;

/// The prefix of every client id this server issues.
const CLIENT_ID_PREFIX: &str = "client_";

/// A registered client, as its id encodes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Client {
    /// The name the client registered, shown when a user is asked to allow it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Title>,
    /// Where codes may be sent, each matched exactly.
    pub redirect_uris: Vec<String>,
}

/// Why a registration or a client id is not accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// A redirect URI is not https or loopback http, has a fragment, or does not parse.
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
    /// Must include only `authorization_code` if given.
    pub grant_types: Option<Vec<String>>,
    /// Must include only `code` if given.
    pub response_types: Option<Vec<String>>,
}

impl Client {
    /// The client a registration describes.
    ///
    /// # Errors
    ///
    /// The registration asks for what this server does not do, or a redirect URI is not
    /// allowed.
    pub fn register(registration: Registration) -> Result<Self, ClientError> {
        let metadata = |reason: &str| Err(ClientError::Metadata(reason.to_owned()));
        if registration
            .token_endpoint_auth_method
            .as_deref()
            .is_some_and(|method| method != "none")
        {
            return metadata("only public clients (token_endpoint_auth_method none) register");
        }
        let only = |values: &Option<Vec<String>>, allowed: &str| {
            values.iter().flatten().all(|value| value == allowed)
        };
        if !only(&registration.grant_types, "authorization_code") {
            return metadata("the only grant type is authorization_code");
        }
        if !only(&registration.response_types, "code") {
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
        client.check()?;
        Ok(client)
    }

    /// The client an id names, checked as at registration.
    ///
    /// # Errors
    ///
    /// The id is not one this server issues, or what it encodes is not allowed.
    pub fn from_id(id: &str) -> Result<Self, ClientError> {
        let unknown = || ClientError::Metadata("unknown client_id".to_owned());
        let encoded = id.strip_prefix(CLIENT_ID_PREFIX).ok_or_else(unknown)?;
        let json = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| unknown())?;
        let client: Client = serde_json::from_slice(&json).map_err(|_| unknown())?;
        client.check()?;
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

    /// Whether `uri` is one of the client's redirect URIs, compared exactly.
    #[must_use]
    pub fn redirects_to(&self, uri: &str) -> bool {
        self.redirect_uris
            .iter()
            .any(|registered| registered == uri)
    }

    /// At least one redirect URI, each allowed, and the whole within the body limit (the
    /// nearest named limit for a document of free text; DECISIONS.md, brief 1.2).
    fn check(&self) -> Result<(), ClientError> {
        if self.redirect_uris.is_empty() {
            return Err(ClientError::RedirectUri(
                "at least one redirect URI".to_owned(),
            ));
        }
        for uri in &self.redirect_uris {
            check_redirect_uri(uri)?;
        }
        let size = serde_json::to_vec(self).map_or(usize::MAX, |json| json.len());
        Limit::BodyBytes
            .check(size)
            .map_err(|error| ClientError::Metadata(error.to_string()))
    }
}

/// A redirect URI is https, or http to a loopback address (RFC 8252's native clients),
/// with no fragment.
fn check_redirect_uri(uri: &str) -> Result<(), ClientError> {
    let refuse = |reason: &str| Err(ClientError::RedirectUri(format!("{uri}: {reason}")));
    let Ok(url) = Url::parse(uri) else {
        return refuse("not a URL");
    };
    if url.fragment().is_some() {
        return refuse("a redirect URI has no fragment");
    }
    if !is_trusted_origin(&url) {
        return refuse("only https, or http to a loopback address");
    }
    Ok(())
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

    #[test]
    fn registration_accepts_only_https_or_loopback_redirect_uris() {
        let cases = [
            ("https://client.example/callback", true),
            ("http://127.0.0.1:33418/callback", true),
            ("http://[::1]:33418/callback", true),
            ("http://localhost:33418/callback", true),
            ("http://client.example/callback", false),
            ("http://10.0.0.2/callback", false),
            ("https://client.example/callback#fragment", false),
            ("custom-scheme://callback", false),
            ("not a url", false),
        ];
        for (uri, accepted) in cases {
            let registered = Client::register(registration(&[uri]));
            assert_eq!(registered.is_ok(), accepted, "{uri}: {registered:?}");
        }
        assert!(Client::register(registration(&[])).is_err());
    }

    #[test]
    fn registration_accepts_only_public_code_clients() {
        let cases = [
            (Some("client_secret_basic"), None, None),
            (None, Some(vec!["client_credentials".to_owned()]), None),
            (None, None, Some(vec!["token".to_owned()])),
        ];
        for (method, grants, responses) in cases {
            let refused = Client::register(Registration {
                token_endpoint_auth_method: method.map(str::to_owned),
                grant_types: grants.clone(),
                response_types: responses.clone(),
                ..registration(&["https://client.example/callback"])
            });
            assert!(
                matches!(refused, Err(ClientError::Metadata(_))),
                "{method:?} {grants:?} {responses:?}"
            );
        }
    }

    #[test]
    fn a_client_id_names_its_registration_and_redirects_match_exactly() {
        let uris = ["https://client.example/callback", "http://127.0.0.1:9/cb"];
        let client = Client::register(registration(&uris)).unwrap();
        assert_eq!(Client::from_id(&client.id()), Ok(client.clone()));
        assert!(client.redirects_to("http://127.0.0.1:9/cb"));
        for other in [
            "http://127.0.0.1:10/cb",
            "https://client.example/callback/",
            "https://client.example/callback?x=1",
        ] {
            assert!(!client.redirects_to(other), "{other}");
        }
        let forged = Client {
            name: None,
            redirect_uris: vec!["http://attacker.example/".to_owned()],
        };
        assert!(Client::from_id(&forged.id()).is_err());
        assert!(Client::from_id("client_!!").is_err());
        assert!(Client::from_id("someone-else").is_err());
    }
}
