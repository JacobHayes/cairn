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
use super::authorize::{AuthorizeQuery, Checked};
use super::client::{BUILT_IN, Client, Registration};
use super::metadata::AUTHORIZE;
use super::pkce::answers;
use super::reply::{escape, framed_never, no_store, oauth_error, page, redirect, registered};
use crate::error::{AuthError, Refusal};
use crate::layer::Authenticate;
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
    Consent { user: UserId, request: Checked },
    /// The user allowed the client; the code awaits its exchange.
    Code { user: UserId, request: Checked },
}

/// RFC 7591: registers a public client.
pub(super) async fn register(Json(registration): Json<Registration>) -> Response {
    registered(Client::register(registration, &BUILT_IN), &BUILT_IN)
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
    let (request, client) = match Checked::of(query, &flow.server.metadata, &BUILT_IN) {
        Ok(checked) => checked,
        Err(refused) => return *refused,
    };
    let user = match flow.browser_user(&headers, &extensions).await {
        Ok(Some(user)) => user,
        Ok(None) => return flow.sign_in_first(&uri),
        Err(response) => return *response,
    };
    flow.ask(user, request, &client)
        .await
        .unwrap_or_else(|error| failure(&error))
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
        Ok(Some(browser)) if browser == user => {}
        Ok(_) => return page(StatusCode::FORBIDDEN, "Sign in as the user who was asked."),
        Err(response) => return *response,
    }
    let back = request.back(&flow.server.metadata);
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
    /// The browser's user; none when it is not signed in. An agent, or any refused
    /// credential, is answered at once.
    async fn browser_user(
        &self,
        headers: &HeaderMap,
        extensions: &Extensions,
    ) -> Result<Option<UserId>, Box<Response>> {
        let found = self.authenticator.authenticate(headers, extensions).await;
        match found.map_err(|refusal| Box::new(refused(&refusal)))? {
            Some(Actor { user, agent: None }) => Ok(Some(user)),
            Some(_) => {
                let refused = page(StatusCode::FORBIDDEN, "An agent cannot authorize a client.");
                Err(Box::new(refused))
            }
            None => Ok(None),
        }
    }

    /// Sends the browser to sign in, returning to this very request.
    fn sign_in_first(&self, uri: &Uri) -> Response {
        match &self.server.config.sign_in_with {
            Some(provider) => {
                let path = uri.path_and_query().map_or("/", |path| path.as_str());
                let return_to: String =
                    url::form_urlencoded::byte_serialize(path.as_bytes()).collect();
                redirect(&format!(
                    "/api/auth/{provider}/sign-in?return_to={return_to}"
                ))
            }
            None => page(StatusCode::UNAUTHORIZED, "Sign in to Cairn first."),
        }
    }

    /// Stores the consent step and shows the page asking the user.
    async fn ask(
        &self,
        user: UserId,
        request: Checked,
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

    async fn issue_code(&self, user: UserId, request: Checked) -> Result<Secret, AuthError> {
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

fn refused(refusal: &Refusal) -> Response {
    let status = match refusal {
        Refusal::Credential => StatusCode::UNAUTHORIZED,
        Refusal::Peer | Refusal::NoVerifiedEmail => StatusCode::FORBIDDEN,
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
