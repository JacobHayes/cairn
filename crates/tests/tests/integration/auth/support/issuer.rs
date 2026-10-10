//! A stub OIDC issuer on loopback (readiness ruling: every provider tests offline): its
//! discovery document, its signing key set, and a token endpoint that checks the PKCE
//! verifier and answers with an ID token signed by a key generated for the test, or one
//! spoiled in a chosen way. A test approves a sign-in by hand (`approve`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use cairn_auth::Clock;
use ed25519_dalek::pkcs8::{EncodePrivateKey, spki::der::pem::LineEnding};
use openidconnect::core::{
    CoreEdDsaPrivateSigningKey, CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet,
    CoreJwsSigningAlgorithm,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserEmail, EndUserName, IssuerUrl, JsonWebKeyId, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use serde::Deserialize;
use serde_json::json;
use url::Url;

/// Who signs in at the issuer.
#[derive(Clone, Debug)]
pub struct Person {
    pub subject: &'static str,
    pub name: &'static str,
    pub email: &'static str,
    pub email_verified: bool,
}

/// How the issued ID token is spoiled, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spoil {
    Nothing,
    Issuer,
    Audience,
    Expired,
    Signature,
    Nonce,
}

/// What the discovery document advertises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advertise {
    /// Its own endpoints, on loopback.
    Honest,
    /// A key set over plain http to an address that is not loopback (`0.0.0.0`, which
    /// Linux connects to this machine, so a client that ignored the rule would succeed).
    PlainHttpKeys,
    /// A token endpoint the same way.
    PlainHttpToken,
    /// A document past the body limit.
    Oversized,
    /// Only `client_secret_post` at the token endpoint, which then refuses a secret sent
    /// any other way.
    SecretInFormOnly,
}

struct Grant {
    client_id: String,
    nonce: String,
    challenge: String,
    person: Person,
    spoil: Spoil,
}

struct Inner {
    url: Url,
    clock: Clock,
    key: CoreEdDsaPrivateSigningKey,
    other_key: CoreEdDsaPrivateSigningKey,
    grants: Mutex<HashMap<String, Grant>>,
    advertise: Mutex<Advertise>,
}

/// The running stub.
pub struct Issuer {
    inner: Arc<Inner>,
}

fn signing_key(seed: u8) -> CoreEdDsaPrivateSigningKey {
    let key = ed25519_dalek::SigningKey::from_bytes(&[seed; 32]);
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kid = Some(JsonWebKeyId::new("stub".to_owned()));
    CoreEdDsaPrivateSigningKey::from_ed25519_pem(&pem, kid).unwrap()
}

impl Issuer {
    /// Starts the stub on a free loopback port, reading `clock` for the tokens it issues.
    pub async fn start(clock: Clock) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url: Url = format!("http://{}/", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let inner = Arc::new(Inner {
            url,
            clock,
            key: signing_key(7),
            other_key: signing_key(8),
            grants: Mutex::new(HashMap::new()),
            advertise: Mutex::new(Advertise::Honest),
        });
        let router = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(Arc::clone(&inner));
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self { inner }
    }

    /// From now on, discovery advertises `advertise`.
    pub fn advertise(&self, advertise: Advertise) {
        *self.inner.advertise.lock().unwrap() = advertise;
    }

    pub fn url(&self) -> Url {
        self.inner.url.clone()
    }

    /// The person signs in and approves: the code the issuer would send back to the
    /// authorization request at `location`.
    pub fn approve(&self, location: &str, person: &Person, spoil: Spoil) -> String {
        let location: Url = location.parse().unwrap();
        assert!(
            location.as_str().starts_with(self.inner.url.as_str()),
            "{location}"
        );
        let query: HashMap<String, String> = location.query_pairs().into_owned().collect();
        grant(&self.inner, &query, person.clone(), spoil)
    }
}

