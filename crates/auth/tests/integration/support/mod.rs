//! What the auth tests share: a memory store, a clock the test moves, a bare router behind
//! the layer, and requests from a chosen peer.

#![allow(dead_code)]

pub mod issuer;
pub mod transcript;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{Request, Response, StatusCode};
use axum::routing::get;
use axum::{Extension, Json, Router};
use cairn_auth::{Accounts, Auth, AuthProvider, Clock, Listener};
use cairn_schema::{Actor, Timestamp};
use cairn_store::{AuthEvent, AuthLogQuery, MemoryStore};
use http_body_util::BodyExt;
use tower::ServiceExt;
use url::Url;

/// A clock the test sets, in whole seconds since the epoch.
#[derive(Clone, Default)]
pub struct TestClock(Arc<AtomicI64>);

impl TestClock {
    pub fn clock(&self) -> Clock {
        let seconds = Arc::clone(&self.0);
        Clock::from_fn(move || Timestamp::from_second(seconds.load(Ordering::SeqCst)).unwrap())
    }

    pub fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }
}

/// A loopback TCP listener.
pub fn loopback() -> Listener {
    Listener::Tcp("127.0.0.1:8080".parse().unwrap())
}

/// A listener every machine on the network reaches.
pub fn public() -> Listener {
    Listener::Tcp("0.0.0.0:8080".parse().unwrap())
}

pub fn base() -> Url {
    "http://127.0.0.1:8080".parse().unwrap()
}

pub struct World {
    pub store: Arc<MemoryStore>,
    pub clock: TestClock,
    pub accounts: Accounts<MemoryStore>,
}

impl World {
    pub fn new() -> Self {
        let store = Arc::new(MemoryStore::new());
        let clock = TestClock::default();
        clock.advance(1_800_000_000);
        let accounts = Accounts::new(Arc::clone(&store), clock.clock());
        Self {
            store,
            clock,
            accounts,
        }
    }

    pub fn auth(
        &self,
        listener: Listener,
        providers: Vec<Arc<dyn AuthProvider>>,
    ) -> Auth<MemoryStore> {
        Auth::new(self.accounts.clone(), listener, &base(), providers)
    }

    /// The auth log's events, in order.
    pub async fn log(&self) -> Vec<AuthEvent> {
        let page = self
            .accounts
            .auth_log(&AuthLogQuery::default())
            .await
            .unwrap();
        page.items
            .into_iter()
            .map(|logged| logged.entry.event)
            .collect()
    }
}

/// A bare router behind the layer with one route answering the actor, plus auth's own.
pub fn app(auth: &Auth<MemoryStore>) -> Router {
    let whoami = Router::new().route(
        "/whoami",
        get(|Extension(actor): Extension<Actor>| async move { Json(actor) }),
    );
    auth.protect(whoami).merge(auth.router())
}

/// A request builder for `uri` from `peer`.
pub fn request(uri: &str, peer: &str) -> axum::http::request::Builder {
    let peer: SocketAddr = peer.parse().unwrap();
    Request::builder().uri(uri).extension(ConnectInfo(peer))
}

pub const LOCAL: &str = "127.0.0.1:50000";
pub const REMOTE: &str = "192.0.2.7:50000";

/// Sends `request` through the router, recording the exchange in the proof's transcript
/// when one is being written.
pub async fn send(router: &Router, request: Request<Body>) -> Response<Body> {
    transcript::request(&request);
    let response = router.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    let response = Response::from_parts(parts, Body::from(bytes.clone()));
    transcript::response(&response, &bytes);
    response
}

/// The actor `/whoami` answers for `request`, or the status it was refused with.
pub async fn whoami(router: &Router, request: Request<Body>) -> Result<Actor, StatusCode> {
    let response = send(router, request).await;
    match response.status() {
        StatusCode::OK => Ok(serde_json::from_slice(&body(response).await).unwrap()),
        status => Err(status),
    }
}

pub async fn body(response: Response<Body>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

/// The `name=value` pairs a response sets, as one `Cookie` header value, ignoring removals.
pub fn cookies_set(response: &Response<Body>) -> Vec<(String, String)> {
    let values = response.headers().get_all(SET_COOKIE).into_iter();
    values
        .map(|value| cookie::Cookie::parse(value.to_str().unwrap().to_owned()).unwrap())
        .map(|cookie| (cookie.name().to_owned(), cookie.value().to_owned()))
        .collect()
}

/// A `Cookie` header value carrying `pairs`.
pub fn cookie_header(pairs: &[(String, String)]) -> String {
    let pairs = pairs.iter().filter(|(_, value)| !value.is_empty());
    pairs
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn with_cookie(
    builder: axum::http::request::Builder,
    cookie: &str,
) -> axum::http::request::Builder {
    builder.header(COOKIE, cookie)
}
