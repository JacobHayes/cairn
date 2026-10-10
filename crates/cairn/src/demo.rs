//! `cairn demo` (README, Try it): the real server with nothing to set up. The fixtures are
//! seeded into a memory store at start, so nothing is saved; the dev provider signs every
//! request in; and, only here, a stub OIDC issuer and a scripted assistant answer under
//! `/demo/` on the same port, so sign-in and the assistant can be tried and tested with no
//! provider, account, or credential. They are configured as any deployment would configure
//! them (an `oidc` provider and a `chat_completions` assistant, both at this address), so
//! the server runs its ordinary code against them.
//!
//! The stub issuer signs in whoever types an email, as that person, with the email verified.
//! The assistant plays what `PUT /demo/assistant/script` last set: a list of steps, each
//! `{"say": text}` or `{"calls": [{"name", "arguments"}]}`, optionally with `"after_ms"` to
//! answer slowly. With no step left it says it is a demo.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, Redirect};
use axum::routing::{get, post, put};
use axum::{Form, Json, Router};
use ed25519_dalek::pkcs8::{EncodePrivateKey, spki::der::pem::LineEnding};
use openidconnect::core::{
    CoreEdDsaPrivateSigningKey, CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet,
    CoreJwsSigningAlgorithm,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserEmail, IssuerUrl, JsonWebKeyId, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use cairn_service::{Parts, Service};
use cairn_store::{InProcessNotifier, MemoryStore};

use crate::config::{self, Config, Environment};
use crate::host::AllowedHosts;
use crate::root;

/// What the assistant says when nothing is scripted.
const UNSCRIPTED: &str = "This is a demo: I only say what I am scripted to say.";

/// The demo's configuration for `listen`, which the dev provider requires to be loopback.
///
/// # Errors
///
/// What the configuration says is wrong with it, for an address that is not loopback, or a
/// port of 0, which would leave the demo's own addresses naming the wrong port.
pub fn config(listen: SocketAddr) -> Result<Config, String> {
    if listen.port() == 0 {
        return Err("the demo needs a fixed port, not 0".to_owned());
    }
    let text = format!(
        r#"
database = "in-memory"
listen = "{listen}"
public_url = "http://{listen}/"
timezone = "UTC"

[[auth]]
kind = "dev"
name = "dev"
user = "dev"

[[auth]]
kind = "oidc"
name = "stub"
issuer = "{issuer}"
client_id = "cairn"

[assistant]
protocol = "chat_completions"
endpoint = "http://{listen}/demo/assistant/v1"
model = "scripted"
"#,
        issuer = issuer_url(listen)
    );
    config::parse(&text, Path::new("."), &Environment::default()).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        lines.join("; ")
    })
}

fn issuer_url(listen: SocketAddr) -> String {
    format!("http://{listen}/demo/issuer/")
}

/// The store the demo serves, seeded with every fixture, and the routes the demo adds.
///
/// # Errors
///
/// A fixture that did not seed, or a signing key that did not build.
pub async fn parts(config: &Config) -> Result<(Arc<MemoryStore>, Router), String> {
    let store = Arc::new(MemoryStore::new());
    let service = Service::new(Parts {
        store: Arc::clone(&store),
        notifier: Arc::new(InProcessNotifier::new()),
        settings: config.settings(),
        capabilities: root::capabilities(config),
    });
    cairn_wasm::fixtures::seed(&service)
        .await
        .map_err(|error| error.to_string())?;
    let url = Url::parse(&issuer_url(config.listen)).map_err(|error| error.to_string())?;
    let issuer = Arc::new(Issuer::new(url)?);
    let issuing = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(keys))
        .route("/authorize", get(sign_in_form).post(sign_in))
        .route("/token", post(token))
        .with_state(issuer);
    let assistant = Router::new()
        .route("/script", put(script))
        .route("/v1/chat/completions", post(complete))
        .with_state(Arc::new(Script::default()));
    let routes = Router::new()
        .nest("/demo/issuer", issuing)
        .nest("/demo/assistant", assistant);
    let hosts = AllowedHosts::new(&config.public_url, config.listen);
    Ok((store, hosts.guard(routes)))
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The stub issuer: what a sign-in granted, until its code is exchanged.
struct Issuer {
    url: Url,
    key: CoreEdDsaPrivateSigningKey,
    codes: AtomicU64,
    grants: Mutex<HashMap<String, Grant>>,
}

struct Grant {
    client_id: String,
    nonce: String,
    challenge: String,
    email: String,
}

impl Issuer {
    fn new(url: Url) -> Result<Self, String> {
        let seed = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
        let pem = seed
            .to_pkcs8_pem(LineEnding::LF)
            .map_err(|error| error.to_string())?;
        let kid = Some(JsonWebKeyId::new("demo".to_owned()));
        let key = CoreEdDsaPrivateSigningKey::from_ed25519_pem(&pem, kid)
            .map_err(|error| error.clone())?;
        Ok(Self {
            url,
            key,
            codes: AtomicU64::new(0),
            grants: Mutex::new(HashMap::new()),
        })
    }
}

