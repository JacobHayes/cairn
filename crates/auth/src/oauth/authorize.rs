//! The part of an authorization request both OAuth servers check the same way (RFC 6749
//! section 4.1.1, RFC 7636, RFC 8707; I2): the client and its redirect URI first, answered
//! with a page (a code or an error is never sent to a URI the client did not register), then
//! the rest of the request, answered at the redirect URI.

use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use super::client::{Client, Policy};
use super::metadata::Metadata;
use super::pkce::is_challenge;
use super::reply::{Back, page};

/// An authorization request's query.
#[derive(Deserialize)]
pub(crate) struct AuthorizeQuery {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    resource: Option<String>,
}

/// An authorization request that passed every check before the user's answer.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Checked {
    pub(crate) client_id: String,
    pub(crate) redirect_uri: String,
    pub(crate) challenge: String,
    pub(crate) state: Option<String>,
}

impl Checked {
    /// The checked request, with the client it names, or the answer that refuses it.
    pub(crate) fn of(
        query: AuthorizeQuery,
        metadata: &Metadata,
        policy: &Policy,
    ) -> Result<(Self, Client), Box<Response>> {
        let client_id = query.client_id.unwrap_or_default();
        let Ok(client) = Client::from_id(&client_id, policy) else {
            let unknown = "This client is not registered here.";
            return Err(Box::new(page(StatusCode::BAD_REQUEST, unknown)));
        };
        let redirect_uri = query.redirect_uri.unwrap_or_default();
        if !client.redirects_to(&redirect_uri, policy) {
            let foreign = "The redirect URI is not the client's.";
            return Err(Box::new(page(StatusCode::BAD_REQUEST, foreign)));
        }
        let back = Back {
            issuer: metadata.issuer(),
            redirect_uri: &redirect_uri,
            state: query.state.as_deref(),
        };
        if query.response_type.as_deref() != Some("code") {
            return Err(Box::new(back.error("unsupported_response_type")));
        }
        let challenge = query.code_challenge.unwrap_or_default();
        if query.code_challenge_method.as_deref() != Some("S256") || !is_challenge(&challenge) {
            return Err(Box::new(back.error("invalid_request")));
        }
        let own = query
            .resource
            .as_deref()
            .is_none_or(|resource| metadata.is_own_resource(resource));
        if !own {
            return Err(Box::new(back.error("invalid_target")));
        }
        let checked = Self {
            client_id,
            redirect_uri,
            challenge,
            state: query.state,
        };
        Ok((checked, client))
    }

    /// Where this request's answer goes back.
    pub(crate) fn back<'a>(&'a self, metadata: &'a Metadata) -> Back<'a> {
        Back {
            issuer: metadata.issuer(),
            redirect_uri: &self.redirect_uri,
            state: self.state.as_deref(),
        }
    }
}
