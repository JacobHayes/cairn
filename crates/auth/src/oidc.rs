//! The OIDC provider (H1: OAuth/OIDC against a registered provider; ARCHITECTURE, Auth):
//! an authorization-code sign-in with PKCE against the configured issuer, ending in a
//! browser session. Discovery, the code exchange, and every ID token check (issuer,
//! audience, expiry, signature, nonce) are `openidconnect`'s; nothing here validates a
//! token by hand.
//!
//! The sign-in's `state` is stored as its digest with the nonce and where to return, and
//! the PKCE verifier is the value of a cookie only the browser that started the sign-in
//! holds, stored as its digest beside the state. The callback needs both, so a sign-in
//! cannot be finished in another browser, and the verifier is never stored at all.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::header::LOCATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use cairn_schema::{Email, Login, Slug, Title};
use cairn_store::{OAuthStateKind, OAuthStateRecord};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreClientAuthMethod, CoreIdTokenClaims,
    CoreProviderMetadata,
};
use openidconnect::{
    AuthType, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::accounts::Accounts;
use crate::cookies::{self, SESSION_COOKIE};
use crate::error::{AuthError, ConfigError};
use crate::layer::Authenticate;
use crate::lifetimes::{BROWSER_STEP_LIFETIME, SESSION_LIFETIME};
use crate::outbound::{Outbound, is_trusted_origin};
use crate::provider::{AuthProvider, Presented, Verdict};
use crate::secret::{digest, secrets_equal};

/// The OIDC provider's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OidcConfig {
    /// The provider's name.
    pub name: Slug,
    /// The issuer: https, or http on a loopback address.
    pub issuer: Url,
    /// Cairn's client id at the issuer.
    pub client_id: String,
    /// Cairn's client secret at the issuer, for a confidential client.
    pub client_secret: Option<String>,
}

/// The OIDC provider.
pub struct OidcProvider {
    inner: Arc<Inner>,
}

struct Inner {
    config: OidcConfig,
    accounts: Accounts,
    http: Outbound,
    redirect: RedirectUrl,
    login_cookie: String,
    secure: bool,
}

/// What a sign-in in progress stores under its `state`'s digest.
#[derive(Serialize, Deserialize)]
struct Pending {
    provider: String,
    nonce: String,
    verifier_digest: String,
    return_to: String,
}

