//! The IAP provider's OAuth relay (I2): an MCP client behind Google Cloud IAP signs in with
//! Google through the standard MCP handshake, and the bearer it ends up holding is
//! Google's ID token, which IAP's programmatic access accepts. Google stays the only
//! issuer and IAP keeps deciding, on every MCP call, who may reach Cairn; the relay only
//! carries the handshake. It mints nothing and stores nothing: Google's refresh token
//! lives with the client.
//!
//! The handshake, against Cairn's own metadata so the client hears Cairn as the issuer
//! (RFC 9207):
//!
//! - Registration is the shared one under the relay's policy: loopback redirects only.
//! - Authorize checks the client and its redirect URI, then sends the browser to Google
//!   with Cairn's callback, the client's PKCE challenge, and the client's request carried
//!   in Google's `state`. A cookie holding the state's digest binds that state to the
//!   browser, so the round trip cannot be altered to send a code anywhere else.
//! - The callback checks the cookie and the client's redirect URI again, and forwards
//!   Google's code to the client with the client's `state` and Cairn's `iss`; Google's own
//!   `iss` never reaches the client.
//! - The token endpoint exchanges the code (with the client's PKCE verifier, which Google
//!   checks) or a refresh token at Google, adding the client secret, which never leaves
//!   the server, and answers with Google's ID token as the access token. No ID token, no
//!   success.
//!
//! `resource` (RFC 8707) is checked here against the deployment and never sent to Google.
//! Every request to these endpoints ends within `IDENTITY_CALL_DURATION_MAX`, its body
//! read included.

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Query, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cairn_schema::{Limit, Slug};
use openidconnect::core::{
    CoreClient, CoreErrorResponseType, CoreIdToken, CoreJsonWebKeySet, CoreTokenResponse,
};
use openidconnect::{
    AuthType, AuthorizationCode, ClientId, ClientSecret, EndpointNotSet, EndpointSet, IssuerUrl,
    OAuth2TokenResponse, PkceCodeVerifier, RedirectUrl, RefreshToken, RequestTokenError,
    TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use super::{GoogleEndpoints, RelayClient};
use crate::cookies;
use crate::lifetimes::BROWSER_STEP_LIFETIME;
use crate::limits::IDENTITY_CALL_DURATION_MAX;
use crate::oauth::client::{Client, RELAY, Registration};
use crate::oauth::metadata::{AUTHORIZE, Metadata, REGISTER, TOKEN};
use crate::oauth::pkce::{is_challenge, is_verifier};
use crate::oauth::reply::{Back, no_store, oauth_error, page, redirect, registered};
use crate::outbound::Outbound;
use crate::secret::{digest, random_hex, secrets_equal};

/// Where Google sends the browser back: behind IAP, like authorize.
const CALLBACK: &str = "/api/oauth/callback";
/// Google's issuer, which its ID tokens name.
const GOOGLE_ISSUER: &str = "https://accounts.google.com";
/// Random bytes in each attempt's id, so no two attempts share a state or a cookie.
const ATTEMPT_BYTES: usize = 16;

/// Google's client as the relay uses it: only the token endpoint is called through it (the
/// authorization URL is built by hand on the fixed endpoint).
type GoogleClient =
    CoreClient<EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// The relay.
pub(super) struct Relay {
    /// The Google client's id: the audience of the ID tokens it hands out.
    client_id: String,
    metadata: Arc<Metadata>,
    base: Url,
    callback: Url,
    authorization: Url,
    google: GoogleClient,
    http: Outbound,
    cookie: String,
    secure: bool,
}

impl Relay {
    /// The relay of the provider `name` for a deployment at `base`, calling `endpoints`.
    pub(super) fn new(
        name: &Slug,
        client: &RelayClient,
        base: &Url,
        endpoints: &GoogleEndpoints,
        http: Outbound,
    ) -> Result<Self, String> {
        for (field, value) in [
            ("client_id", &client.client_id),
            ("client_secret", &client.client_secret),
        ] {
            if value.is_empty() || value.trim() != value {
                return Err(format!("the relay's {field} is empty or padded"));
            }
        }
        let callback = base
            .join(CALLBACK.trim_start_matches('/'))
            .map_err(|error| error.to_string())?;
        let issuer = IssuerUrl::new(GOOGLE_ISSUER.to_owned()).map_err(|error| error.to_string())?;
        let google = CoreClient::new(
            ClientId::new(client.client_id.clone()),
            issuer,
            CoreJsonWebKeySet::default(),
        )
        .set_client_secret(ClientSecret::new(client.client_secret.clone()))
        .set_token_uri(TokenUrl::from_url(endpoints.token.clone()))
        .set_redirect_uri(RedirectUrl::from_url(callback.clone()))
        .set_auth_type(AuthType::RequestBody);
        Ok(Self {
            client_id: client.client_id.clone(),
            metadata: Arc::new(Metadata::new(base, &RELAY)),
            base: base.clone(),
            callback,
            authorization: endpoints.authorization.clone(),
            google,
            http,
            cookie: format!("cairn_relay_{name}"),
            secure: base.scheme() == "https",
        })
    }

    /// RFC 8707: a resource is this deployment (any path on its origin, as the protected
    /// resource metadata serves every path), never a query or a fragment.
    fn is_own_resource(&self, resource: &str) -> bool {
        Url::parse(resource).is_ok_and(|url| {
            url.origin() == self.base.origin() && url.query().is_none() && url.fragment().is_none()
        })
    }

    /// Google's authorization URL for an attempt carried in `state`, with the client's
    /// PKCE challenge, which Google checks at the exchange. `offline` and `consent` make
    /// Google issue a refresh token.
    fn google_authorization(&self, state: &str, challenge: &str) -> Url {
        let mut url = self.authorization.clone();
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", self.callback.as_str())
            .append_pair("scope", "openid email")
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
        url
    }
}

/// Whether `token` has the shape of an ID token (a JWT with an ID token's claims). It is
/// not verified here: IAP verified it, and the assertion beside it names the user.
pub(super) fn is_id_token(token: &str) -> bool {
    token.parse::<CoreIdToken>().is_ok()
}

/// The relay's routes: the metadata, registration, authorize, the callback, and the token
/// endpoint, each request ending within `IDENTITY_CALL_DURATION_MAX` and its body within
/// `BODY_BYTES_MAX` (these public endpoints take only small forms and JSON).
pub(super) fn router(relay: &Arc<Relay>) -> Router {
    let endpoints = Router::new()
        .route(REGISTER, post(register))
        .route(AUTHORIZE, get(authorize))
        .route(CALLBACK, get(callback))
        .route(TOKEN, post(token))
        .layer(DefaultBodyLimit::max(
            usize::try_from(Limit::BodyBytes.max()).unwrap_or(usize::MAX),
        ))
        .with_state(Arc::clone(relay));
    endpoints
        .merge(relay.metadata.router())
        .layer(middleware::from_fn(deadline))
}

/// Auth's routes sit outside the API's request duration, and a body that trickles in or a
/// Google that does not answer would otherwise hold the request open: one deadline covers
/// the whole request (PRACTICES, Explicit limits: outbound identity call).
async fn deadline(request: Request, next: Next) -> Response {
    match tokio::time::timeout(IDENTITY_CALL_DURATION_MAX, next.run(request)).await {
        Ok(response) => response,
        Err(_) => oauth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "temporarily_unavailable",
            "the sign-in took too long",
        ),
    }
}

