//! The built-in server's endpoints: registration, authorization with consent, and the
//! token exchange (RFC 6749 section 4.1, RFC 7636, RFC 7591; I2).
//!
//! An authorization request is checked in this order: the client and its redirect URI
//! first, answered with an error page (a code or an error is never sent to a URI the
//! client did not register); then the request, answered at the redirect URI; then the
//! browser, which must be a signed-in user (an agent cannot authorize another). The user
//! is asked to allow the client on a page that cannot be framed, whose form carries a
//! single-use step secret stored by digest, so another site cannot post an approval. An
//! approval sends a single-use code, stored by digest for a minute with the PKCE
//! challenge, which the token endpoint exchanges, given the matching verifier, for an agent
//! token acting for the user.

use std::sync::Arc;

use axum::extract::{Form, Query, State};
use axum::http::{Extensions, HeaderMap, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Json, Response};
use cairn_schema::{Actor, Title, UserId};
use cairn_store::{OAuthStateKind, OAuthStateRecord};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use super::Server;
use super::client::{BUILT_IN, Client, Registration};
use super::metadata::AUTHORIZE;
use super::pkce::{answers, is_challenge};
use super::reply::{Back, escape, framed_never, no_store, oauth_error, page, redirect, registered};
use crate::cookies::{self, SESSION_COOKIE};
use crate::error::{AuthError, Refusal};
use crate::layer::{Authenticate, browser};
use crate::lifetimes::{AUTHORIZATION_CODE_LIFETIME, BROWSER_STEP_LIFETIME};
use crate::secret::{Secret, SecretKind, digest};

/// What the endpoints share.
pub(super) struct Flow {
    pub server: Arc<Server>,
    pub authenticator: Arc<dyn Authenticate>,
}

/// A pending step of the flow, stored under its secret's digest.
#[derive(Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case", deny_unknown_fields)]
enum Pending {
    /// The user was shown the consent page.
    Consent { user: UserId, request: Approved },
    /// The user allowed the client; the code awaits its exchange.
    Code { user: UserId, request: Approved },
}

/// An authorization request that passed every check before the user's answer.
#[derive(Clone, Serialize, Deserialize)]
struct Approved {
    client_id: String,
    redirect_uri: String,
    challenge: String,
    state: Option<String>,
}

/// RFC 7591: registers a public client.
pub(super) async fn register(Json(registration): Json<Registration>) -> Response {
    registered(Client::register(registration, &BUILT_IN), &BUILT_IN)
}

#[derive(Deserialize)]
pub(super) struct AuthorizeQuery {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

/// RFC 6749 section 4.1.1: checks the request, then shows a signed-in user the consent
/// page, or sends the browser to sign in first.
pub(super) async fn authorize(
    State(flow): State<Arc<Flow>>,
    headers: HeaderMap,
    extensions: Extensions,
    uri: Uri,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    let client_id = query.client_id.clone().unwrap_or_default();
    let Ok(client) = Client::from_id(&client_id, &BUILT_IN) else {
        return page(
            StatusCode::BAD_REQUEST,
            "This client is not registered here.",
        );
    };
    let redirect_uri = query.redirect_uri.clone().unwrap_or_default();
    if !client.redirects_to(&redirect_uri, &BUILT_IN) {
        return page(
            StatusCode::BAD_REQUEST,
            "The redirect URI is not the client's.",
        );
    }
    let back = Back {
        issuer: flow.server.metadata.issuer(),
        redirect_uri: &redirect_uri,
        state: query.state.as_deref(),
    };
    if query.response_type.as_deref() != Some("code") {
        return back.error("unsupported_response_type");
    }
    let challenge = query.code_challenge.clone().unwrap_or_default();
    if query.code_challenge_method.as_deref() != Some("S256") || !is_challenge(&challenge) {
        return back.error("invalid_request");
    }
    let (user, lapsed) = match flow.browser_user(&headers, &extensions).await {
        Ok((Some(user), lapsed)) => (user, lapsed),
        Ok((None, lapsed)) => return flow.sign_in_first(lapsed, &uri),
        Err(response) => return *response,
    };
    let request = Approved {
        client_id,
        redirect_uri,
        challenge,
        state: query.state,
    };
    let shown = flow.ask(user, request, &client).await;
    flow.clear(lapsed, shown.unwrap_or_else(|error| failure(&error)))
}

#[derive(Deserialize)]
pub(super) struct ConsentForm {
    step: String,
    decision: String,
}

/// The consent page's answer: the step must be the one this browser's user was shown.
pub(super) async fn consent(
    State(flow): State<Arc<Flow>>,
    headers: HeaderMap,
    extensions: Extensions,
    Form(form): Form<ConsentForm>,
) -> Response {
    let pending = flow.take(&form.step, OAuthStateKind::LoginState).await;
    let (user, request) = match pending {
        Ok(Some(Pending::Consent { user, request })) => (user, request),
        Ok(_) => return page(StatusCode::BAD_REQUEST, "This request has expired."),
        Err(error) => return failure(&error),
    };
    match flow.browser_user(&headers, &extensions).await {
        Ok((Some(browser), _)) if browser == user => {}
        Ok(_) => return page(StatusCode::FORBIDDEN, "Sign in as the user who was asked."),
        Err(response) => return *response,
    }
    let back = Back {
        issuer: flow.server.metadata.issuer(),
        redirect_uri: &request.redirect_uri,
        state: request.state.as_deref(),
    };
    if form.decision != "allow" {
        return back.error("access_denied");
    }
    match flow.issue_code(user, request.clone()).await {
        Ok(code) => back.send(&[("code", code.expose())]),
        Err(error) => failure(&error),
    }
}

#[derive(Deserialize)]
pub(super) struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    code_verifier: Option<String>,
}