impl OidcProvider {
    /// The provider for a deployment at the public `base` URL.
    ///
    /// # Errors
    ///
    /// The issuer is neither https nor http on loopback, or the HTTP client cannot start.
    pub fn new(config: OidcConfig, accounts: Accounts, base: &Url) -> Result<Self, ConfigError> {
        let refuse = |reason: String| ConfigError {
            provider: config.name.to_string(),
            reason,
        };
        if !is_trusted_origin(&config.issuer) {
            return Err(refuse(
                "the issuer must be https, or http on loopback".to_owned(),
            ));
        }
        let callback = format!("api/auth/{}/callback", config.name);
        let redirect = base
            .join(&callback)
            .map_err(|error| refuse(error.to_string()))
            .map(RedirectUrl::from_url)?;
        let http = Outbound::new().map_err(refuse)?;
        let inner = Inner {
            login_cookie: format!("cairn_sign_in_{}", config.name),
            secure: base.scheme() == "https",
            config,
            accounts,
            http,
            redirect,
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
}

impl Inner {
    /// One discovery per sign-in step, so a rotated signing key needs nothing. Discovery,
    /// the key set, and the code exchange all go through `Outbound`, so an endpoint the
    /// issuer advertises over plain http is never called.
    async fn client(&self) -> Result<OidcClient, AuthError> {
        let issuer = IssuerUrl::from_url(self.config.issuer.clone());
        let metadata = CoreProviderMetadata::discover_async(issuer, &self.http)
            .await
            .map_err(|error| AuthError::Upstream(format!("discovery: {error}")))?;
        let auth_type = auth_type(&metadata);
        let secret = self.config.client_secret.clone().map(ClientSecret::new);
        let client_id = ClientId::new(self.config.client_id.clone());
        let client = CoreClient::from_provider_metadata(metadata, client_id, secret);
        Ok(client
            .set_auth_type(auth_type)
            .set_redirect_uri(self.redirect.clone()))
    }

    /// Starts a sign-in: stores its `state` and sends the browser to the issuer.
    async fn start(&self, return_to: &str) -> Result<Response, Failure> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_owned()))
            .add_scope(Scope::new("profile".to_owned()))
            .set_pkce_challenge(challenge)
            .url();
        if !is_trusted_origin(&url) {
            return Err(
                AuthError::Upstream(format!("{url} is neither https nor this machine")).into(),
            );
        }
        let pending = Pending {
            provider: self.config.name.to_string(),
            nonce: nonce.secret().clone(),
            verifier_digest: digest(verifier.secret()).to_string(),
            return_to: return_to.to_owned(),
        };
        let payload = serde_json::to_string(&pending).map_err(|error| Failure::bug(&error))?;
        let record = OAuthStateRecord {
            key_hash: digest(state.secret()),
            kind: OAuthStateKind::LoginState,
            payload: payload.parse().map_err(|error| Failure::bug(&error))?,
            expires_at: self.accounts.clock().after(BROWSER_STEP_LIFETIME),
        };
        self.accounts
            .store()
            .put_oauth_state(record)
            .await
            .map_err(AuthError::from)?;
        let path = format!("/api/auth/{}", self.config.name);
        let cookie = cookies::set(
            &self.login_cookie,
            verifier.secret(),
            &path,
            BROWSER_STEP_LIFETIME,
            self.secure,
        );
        Ok(redirect(url.as_str(), &[cookie]))
    }

    /// Finishes a sign-in: checks the state against this browser, exchanges the code, has
    /// the ID token verified, and signs the browser in.
    async fn finish(&self, headers: &HeaderMap, query: &Callback) -> Result<Response, Failure> {
        let (Some(code), Some(state)) = (&query.code, &query.state) else {
            return Err(Failure::Refused("the issuer did not sign in"));
        };
        let pending = self.take_pending(headers, state).await?;
        let verifier = cookies::read(headers, &self.login_cookie).unwrap_or_default();
        let client = self.client().await?;
        let exchange = client
            .exchange_code(AuthorizationCode::new(code.clone()))
            .map_err(|error| AuthError::Upstream(error.to_string()))?
            .set_pkce_verifier(PkceCodeVerifier::new(verifier));
        let tokens = exchange
            .request_async(&self.http)
            .await
            .map_err(|error| AuthError::Upstream(format!("code exchange: {error}")))?;
        let Some(id_token) = tokens.id_token() else {
            return Err(Failure::Refused("the issuer returned no ID token"));
        };
        let clock = self.accounts.clock().clone();
        let verifier = client
            .id_token_verifier()
            .set_time_fn(move || chrono_time(clock.now()));
        let nonce = Nonce::new(pending.nonce);
        let claims = id_token
            .claims(&verifier, &nonce)
            .map_err(|_| Failure::Refused("the ID token is not valid"))?;
        let login = self.login(claims)?;
        self.sign_in(headers, &login, &pending.return_to).await
    }

    /// The pending sign-in `state` names, which only the browser holding its verifier
    /// cookie may finish. Taken either way, so a state is used once.
    async fn take_pending(&self, headers: &HeaderMap, state: &str) -> Result<Pending, Failure> {
        let now = self.accounts.clock().now();
        let store = self.accounts.store();
        let record = store.take_oauth_state(&digest(state), now).await;
        let record = record.map_err(AuthError::from)?;
        let pending = record
            .filter(|record| record.kind == OAuthStateKind::LoginState)
            .and_then(|record| serde_json::from_str::<Pending>(record.payload.as_str()).ok())
            .filter(|pending| pending.provider == self.config.name.as_str());
        let Some(pending) = pending else {
            return Err(Failure::BadRequest("no sign-in is waiting for this state"));
        };
        let verifier = cookies::read(headers, &self.login_cookie).unwrap_or_default();
        if !secrets_equal(digest(&verifier).as_str(), &pending.verifier_digest) {
            return Err(Failure::BadRequest(
                "this sign-in was started in another browser",
            ));
        }
        Ok(pending)
    }

    /// The login an ID token's claims describe; its email only if the issuer marks it
    /// verified (H3).
    fn login(&self, claims: &CoreIdTokenClaims) -> Result<Login, Failure> {
        let subject: Title = claims
            .subject()
            .as_str()
            .parse()
            .map_err(|_| Failure::Refused("the subject is not a usable identifier"))?;
        let email = claims
            .email()
            .and_then(|email| email.as_str().parse::<Email>().ok());
        let verified = claims.email_verified() == Some(true);
        let verified_emails = email.filter(|_| verified).into_iter().collect();
        let name = claims
            .name()
            .and_then(|name| name.get(None))
            .and_then(|name| name.as_str().parse::<Title>().ok());
        Ok(Login {
            provider: self.config.name.clone(),
            subject,
            name,
            verified_emails,
        })
    }

    /// Resolves the login and starts a new session in place of any the browser had.
    async fn sign_in(
        &self,
        headers: &HeaderMap,
        login: &Login,
        return_to: &str,
    ) -> Result<Response, Failure> {
        let user = self.accounts.resolve(login).await?;
        if let Some(token) = cookies::read(headers, SESSION_COOKIE) {
            self.accounts.end_session(&token).await?;
        }
        let session = self.accounts.start_session(&user).await?;
        let path = format!("/api/auth/{}", self.config.name);
        let set = [
            cookies::set(
                SESSION_COOKIE,
                session.expose(),
                "/",
                SESSION_LIFETIME,
                self.secure,
            ),
            cookies::clear(&self.login_cookie, &path, self.secure),
        ];
        Ok(redirect(return_to, &set))
    }
}