/// RFC 7591, under the relay's policy.
async fn register(Json(registration): Json<Registration>) -> Response {
    registered(Client::register(registration, &RELAY), &RELAY)
}

/// The client's request, carried through Google in its `state` and back.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Carried {
    client_id: String,
    redirect_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    /// This attempt's own random id.
    attempt: String,
}

impl Carried {
    fn encode(&self) -> String {
        match serde_json::to_vec(self) {
            Ok(json) => URL_SAFE_NO_PAD.encode(json),
            Err(error) => unreachable!("strings serialize: {error}"),
        }
    }

    fn decode(state: &str) -> Option<Self> {
        let json = URL_SAFE_NO_PAD.decode(state).ok()?;
        serde_json::from_slice(&json).ok()
    }
}

#[derive(Deserialize)]
struct AuthorizeQuery {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    resource: Option<String>,
}

/// I2, H1: RFC 6749 section 4.1.1: the client and its redirect URI first (answered with a page,
/// never sent to a URI the client did not register), then the request (answered at the
/// redirect URI), then on to Google with a cookie binding the attempt to this browser.
async fn authorize(
    State(relay): State<Arc<Relay>>,
    Query(query): Query<AuthorizeQuery>,
) -> Response {
    let client_id = query.client_id.unwrap_or_default();
    let Ok(client) = Client::from_id(&client_id, &RELAY) else {
        let unknown = "This client is not registered here.";
        return page(StatusCode::BAD_REQUEST, unknown);
    };
    let redirect_uri = query.redirect_uri.unwrap_or_default();
    if !client.redirects_to(&redirect_uri, &RELAY) {
        let foreign = "The redirect URI is not the client's.";
        return page(StatusCode::BAD_REQUEST, foreign);
    }
    let back = Back {
        issuer: relay.metadata.issuer(),
        redirect_uri: &redirect_uri,
        state: query.state.as_deref(),
    };
    if query.response_type.as_deref() != Some("code") {
        return back.error("unsupported_response_type");
    }
    let challenge = query.code_challenge.unwrap_or_default();
    if query.code_challenge_method.as_deref() != Some("S256") || !is_challenge(&challenge) {
        return back.error("invalid_request");
    }
    if query
        .resource
        .as_deref()
        .is_some_and(|resource| !relay.is_own_resource(resource))
    {
        return back.error("invalid_target");
    }
    let Ok(attempt) = random_hex(ATTEMPT_BYTES) else {
        return back.error("temporarily_unavailable");
    };
    let carried = Carried {
        client_id,
        redirect_uri: redirect_uri.clone(),
        state: query.state.clone(),
        attempt,
    };
    let state = carried.encode();
    let google = relay.google_authorization(&state, &challenge);
    if Limit::LinkBytes.check(google.as_str().len()).is_err() {
        return back.error("invalid_request");
    }
    let mut response = redirect(google.as_str());
    let bound = digest(&state);
    let cookie = cookies::set(
        &relay.cookie,
        bound.as_str(),
        CALLBACK,
        BROWSER_STEP_LIFETIME,
        relay.secure,
    );
    cookies::append(response.headers_mut(), cookie);
    response
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// Google's answer, forwarded to the client the browser's own attempt named, with the
/// client's `state` and Cairn as the issuer. A callback that is not the browser's current
/// attempt is refused and leaves that attempt's cookie alone; the current one clears it,
/// whatever Google answered.
async fn callback(
    State(relay): State<Arc<Relay>>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let current = query.state.as_deref().filter(|state| {
        cookies::read(&headers, &relay.cookie)
            .is_some_and(|bound| secrets_equal(digest(state).as_str(), &bound))
    });
    let Some(state) = current else {
        return refused();
    };
    let mut response = forward(&relay, state, &query);
    let cleared = cookies::clear(&relay.cookie, CALLBACK, relay.secure);
    cookies::append(response.headers_mut(), cleared);
    response
}

/// A callback this browser did not start, or whose round trip was altered.
fn refused() -> Response {
    page(
        StatusCode::BAD_REQUEST,
        "This sign-in was not started here.",
    )
}

/// The client's answer for the browser's current attempt, carried in `state`.
fn forward(relay: &Relay, state: &str, query: &CallbackQuery) -> Response {
    let Some(carried) = Carried::decode(state) else {
        return refused();
    };
    let registered = Client::from_id(&carried.client_id, &RELAY)
        .is_ok_and(|client| client.redirects_to(&carried.redirect_uri, &RELAY));
    if !registered {
        return refused();
    }
    let back = Back {
        issuer: relay.metadata.issuer(),
        redirect_uri: &carried.redirect_uri,
        state: carried.state.as_deref(),
    };
    match (&query.code, query.error.as_deref()) {
        (_, Some("access_denied")) => back.error("access_denied"),
        (Some(code), None) if !code.is_empty() => back.send(&[("code", code)]),
        _ => back.error("server_error"),
    }
}

#[derive(Deserialize)]
struct TokenForm {
    grant_type: Option<String>,
    code: Option<String>,
    code_verifier: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    refresh_token: Option<String>,
    resource: Option<String>,
}

/// RFC 6749 sections 4.1.3 and 6: a code or a refresh token, exchanged at Google with the
/// client secret, answered with Google's ID token as the access token.
async fn token(State(relay): State<Arc<Relay>>, Form(form): Form<TokenForm>) -> Response {
    let refuse =
        |error: &str, description: &str| oauth_error(StatusCode::BAD_REQUEST, error, description);
    let client_id = form.client_id.as_deref().unwrap_or_default();
    let Ok(client) = Client::from_id(client_id, &RELAY) else {
        return refuse("invalid_client", "unknown client_id");
    };
    if form
        .resource
        .as_deref()
        .is_some_and(|resource| !relay.is_own_resource(resource))
    {
        return refuse("invalid_target", "the resource is not this deployment");
    }
    let exchanged = match form.grant_type.as_deref() {
        Some("authorization_code") => {
            let code = form.code.unwrap_or_default();
            let verifier = form.code_verifier.unwrap_or_default();
            let redirect_uri = form.redirect_uri.unwrap_or_default();
            if code.is_empty()
                || !is_verifier(&verifier)
                || !client.redirects_to(&redirect_uri, &RELAY)
            {
                return refuse("invalid_grant", "the code, verifier, or redirect URI");
            }
            let request = relay.google.exchange_code(AuthorizationCode::new(code));
            let request = request.set_pkce_verifier(PkceCodeVerifier::new(verifier));
            request.request_async(&relay.http).await
        }
        Some("refresh_token") => {
            let Some(refresh) = form.refresh_token.filter(|token| !token.is_empty()) else {
                return refuse("invalid_request", "a refresh_token");
            };
            let refresh = RefreshToken::new(refresh);
            let request = relay.google.exchange_refresh_token(&refresh);
            request.request_async(&relay.http).await
        }
        _ => {
            return refuse(
                "unsupported_grant_type",
                "authorization_code or refresh_token",
            );
        }
    };
    answer(exchanged)
}

type Exchanged = Result<
    CoreTokenResponse,
    RequestTokenError<
        crate::outbound::OutboundError,
        openidconnect::StandardErrorResponse<CoreErrorResponseType>,
    >,
>;

/// The client's answer to Google's: the ID token as the access token, or an error. Google's
/// access token is never handed back, and Google's own error text is not passed on.
fn answer(exchanged: Exchanged) -> Response {
    let failed = || {
        let description = "Google did not issue an ID token";
        oauth_error(StatusCode::BAD_GATEWAY, "server_error", description)
    };
    let tokens = match exchanged {
        Ok(tokens) => tokens,
        Err(RequestTokenError::ServerResponse(error))
            if *error.error() == CoreErrorResponseType::InvalidGrant =>
        {
            let description = "Google refused the code or refresh token";
            return oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", description);
        }
        Err(_) => return failed(),
    };
    let Some(id_token) = tokens.id_token() else {
        return failed();
    };
    let mut body = json!({
        "access_token": id_token.to_string(),
        "token_type": "Bearer",
    });
    if let Some(expires_in) = tokens.expires_in() {
        body["expires_in"] = json!(expires_in.as_secs());
    }
    if let Some(refresh) = tokens.refresh_token() {
        body["refresh_token"] = json!(refresh.secret());
    }
    no_store(Json(body).into_response())
}