/// RFC 6749 section 4.1.3 and RFC 7636: exchanges a code, once, for an agent token acting
/// for the user who allowed the client (H2).
pub(super) async fn token(
    State(flow): State<Arc<Flow>>,
    Form(request): Form<TokenRequest>,
) -> Response {
    if request.grant_type.as_deref() != Some("authorization_code") {
        let only = "only authorization_code";
        return oauth_error(StatusCode::BAD_REQUEST, "unsupported_grant_type", only);
    }
    let code = request.code.unwrap_or_default();
    let pending = flow.take(&code, OAuthStateKind::AuthorizationCode).await;
    let (user, approved) = match pending {
        Ok(Some(Pending::Code { user, request })) => (user, request),
        Ok(_) => {
            let unknown = "unknown, used, or expired code";
            return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", unknown);
        }
        Err(error) => return failure(&error),
    };
    let verifier = request.code_verifier.unwrap_or_default();
    let matches = request.client_id.as_deref() == Some(approved.client_id.as_str())
        && request.redirect_uri.as_deref() == Some(approved.redirect_uri.as_str())
        && answers(&verifier, &approved.challenge);
    if !matches {
        let differ = "the client, redirect URI, or verifier differ";
        return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", differ);
    }
    match flow.mint(user, &approved.client_id).await {
        Ok(token) => {
            let body = json!({"access_token": token.expose(), "token_type": "Bearer"});
            no_store(Json(body).into_response())
        }
        Err(error) => failure(&error),
    }
}

impl Flow {
    /// The browser's user, past a lapsed session cookie (which the answer must clear);
    /// none when it is not signed in. An agent, or any other refused credential, is
    /// answered at once.
    async fn browser_user(
        &self,
        headers: &HeaderMap,
        extensions: &Extensions,
    ) -> Result<(Option<UserId>, Lapsed), Box<Response>> {
        let found = browser(self.authenticator.as_ref(), headers, extensions).await;
        let found = found.map_err(|refusal| Box::new(refused(&refusal)))?;
        let lapsed = Lapsed(found.stale_session);
        match found.actor {
            Some(Actor { user, agent: None }) => Ok((Some(user), lapsed)),
            Some(_) => {
                let refused = page(StatusCode::FORBIDDEN, "An agent cannot authorize a client.");
                Err(Box::new(refused))
            }
            None => Ok((None, lapsed)),
        }
    }

    /// Sends the browser to sign in, returning to this very request, and clears the
    /// session cookie it carried, which did not sign it in.
    fn sign_in_first(&self, lapsed: Lapsed, uri: &Uri) -> Response {
        let response = match &self.server.config.sign_in_with {
            Some(provider) => {
                let path = uri.path_and_query().map_or("/", |path| path.as_str());
                let return_to: String =
                    url::form_urlencoded::byte_serialize(path.as_bytes()).collect();
                redirect(&format!(
                    "/api/auth/{provider}/sign-in?return_to={return_to}"
                ))
            }
            None => page(StatusCode::UNAUTHORIZED, "Sign in to Cairn first."),
        };
        self.clear(lapsed, response)
    }

    /// Clears a lapsed session cookie in `response`.
    fn clear(&self, lapsed: Lapsed, mut response: Response) -> Response {
        if lapsed.0 {
            let secure = self.server.metadata.issuer().starts_with("https://");
            let cleared = cookies::clear(SESSION_COOKIE, "/", secure);
            cookies::append(response.headers_mut(), cleared);
        }
        response
    }