/// How Cairn presents its client secret at the token endpoint: HTTP Basic, the default
/// when the issuer lists nothing (the discovery specification's default), unless it lists only
/// the form post.
fn auth_type(metadata: &CoreProviderMetadata) -> AuthType {
    let methods = metadata.token_endpoint_auth_methods_supported();
    let supports = |method: &CoreClientAuthMethod| methods.is_none_or(|all| all.contains(method));
    if !supports(&CoreClientAuthMethod::ClientSecretBasic)
        && supports(&CoreClientAuthMethod::ClientSecretPost)
    {
        AuthType::RequestBody
    } else {
        AuthType::BasicAuth
    }
}

type OidcClient = openidconnect::core::CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

/// `now` as the time `openidconnect` reads.
pub(crate) fn chrono_time(now: cairn_schema::Timestamp) -> chrono::DateTime<chrono::Utc> {
    let time =
        chrono::DateTime::from_timestamp(now.as_second(), now.subsec_nanosecond().unsigned_abs());
    time.unwrap_or_default()
}

/// Why a sign-in step failed, and the status that says so.
#[derive(Debug)]
enum Failure {
    /// The request is not a sign-in Cairn is waiting for.
    BadRequest(&'static str),
    /// The issuer did not vouch for anyone.
    Refused(&'static str),
    /// An operation failed.
    Auth(AuthError),
}

impl Failure {
    fn bug(error: &dyn std::fmt::Display) -> Self {
        Failure::Auth(AuthError::Upstream(error.to_string()))
    }
}

impl From<AuthError> for Failure {
    fn from(error: AuthError) -> Self {
        Failure::Auth(error)
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Failure::BadRequest(message) => (StatusCode::BAD_REQUEST, message.to_owned()),
            Failure::Refused(message) => (StatusCode::UNAUTHORIZED, message.to_owned()),
            Failure::Auth(error) => {
                let status = match error {
                    AuthError::NoVerifiedEmail => StatusCode::FORBIDDEN,
                    AuthError::Upstream(_) => StatusCode::BAD_GATEWAY,
                    _ => StatusCode::SERVICE_UNAVAILABLE,
                };
                (status, error.to_string())
            }
        };
        (status, message).into_response()
    }
}

/// A 303 to `location`, setting `cookies`.
fn redirect(location: &str, set: &[HeaderValue]) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(location) = HeaderValue::from_str(location) {
        response.headers_mut().insert(LOCATION, location);
    }
    for cookie in set {
        cookies::append(response.headers_mut(), cookie.clone());
    }
    response
}

/// Where a sign-in returns to: a path on this deployment, never another site.
pub(crate) fn local_path(return_to: Option<&str>) -> String {
    let path = return_to.unwrap_or("/");
    let local = path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && !path.chars().any(char::is_control);
    if local {
        path.to_owned()
    } else {
        "/".to_owned()
    }
}

#[derive(Deserialize)]
struct SignIn {
    return_to: Option<String>,
}

#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
}

async fn start(State(inner): State<Arc<Inner>>, Query(query): Query<SignIn>) -> Response {
    let return_to = local_path(query.return_to.as_deref());
    inner
        .start(&return_to)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn finish(
    State(inner): State<Arc<Inner>>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Response {
    inner
        .finish(&headers, &query)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

#[async_trait]
impl AuthProvider for OidcProvider {
    /// The OIDC provider signs a browser in through its routes; requests after that carry
    /// the session cookie, so a request on its own holds nothing of this provider's.
    async fn authenticate(&self, request: Presented<'_>) -> Verdict {
        let _ = request;
        Verdict::Absent
    }

    /// `GET /api/auth/<name>/sign-in?return_to=<path>` and the issuer's callback.
    fn router(&self, _authenticator: Arc<dyn Authenticate>) -> Option<Router> {
        let name = &self.inner.config.name;
        let router = Router::new()
            .route(&format!("/api/auth/{name}/sign-in"), get(start))
            .route(&format!("/api/auth/{name}/callback"), get(finish))
            .with_state(Arc::clone(&self.inner));
        Some(router)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_in_returns_only_to_a_path_on_this_deployment() {
        let cases = [
            (None, "/"),
            (Some("/journeys/j_1?tab=next"), "/journeys/j_1?tab=next"),
            (Some("https://elsewhere.example"), "/"),
            (Some("//elsewhere.example/"), "/"),
            (Some("/\\elsewhere.example"), "/"),
            (Some("journeys"), "/"),
            (Some("/a\nb"), "/"),
        ];
        for (return_to, expected) in cases {
            assert_eq!(local_path(return_to), expected, "{return_to:?}");
        }
    }
}
