//! A stub OIDC issuer on loopback (readiness ruling: every provider tests offline): its
//! discovery document, its signing key set, and a token endpoint that checks the PKCE
//! verifier and answers with an ID token signed by a key generated for the test, or one
//! spoiled in a chosen way. A test approves a sign-in by hand (`approve`).
//!
//! In Google mode (`start_google`) it stands for Google as the IAP relay meets it: ID
//! tokens name Google as their issuer and the requesting client as their audience and
//! last an hour; the token endpoint wants the client's secret, binds a code to the
//! redirect URI and PKCE challenge of its authorization, issues a refresh token, and
//! answers the refresh grant. A test can make it leave the ID token out, or never answer;
//! it records what it was sent.

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
    CoreEdDsaPrivateSigningKey, CoreIdToken, CoreIdTokenClaims, CoreIdTokenVerifier,
    CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
};
use openidconnect::{
    Audience, ClientId, EmptyAdditionalClaims, EndUserEmail, EndUserName, IssuerUrl, JsonWebKeyId,
    Nonce, PkceCodeChallenge, PkceCodeVerifier, PrivateSigningKey, StandardClaims,
    SubjectIdentifier,
};
use serde_json::json;
use tokio::sync::Notify;
use url::Url;

/// The issuer Google's ID tokens name.
pub const GOOGLE_ISSUER: &str = "https://accounts.google.com";
/// How long a Google ID token lasts.
pub const GOOGLE_ID_TOKEN_SECONDS: i64 = 3600;

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
    nonce: Option<String>,
    challenge: String,
    redirect_uri: Option<String>,
    person: Person,
    spoil: Spoil,
    /// Whether the request asked for offline access with a consent prompt, which is when
    /// Google issues a refresh token.
    offline: bool,
}

/// The stub as Google: its OAuth client.
struct Google {
    client_id: String,
    secret: String,
}

/// How the token endpoint misbehaves, if at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct Switches {
    /// Answers without an ID token.
    pub omit_id_token: bool,
    /// Records the request, signals `entered`, and never answers.
    pub hang: bool,
}

struct Inner {
    url: Url,
    clock: Clock,
    key: CoreEdDsaPrivateSigningKey,
    other_key: CoreEdDsaPrivateSigningKey,
    grants: Mutex<HashMap<String, Grant>>,
    advertise: Mutex<Advertise>,
    google: Option<Google>,
    refreshes: Mutex<HashMap<String, Grant>>,
    switches: Mutex<Switches>,
    token_forms: Mutex<Vec<HashMap<String, String>>>,
    authorizations: Mutex<Vec<HashMap<String, String>>>,
    id_tokens: Mutex<Vec<String>>,
    entered: Notify,
}

/// Who a Google ID token names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signed {
    pub subject: String,
    pub email: String,
}