/// Records a grant for the authorization request `query` and returns its code.
fn grant(inner: &Inner, query: &HashMap<String, String>, person: Person, spoil: Spoil) -> String {
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(query["response_type"], "code");
    assert!(query["scope"].split(' ').any(|scope| scope == "openid"));
    let mut grants = inner.grants.lock().unwrap();
    let code = format!("code-{}", grants.len());
    let grant = Grant {
        client_id: query["client_id"].clone(),
        nonce: query["nonce"].clone(),
        challenge: query["code_challenge"].clone(),
        person,
        spoil,
    };
    grants.insert(code.clone(), grant);
    code
}

async fn discovery(State(inner): State<Arc<Inner>>) -> Json<serde_json::Value> {
    let url = &inner.url;
    let advertise = *inner.advertise.lock().unwrap();
    let plain = |path: &str| format!("http://0.0.0.0:{}/{path}", url.port().unwrap());
    let own = |path: &str| url.join(path).unwrap().to_string();
    let mut document = json!({
        "issuer": url.as_str(),
        "authorization_endpoint": own("authorize"),
        "token_endpoint": if advertise == Advertise::PlainHttpToken { plain("token") } else { own("token") },
        "jwks_uri": if advertise == Advertise::PlainHttpKeys { plain("jwks") } else { own("jwks") },
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
    });
    if advertise == Advertise::Oversized {
        document["padding"] = json!("x".repeat(100_000));
    }
    if advertise == Advertise::SecretInFormOnly {
        document["token_endpoint_auth_methods_supported"] = json!(["client_secret_post"]);
    }
    Json(document)
}

async fn jwks(State(inner): State<Arc<Inner>>) -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        inner.key.as_verification_key(),
    ]))
}

#[derive(Deserialize)]
struct TokenRequest {
    grant_type: String,
    code: String,
    code_verifier: String,
    client_secret: Option<String>,
}

/// RFC 6749 and 7636: one use per code, and the verifier must answer the challenge.
async fn token(
    State(inner): State<Arc<Inner>>,
    headers: axum::http::HeaderMap,
    Form(request): Form<TokenRequest>,
) -> Response {
    if *inner.advertise.lock().unwrap() == Advertise::SecretInFormOnly
        && (headers.contains_key("authorization") || request.client_secret.is_none())
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid_client"})),
        )
            .into_response();
    }
    let grant = inner.grants.lock().unwrap().remove(&request.code);
    let Some(grant) = grant else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    };
    let verifier = PkceCodeVerifier::new(request.code_verifier);
    let answered = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
    if request.grant_type != "authorization_code" || answered.as_str() != grant.challenge {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response();
    }
    let id_token = id_token(&inner, &grant);
    let body = json!({"access_token": "stub-access", "token_type": "Bearer", "id_token": id_token});
    Json(body).into_response()
}

fn id_token(inner: &Inner, grant: &Grant) -> CoreIdToken {
    let now = inner.clock.now().as_second();
    let at = |seconds: i64| chrono::DateTime::from_timestamp(now + seconds, 0).unwrap();
    let spoiled = |spoil: Spoil| grant.spoil == spoil;
    let issuer = if spoiled(Spoil::Issuer) {
        "http://127.0.0.1:1/".to_owned()
    } else {
        inner.url.to_string()
    };
    let audience = if spoiled(Spoil::Audience) {
        "another-client".to_owned()
    } else {
        grant.client_id.clone()
    };
    let nonce = if spoiled(Spoil::Nonce) {
        "another-nonce".to_owned()
    } else {
        grant.nonce.clone()
    };
    let (issued_at, expires_at) = if spoiled(Spoil::Expired) {
        (-600, -300)
    } else {
        (0, 300)
    };
    let person = &grant.person;
    let standard = StandardClaims::new(SubjectIdentifier::new(person.subject.to_owned()))
        .set_email(Some(EndUserEmail::new(person.email.to_owned())))
        .set_email_verified(Some(person.email_verified))
        .set_name(Some(EndUserName::new(person.name.to_owned()).into()));
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(issuer).unwrap(),
        vec![Audience::new(audience)],
        at(expires_at),
        at(issued_at),
        standard,
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(nonce)));
    let key = if spoiled(Spoil::Signature) {
        &inner.other_key
    } else {
        &inner.key
    };
    CoreIdToken::new(claims, key, CoreJwsSigningAlgorithm::EdDsa, None, None).unwrap()
}
