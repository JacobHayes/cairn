//! Google Cloud IAP, faked offline: ES256 keys generated for the test, a stub of Google's
//! IAP key set on loopback, assertions signed as IAP signs them, and a fake IAP standing
//! in front of Cairn as IAP does, so a test drives the same handshake a real client meets.
//!
//! The fake IAP behaves as IAP was observed to: a request with no credential gets a bare
//! 401 with no `WWW-Authenticate`; a Google ID token in `Proxy-Authorization` or
//! `Authorization` passes only if Google signed it for the allowlisted client and it has
//! not expired; `Proxy-Authorization` is removed, and an `Authorization` that carried the
//! token is passed on to Cairn blank; a browser passes with its IAP session cookie; and
//! every request let through carries a fresh assertion. The OAuth paths a deployment routes around IAP pass with none.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, PROXY_AUTHORIZATION};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cairn_auth::Clock;
use cairn_auth::gcp_iap::ASSERTION_HEADER;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{Value, json};
use url::Url;

use super::issuer::Issuer;

/// The audience IAP signs for: the protected backend service.
pub const AUDIENCE: &str = "/projects/123456789/global/backendServices/987654321";
/// The issuer of every IAP assertion.
pub const ISSUER: &str = "https://cloud.google.com/iap";
/// The cookie a browser holds once signed in at IAP; its value is the person's subject.
pub const BROWSER_COOKIE: &str = "fake_iap_session";

/// A key the test signs with, under its key id.
#[derive(Clone)]
pub struct Key {
    pub id: &'static str,
    signing: SigningKey,
}

pub fn key(id: &'static str, seed: u8) -> Key {
    let signing = SigningKey::from_bytes(&[seed; 32].into()).unwrap();
    Key { id, signing }
}

impl Key {
    fn jwk(&self) -> Value {
        let point = self.signing.verifying_key().to_encoded_point(false);
        json!({
            "kty": "EC",
            "crv": "P-256",
            "alg": "ES256",
            "use": "sig",
            "kid": self.id,
            "x": URL_SAFE_NO_PAD.encode(point.x().unwrap()),
            "y": URL_SAFE_NO_PAD.encode(point.y().unwrap()),
        })
    }
}

/// A stub of Google's IAP key set, counting fetches.
pub struct KeySet {
    pub url: Url,
    keys: Arc<Mutex<Vec<Value>>>,
    fetches: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct Served {
    keys: Arc<Mutex<Vec<Value>>>,
    fetches: Arc<AtomicUsize>,
}

async fn serve_keys(State(served): State<Served>) -> axum::Json<Value> {
    served.fetches.fetch_add(1, Ordering::SeqCst);
    axum::Json(json!({ "keys": served.keys.lock().unwrap().clone() }))
}

impl KeySet {
    pub async fn start(keys: &[&Key]) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/keys", listener.local_addr().unwrap());
        let served = Served {
            keys: Arc::new(Mutex::new(keys.iter().map(|key| key.jwk()).collect())),
            fetches: Arc::new(AtomicUsize::new(0)),
        };
        let router = Router::new()
            .route("/keys", get(serve_keys))
            .with_state(served.clone());
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            url: url.parse().unwrap(),
            keys: served.keys,
            fetches: served.fetches,
        }
    }

    pub fn publish(&self, keys: &[&Key]) {
        *self.keys.lock().unwrap() = keys.iter().map(|key| key.jwk()).collect();
    }

    pub fn fetches(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

/// The claims IAP signs for `subject` (IAP's account id) at `now`, with what Google adds
/// beside them.
pub fn claims_of(subject: &str, email: &str, now: i64) -> Value {
    json!({
        "iss": ISSUER,
        "aud": AUDIENCE,
        "sub": subject,
        "email": email,
        "hd": "example.org",
        "iat": now,
        "exp": now + 600,
        "login_source": "GOOGLE",
        "google": { "access_levels": [] },
    })
}

/// An assertion over `claims`, its header naming `kid` and `alg`, signed by `key`.
pub fn sign_as(claims: &Value, kid: &str, alg: &str, key: &Key) -> String {
    let header = json!({ "alg": alg, "typ": "JWT", "kid": kid });
    let encode = |value: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(value).unwrap());
    let message = format!("{}.{}", encode(&header), encode(claims));
    let signature: Signature = key.signing.sign(message.as_bytes());
    format!("{message}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}

pub fn sign(claims: &Value, key: &Key) -> String {
    sign_as(claims, key.id, "ES256", key)
}

/// IAP in front of Cairn.
#[derive(Clone)]
pub struct FakeIap {
    /// The fake Google whose ID tokens IAP accepts.
    pub google: Issuer,
    /// The Google client on the programmatic allowlist: the audience IAP accepts.
    pub allowlisted: String,
    /// The key IAP signs its assertions with.
    pub key: Key,
    pub clock: Clock,
}

/// The paths a deployment routes to Cairn around IAP: discovery, registration, and the
/// token endpoint.
fn is_public(path: &str) -> bool {
    path == "/.well-known/oauth-protected-resource"
        || path.starts_with("/.well-known/oauth-protected-resource/")
        || path == "/.well-known/oauth-authorization-server"
        || path == "/api/oauth/register"
        || path == "/api/oauth/token"
}

impl FakeIap {
    /// `router` behind this IAP.
    pub fn in_front_of(&self, router: Router) -> Router {
        let iap = Arc::new(self.clone());
        router.layer(middleware::from_fn_with_state(iap, pass))
    }

    /// Who a request is, as IAP decides: by a Google ID token if it carries one (with the
    /// header that carried it), else by its browser session; `None` is refused.
    fn person(&self, headers: &HeaderMap) -> Option<Person> {
        let bearer = |name: HeaderName| {
            let value = headers.get(&name)?.to_str().ok()?;
            let token = value.strip_prefix("Bearer ")?.to_owned();
            Some((name, token))
        };
        if let Some((carrier, token)) =
            bearer(PROXY_AUTHORIZATION).or_else(|| bearer(AUTHORIZATION))
        {
            let signed = self.google.verified(&token, &self.allowlisted)?;
            return Some(Person {
                subject: signed.subject,
                email: signed.email,
                carrier: Some(carrier),
            });
        }
        let subject = cairn_auth::cookies::read(headers, BROWSER_COOKIE)?;
        let email = format!("{subject}@example.org");
        Some(Person {
            subject,
            email,
            carrier: None,
        })
    }
}

/// Who IAP let through, and the header whose Google ID token said so, if any.
struct Person {
    subject: String,
    email: String,
    carrier: Option<HeaderName>,
}

async fn pass(State(iap): State<Arc<FakeIap>>, mut request: Request, next: Next) -> Response {
    request.headers_mut().remove(ASSERTION_HEADER);
    if is_public(request.uri().path()) {
        return next.run(request).await;
    }
    let Some(person) = iap.person(request.headers()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let now = iap.clock.now().as_second();
    let subject = format!("accounts.google.com:{}", person.subject);
    let claims = claims_of(&subject, &person.email, now);
    let assertion = sign(&claims, &iap.key);
    let headers = request.headers_mut();
    headers.remove(PROXY_AUTHORIZATION);
    if person.carrier == Some(AUTHORIZATION) {
        headers.insert(AUTHORIZATION, HeaderValue::from_static(""));
    }
    headers.insert(ASSERTION_HEADER, assertion.parse().unwrap());
    next.run(request).await
}