    /// Stores the consent step and shows the page asking the user.
    async fn ask(
        &self,
        user: UserId,
        request: Approved,
        client: &Client,
    ) -> Result<Response, AuthError> {
        let step = Secret::mint(SecretKind::Step)?;
        let pending = Pending::Consent {
            user,
            request: request.clone(),
        };
        let kind = OAuthStateKind::LoginState;
        self.store(&step, kind, &pending, BROWSER_STEP_LIFETIME)
            .await?;
        let name = client
            .name
            .as_ref()
            .map_or("An unnamed client", Title::as_str);
        Ok(consent_page(name, &request.redirect_uri, step.expose()))
    }

    async fn issue_code(&self, user: UserId, request: Approved) -> Result<Secret, AuthError> {
        let code = Secret::mint(SecretKind::Code)?;
        let pending = Pending::Code { user, request };
        let kind = OAuthStateKind::AuthorizationCode;
        self.store(&code, kind, &pending, AUTHORIZATION_CODE_LIFETIME)
            .await?;
        Ok(code)
    }

    async fn store(
        &self,
        secret: &Secret,
        kind: OAuthStateKind,
        pending: &Pending,
        lifetime: std::time::Duration,
    ) -> Result<(), AuthError> {
        let accounts = &self.server.accounts;
        let payload = serde_json::to_string(pending)
            .ok()
            .and_then(|json| json.parse().ok())
            .ok_or_else(|| AuthError::Upstream("the request is too large to hold".to_owned()))?;
        let record = OAuthStateRecord {
            key_hash: secret.digest(),
            kind,
            payload,
            expires_at: accounts.clock().after(lifetime),
        };
        Ok(accounts.store().put_oauth_state(record).await?)
    }

    /// Takes a pending step of `kind` by its secret: used once, whatever comes of it.
    async fn take(&self, secret: &str, kind: OAuthStateKind) -> Result<Option<Pending>, AuthError> {
        let expected = match kind {
            OAuthStateKind::AuthorizationCode => SecretKind::Code,
            _ => SecretKind::Step,
        };
        if !expected.matches(secret) {
            return Ok(None);
        }
        let accounts = &self.server.accounts;
        let now = accounts.clock().now();
        let record = accounts
            .store()
            .take_oauth_state(&digest(secret), now)
            .await?;
        let record = record.filter(|record| record.kind == kind);
        Ok(record.and_then(|record| serde_json::from_str(record.payload.as_str()).ok()))
    }

    /// The agent token for the client, named after it.
    async fn mint(&self, user: UserId, client_id: &str) -> Result<Secret, AuthError> {
        let name = Client::from_id(client_id, &BUILT_IN)
            .ok()
            .and_then(|client| client.name);
        let name = name.map_or_else(|| "An MCP client".parse::<Title>(), Ok);
        let name = name.map_err(|error| AuthError::Upstream(error.to_string()))?;
        let actor = Actor { user, agent: None };
        let minted = self.server.accounts.mint_token(&actor, name).await?;
        Ok(minted.token)
    }
}

/// Whether the request carried a session cookie that has lapsed.
#[derive(Clone, Copy)]
struct Lapsed(bool);

fn refused(refusal: &Refusal) -> Response {
    let status = match refusal {
        Refusal::Credential => StatusCode::UNAUTHORIZED,
        Refusal::Peer => StatusCode::FORBIDDEN,
        Refusal::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
    };
    page(status, &refusal.to_string())
}

fn failure(error: &AuthError) -> Response {
    page(StatusCode::SERVICE_UNAVAILABLE, &error.to_string())
}

fn consent_page(client: &str, redirect_uri: &str, step: &str) -> Response {
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Allow access to Cairn</title>\
         <h1>Allow {client} to act for you in Cairn?</h1>\
         <p>It will be able to read and change everything you can, until you revoke it. \
         Its answer goes to <code>{redirect_uri}</code>.</p>\
         <form method=\"post\" action=\"{AUTHORIZE}\">\
         <input type=\"hidden\" name=\"step\" value=\"{step}\">\
         <button name=\"decision\" value=\"allow\">Allow</button> \
         <button name=\"decision\" value=\"deny\">Deny</button></form>",
        client = escape(client),
        redirect_uri = escape(redirect_uri),
        step = escape(step),
    );
    let answer_to = Url::parse(redirect_uri).ok();
    framed_never(
        (StatusCode::OK, Html(body)).into_response(),
        answer_to.as_ref(),
    )
}