/// The running stub.
#[derive(Clone)]
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
        Self::launch(clock, None).await
    }

    /// Starts the stub as Google, for the OAuth client `client_id` with `secret`.
    pub async fn start_google(clock: Clock, client_id: &str, secret: &str) -> Self {
        let google = Google {
            client_id: client_id.to_owned(),
            secret: secret.to_owned(),
        };
        Self::launch(clock, Some(google)).await
    }

    async fn launch(clock: Clock, google: Option<Google>) -> Self {
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
            google,
            refreshes: Mutex::new(HashMap::new()),
            switches: Mutex::new(Switches::default()),
            token_forms: Mutex::new(Vec::new()),
            authorizations: Mutex::new(Vec::new()),
            id_tokens: Mutex::new(Vec::new()),
            entered: Notify::new(),
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

    /// Google's authorization endpoint, which the relay sends a browser to.
    pub fn authorization_url(&self) -> Url {
        self.inner.url.join("authorize").unwrap()
    }

    /// Google's token endpoint.
    pub fn token_url(&self) -> Url {
        self.inner.url.join("token").unwrap()
    }

    /// The person signs in at Google and allows the client: where Google sends the
    /// browser back from the authorization request at `location`, carrying the code, the
    /// state, and Google's own `iss`, as Google does.
    pub fn sign_in(&self, location: &str, person: &Person) -> String {
        let query = self.authorization(location);
        let code = grant(&self.inner, &query, person.clone(), Spoil::Nothing);
        let mut back: Url = query["redirect_uri"].parse().unwrap();
        back.query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &query["state"])
            .append_pair("iss", GOOGLE_ISSUER)
            .append_pair("scope", "email openid")
            .append_pair("authuser", "0");
        back.into()
    }

    /// The person declines at Google: where Google sends the browser back.
    pub fn decline(&self, location: &str) -> String {
        let query = self.authorization(location);
        let mut back: Url = query["redirect_uri"].parse().unwrap();
        back.query_pairs_mut()
            .append_pair("error", "access_denied")
            .append_pair("state", &query["state"])
            .append_pair("iss", GOOGLE_ISSUER);
        back.into()
    }

    /// The authorization request at `location`, recorded.
    fn authorization(&self, location: &str) -> HashMap<String, String> {
        let location: Url = location.parse().unwrap();
        assert_eq!(
            location.as_str().split('?').next(),
            Some(self.authorization_url().as_str()),
            "{location}"
        );
        let query: HashMap<String, String> = location.query_pairs().into_owned().collect();
        self.inner
            .authorizations
            .lock()
            .unwrap()
            .push(query.clone());
        query
    }

    /// Every authorization request a browser brought, in order.
    pub fn authorizations(&self) -> Vec<HashMap<String, String>> {
        self.inner.authorizations.lock().unwrap().clone()
    }

    /// Every ID token the token endpoint issued, in order.
    pub fn id_tokens(&self) -> Vec<String> {
        self.inner.id_tokens.lock().unwrap().clone()
    }

    /// Every form the token endpoint received, in order.
    pub fn token_forms(&self) -> Vec<HashMap<String, String>> {
        self.inner.token_forms.lock().unwrap().clone()
    }

    /// From now on, the token endpoint misbehaves as `switches` say.
    pub fn switch(&self, switches: Switches) {
        *self.inner.switches.lock().unwrap() = switches;
    }

    /// Waits until a token request has reached a hanging token endpoint.
    pub async fn entered(&self) {
        self.inner.entered.notified().await;
    }

    /// Who a Google ID token names, if this stub signed it for `audience` and it has not
    /// expired: IAP's check of a bearer.
    pub fn verified(&self, token: &str, audience: &str) -> Option<Signed> {
        let token: CoreIdToken = token.parse().ok()?;
        let keys = CoreJsonWebKeySet::new(vec![self.inner.key.as_verification_key()]);
        let issuer = IssuerUrl::new(GOOGLE_ISSUER.to_owned()).unwrap();
        let clock = self.inner.clock.clone();
        let verifier = CoreIdTokenVerifier::new_public_client(
            ClientId::new(audience.to_owned()),
            issuer,
            keys,
        )
        .set_allowed_algs([CoreJwsSigningAlgorithm::EdDsa])
        .set_time_fn(move || chrono::DateTime::from_timestamp(clock.now().as_second(), 0).unwrap());
        let claims = token.claims(&verifier, |_: Option<&Nonce>| Ok(())).ok()?;
        Some(Signed {
            subject: claims.subject().as_str().to_owned(),
            email: claims.email()?.as_str().to_owned(),
        })
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
        nonce: query.get("nonce").cloned(),
        challenge: query["code_challenge"].clone(),
        redirect_uri: query.get("redirect_uri").cloned(),
        person,
        spoil,
        offline: query.get("access_type").map(String::as_str) == Some("offline")
            && query.get("prompt").map(String::as_str) == Some("consent"),
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

fn refused(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({ "error": error }))).into_response()
}