async fn discovery(State(issuer): State<Arc<Issuer>>) -> Json<Value> {
    let url = issuer.url.as_str();
    Json(json!({
        "issuer": url,
        "authorization_endpoint": format!("{url}authorize"),
        "token_endpoint": format!("{url}token"),
        "jwks_uri": format!("{url}jwks"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
    }))
}

async fn keys(State(issuer): State<Arc<Issuer>>) -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        issuer.key.as_verification_key(),
    ]))
}

/// The authorization request, as Cairn's OIDC provider sends it.
#[derive(Deserialize)]
struct Authorization {
    client_id: String,
    nonce: String,
    code_challenge: String,
    redirect_uri: String,
    state: String,
}

/// Asks who is signing in; the form posts back to this address, request included.
async fn sign_in_form() -> Html<&'static str> {
    Html(
        r#"<!doctype html><title>Demo sign-in</title>
<form method="post"><label>Email <input name="email" type="email" required></label>
<button type="submit">Sign in</button></form>"#,
    )
}

#[derive(Deserialize)]
struct Person {
    email: String,
}

/// The person signs in: the browser goes back to Cairn with a code for the token endpoint.
async fn sign_in(
    State(issuer): State<Arc<Issuer>>,
    Query(request): Query<Authorization>,
    Form(person): Form<Person>,
) -> Result<Redirect, StatusCode> {
    let mut back = Url::parse(&request.redirect_uri).map_err(|_| StatusCode::BAD_REQUEST)?;
    // Only back to this server: the demo is no open redirect.
    if back.origin() != issuer.url.origin() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let code = format!("code-{}", issuer.codes.fetch_add(1, Ordering::Relaxed));
    back.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &request.state);
    locked(&issuer.grants).insert(
        code,
        Grant {
            client_id: request.client_id,
            nonce: request.nonce,
            challenge: request.code_challenge,
            email: person.email,
        },
    );
    Ok(Redirect::to(back.as_str()))
}

#[derive(Deserialize)]
struct TokenRequest {
    code: String,
    code_verifier: String,
}

/// RFC 6749 and 7636: one use per code, and the verifier must answer the challenge.
async fn token(
    State(issuer): State<Arc<Issuer>>,
    Form(request): Form<TokenRequest>,
) -> Result<Json<Value>, StatusCode> {
    let grant = locked(&issuer.grants)
        .remove(&request.code)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let verifier = PkceCodeVerifier::new(request.code_verifier);
    if PkceCodeChallenge::from_code_verifier_sha256(&verifier).as_str() != grant.challenge {
        return Err(StatusCode::BAD_REQUEST);
    }
    let now = jiff::Timestamp::now().as_second();
    let at = |seconds: i64| {
        chrono::DateTime::from_timestamp(now + seconds, 0).ok_or(StatusCode::INTERNAL_SERVER_ERROR)
    };
    let person = StandardClaims::new(SubjectIdentifier::new(grant.email.clone()))
        .set_email(Some(EndUserEmail::new(grant.email)))
        .set_email_verified(Some(true));
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::from_url(issuer.url.clone()),
        vec![Audience::new(grant.client_id)],
        at(300)?,
        at(0)?,
        person,
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(grant.nonce)));
    let id_token = CoreIdToken::new(
        claims,
        &issuer.key,
        CoreJwsSigningAlgorithm::EdDsa,
        None,
        None,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        json!({"access_token": "demo", "token_type": "Bearer", "id_token": id_token}),
    ))
}

/// The steps the assistant plays next, in order.
#[derive(Default)]
struct Script(Mutex<VecDeque<Step>>);

#[derive(Deserialize)]
struct Step {
    say: Option<String>,
    #[serde(default)]
    calls: Vec<Call>,
    after_ms: Option<u64>,
}

#[derive(Deserialize)]
struct Call {
    name: String,
    arguments: Value,
}

/// `PUT /demo/assistant/script`: replaces whatever the last script left unplayed.
async fn script(State(script): State<Arc<Script>>, Json(steps): Json<Vec<Step>>) -> StatusCode {
    *locked(&script.0) = steps.into();
    StatusCode::NO_CONTENT
}

/// The model's endpoint: the next step as a chat completion.
async fn complete(State(script): State<Arc<Script>>) -> Json<Value> {
    let step = locked(&script.0).pop_front().unwrap_or_else(|| Step {
        say: Some(UNSCRIPTED.to_owned()),
        calls: Vec::new(),
        after_ms: None,
    });
    if let Some(after) = step.after_ms {
        tokio::time::sleep(Duration::from_millis(after)).await;
    }
    let calls: Vec<Value> = step
        .calls
        .iter()
        .enumerate()
        .map(|(index, call)| {
            json!({
                "id": format!("call_{index}"),
                "type": "function",
                "function": {"name": call.name, "arguments": call.arguments.to_string()},
            })
        })
        .collect();
    Json(
        json!({"choices": [{"message": {"role": "assistant", "content": step.say, "tool_calls": calls}}]}),
    )
}
