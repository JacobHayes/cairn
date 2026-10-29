//! The auth layer the HTTP API and MCP endpoint sit behind (ARCHITECTURE, Auth; I2), and
//! the routes auth serves outside it: each provider's own, and sign-out.
//!
//! A request is authenticated by its session cookie and by every configured provider. Any
//! provider's refusal refuses the request, so a credential that fails never falls through
//! to another provider (3.2, Security); otherwise the session, then the first provider in
//! configured order with a credential, names the actor. A session cookie that is no live
//! session is no credential: it names no one, and the answer clears it. Handlers behind the layer read it as an
//! `Extension<Actor>`.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{Extensions, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use cairn_schema::Actor;
use url::Url;

use crate::accounts::Accounts;
use crate::cookies::{self, SESSION_COOKIE};
use crate::error::Refusal;
use crate::provider::{AuthProvider, Presented, Verdict};

/// Authenticates a request the way the layer does; what the built-in OAuth server's consent
/// uses to learn who the browser is.
#[async_trait]
pub trait Authenticate: Send + Sync {
    /// The actor a request's credentials name; none when it carries none.
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        extensions: &Extensions,
    ) -> Result<Option<Actor>, Refusal>;
}

/// The deployment's auth: accounts and the configured providers.
#[derive(Clone)]
pub struct Auth {
    inner: Arc<Inner>,
}

struct Inner {
    accounts: Accounts,
    providers: Vec<Arc<dyn AuthProvider>>,
    secure: bool,
}

impl Auth {
    /// Auth for a server at the public `base` URL, asking `providers` in this order.
    #[must_use]
    pub fn new(accounts: Accounts, base: &Url, providers: Vec<Arc<dyn AuthProvider>>) -> Self {
        let inner = Inner {
            accounts,
            providers,
            secure: base.scheme() == "https",
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    /// The accounts.
    #[must_use]
    pub fn accounts(&self) -> &Accounts {
        &self.inner.accounts
    }

    /// `router` behind the auth layer: every request reaches it with an `Actor` extension,
    /// or is answered 401, 403, or 503 without reaching it. The server must record peers
    /// (axum's `into_make_service_with_connect_info::<SocketAddr>`): a request with no
    /// recorded peer is never local, so the providers that need one refuse it.
    pub fn protect(&self, router: Router) -> Router {
        router.layer(middleware::from_fn_with_state(self.clone(), require_actor))
    }

    /// The routes auth serves outside the layer: every provider's own, and sign-out.
    pub fn router(&self) -> Router {
        let authenticator: Arc<dyn Authenticate> = Arc::new(self.clone());
        let sign_out = Router::new()
            .route("/api/auth/sign-out", post(sign_out))
            .with_state(self.clone());
        let routers = self.inner.providers.iter();
        routers
            .filter_map(|provider| provider.router(Arc::clone(&authenticator)))
            .fold(sign_out, Router::merge)
    }

    /// The session cookie's user; none for no cookie or one that is not a live session.
    async fn session_actor(&self, headers: &HeaderMap) -> Result<Option<Actor>, Refusal> {
        let Some(token) = cookies::read(headers, SESSION_COOKIE) else {
            return Ok(None);
        };
        let user = self.inner.accounts.session_user(&token).await?;
        Ok(user.map(|user| Actor { user, agent: None }))
    }

    /// An `Authorization` header must hold a bearer token some provider claims: anything
    /// else is a credential that cannot be checked, so it is refused rather than ignored.
    /// Blank values hold no credential and count as absent: IAP passes on the header it took
    /// a Google ID token from, emptied.
    fn check_authorization(&self, request: Presented<'_>) -> Result<(), Refusal> {
        let mut values = request.headers.get_all(AUTHORIZATION).iter();
        if values.all(|value| value.as_bytes().trim_ascii().is_empty()) {
            return Ok(());
        }
        let claimed = request.bearer().is_some_and(|token| {
            let mut providers = self.inner.providers.iter();
            providers.any(|provider| provider.claims_bearer(token))
        });
        if claimed {
            Ok(())
        } else {
            Err(Refusal::Credential)
        }
    }

    /// 401 with the `Bearer` challenge, naming the resource metadata where the built-in
    /// OAuth server serves it (I2), and clearing a session cookie that failed.
    fn challenge(&self, path: &str, error: Option<&str>, clear_session: bool) -> Response {
        let mut challenge = String::from("Bearer");
        let mut parameters: Vec<String> = error
            .map(|error| format!("error=\"{error}\""))
            .into_iter()
            .collect();
        let mut providers = self.inner.providers.iter();
        parameters.extend(providers.find_map(|provider| provider.challenge(path)));
        if !parameters.is_empty() {
            challenge.push(' ');
            challenge.push_str(&parameters.join(", "));
        }
        let mut response = (StatusCode::UNAUTHORIZED, "authentication required").into_response();
        if let Ok(value) = HeaderValue::from_str(&challenge) {
            response.headers_mut().insert(WWW_AUTHENTICATE, value);
        }
        if clear_session {
            let cleared = cookies::clear(SESSION_COOKIE, "/", self.inner.secure);
            cookies::append(response.headers_mut(), cleared);
        }
        response
    }
}

#[async_trait]
impl Authenticate for Auth {
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        extensions: &Extensions,
    ) -> Result<Option<Actor>, Refusal> {
        let peer = extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(address)| *address);
        let request = Presented { headers, peer };
        let mut found = self.session_actor(request.headers).await?;
        self.check_authorization(request)?;
        for provider in &self.inner.providers {
            let actor = match provider.authenticate(request).await {
                Verdict::Absent => continue,
                Verdict::Refused(refusal) => return Err(refusal),
                Verdict::Actor(actor) => actor,
                Verdict::Login(_) if found.is_some() => continue,
                Verdict::Login(login) => {
                    let user = self.inner.accounts.resolve(&login).await?;
                    Actor { user, agent: None }
                }
            };
            found.get_or_insert(actor);
        }
        Ok(found)
    }
}

async fn require_actor(State(auth): State<Auth>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let had_session = cookies::read(request.headers(), SESSION_COOKIE).is_some();
    match auth
        .authenticate(request.headers(), request.extensions())
        .await
    {
        Ok(Some(actor)) => {
            request.extensions_mut().insert(actor);
            next.run(request).await
        }
        Ok(None) => auth.challenge(&path, None, had_session),
        Err(Refusal::Credential) => auth.challenge(&path, Some("invalid_token"), had_session),
        Err(refusal @ (Refusal::Peer | Refusal::NoVerifiedEmail)) => {
            (StatusCode::FORBIDDEN, refusal.to_string()).into_response()
        }
        Err(refusal @ Refusal::Unavailable(_)) => {
            (StatusCode::SERVICE_UNAVAILABLE, refusal.to_string()).into_response()
        }
    }
}

/// Ends the browser's session and clears its cookie.
async fn sign_out(State(auth): State<Auth>, headers: HeaderMap) -> Response {
    if let Some(token) = cookies::read(&headers, SESSION_COOKIE)
        && let Err(error) = auth.inner.accounts.end_session(&token).await
    {
        return (StatusCode::SERVICE_UNAVAILABLE, error.to_string()).into_response();
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    let cleared = cookies::clear(SESSION_COOKIE, "/", auth.inner.secure);
    cookies::append(response.headers_mut(), cleared);
    response
}