/// RFC 6749 and 7636: one use per code, and the verifier must answer the challenge; as
/// Google, also the client's secret, the code's redirect URI, and the refresh grant.
async fn token(
    State(inner): State<Arc<Inner>>,
    headers: axum::http::HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    inner.token_forms.lock().unwrap().push(form.clone());
    let switches = *inner.switches.lock().unwrap();
    if switches.hang {
        inner.entered.notify_one();
        return std::future::pending().await;
    }
    if *inner.advertise.lock().unwrap() == Advertise::SecretInFormOnly
        && (headers.contains_key("authorization") || !form.contains_key("client_secret"))
    {
        return refused(StatusCode::UNAUTHORIZED, "invalid_client");
    }
    if let Some(google) = &inner.google {
        let field = |name: &str| form.get(name).map(String::as_str);
        if field("client_id") != Some(&google.client_id)
            || field("client_secret") != Some(&google.secret)
        {
            return refused(StatusCode::UNAUTHORIZED, "invalid_client");
        }
    }
    let grant = match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => code_grant(&inner, &form),
        Some("refresh_token") if inner.google.is_some() => refresh_grant(&inner, &form),
        _ => None,
    };
    let Some(grant) = grant else {
        return refused(StatusCode::BAD_REQUEST, "invalid_grant");
    };
    let mut body = json!({
        "access_token": format!("stub-access-{}", inner.token_forms.lock().unwrap().len()),
        "token_type": "Bearer",
        "expires_in": 3599,
    });
    if !switches.omit_id_token {
        let id_token = id_token(&inner, &grant).to_string();
        inner.id_tokens.lock().unwrap().push(id_token.clone());
        body["id_token"] = json!(id_token);
    }
    if inner.google.is_some() && form["grant_type"] == "authorization_code" && grant.offline {
        let refresh = format!("stub-refresh-{}", inner.refreshes.lock().unwrap().len());
        body["refresh_token"] = json!(refresh);
        inner.refreshes.lock().unwrap().insert(refresh, grant);
    }
    Json(body).into_response()
}

/// The grant a code names, used up, if the form's verifier (and, as Google, its redirect
/// URI) match its authorization.
fn code_grant(inner: &Inner, form: &HashMap<String, String>) -> Option<Grant> {
    let grant = inner.grants.lock().unwrap().remove(form.get("code")?)?;
    let verifier = PkceCodeVerifier::new(form.get("code_verifier")?.clone());
    let answered = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
    if answered.as_str() != grant.challenge {
        return None;
    }
    if inner.google.is_some() && form.get("redirect_uri") != grant.redirect_uri.as_ref() {
        return None;
    }
    Some(grant)
}

/// The grant a refresh token this stub issued names, kept for the next refresh.
fn refresh_grant(inner: &Inner, form: &HashMap<String, String>) -> Option<Grant> {
    let refreshes = inner.refreshes.lock().unwrap();
    let grant = refreshes.get(form.get("refresh_token")?)?;
    Some(Grant {
        client_id: grant.client_id.clone(),
        nonce: None,
        challenge: grant.challenge.clone(),
        redirect_uri: grant.redirect_uri.clone(),
        person: grant.person.clone(),
        spoil: grant.spoil,
        offline: grant.offline,
    })
}

fn id_token(inner: &Inner, grant: &Grant) -> CoreIdToken {
    let now = inner.clock.now().as_second();
    let at = |seconds: i64| chrono::DateTime::from_timestamp(now + seconds, 0).unwrap();
    let spoiled = |spoil: Spoil| grant.spoil == spoil;
    let issuer = if spoiled(Spoil::Issuer) {
        "http://127.0.0.1:1/".to_owned()
    } else if inner.google.is_some() {
        GOOGLE_ISSUER.to_owned()
    } else {
        inner.url.to_string()
    };
    let audience = if spoiled(Spoil::Audience) {
        "another-client".to_owned()
    } else {
        grant.client_id.clone()
    };
    let nonce = if spoiled(Spoil::Nonce) {
        Some("another-nonce".to_owned())
    } else {
        grant.nonce.clone()
    };
    let (issued_at, expires_at) = if spoiled(Spoil::Expired) {
        (-600, -300)
    } else if inner.google.is_some() {
        (0, GOOGLE_ID_TOKEN_SECONDS)
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
    .set_nonce(nonce.map(Nonce::new));
    let key = if spoiled(Spoil::Signature) {
        &inner.other_key
    } else {
        &inner.key
    };
    CoreIdToken::new(claims, key, CoreJwsSigningAlgorithm::EdDsa, None, None).unwrap()
}
