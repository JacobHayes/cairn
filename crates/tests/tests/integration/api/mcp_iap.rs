//! A local MCP client behind Google Cloud IAP (I2, H1, H2), end to end against a fake IAP in
//! front of the API as served and a fake Google: from IAP's bare 401 it discovers Cairn's
//! metadata, registers, signs in at Google, exchanges the code, calls MCP with the Google
//! ID token it was given, and refreshes it when it expires. Cairn stores nothing for it,
//! and logs no credential.
#![cfg(test)]

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{
    AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE,
};
use axum::http::{Request, Response, StatusCode};
use http_body_util::BodyExt;
use openidconnect::PkceCodeChallenge;
use serde_json::{Value, json};
use tower::ServiceExt;
use url::Url;

use crate::api::support::{GOOGLE_CLIENT, World};
use crate::auth::support::iap::BROWSER_COOKIE;
use crate::auth::support::issuer::{Issuer, Person};

/// Where the client listens for its answer.
const REDIRECT: &str = "http://localhost:59411/callback";

/// Who signs in at Google: not the person whose browser is at IAP, so a test sees whose
/// token the client receives.
const PAT: Person = Person {
    subject: "2002",
    name: "Pat",
    email: "pat@example.org",
    email_verified: true,
};

/// The person whose browser holds the IAP session during the sign-in.
const BROWSER: &str = "ann";

async fn call(
    router: &Router,
    request: axum::http::request::Builder,
    body: Body,
) -> Response<Body> {
    let mut request = request.body(body).unwrap();
    let peer = SocketAddr::from(([203, 0, 113, 9], 40_000));
    request.extensions_mut().insert(ConnectInfo(peer));
    router.clone().oneshot(request).await.unwrap()
}

async fn get(router: &Router, uri: &str, cookies: &str) -> Response<Body> {
    let request = Request::builder().uri(uri).header(COOKIE, cookies);
    call(router, request, Body::empty()).await
}

async fn post_form(router: &Router, form: &[(&str, &str)]) -> Response<Body> {
    let encoded = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form)
        .finish();
    let request = Request::builder()
        .method("POST")
        .uri("/api/oauth/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    call(router, request, Body::from(encoded)).await
}

async fn bytes(response: Response<Body>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

async fn json_of(response: Response<Body>) -> Value {
    serde_json::from_slice(&bytes(response).await).unwrap()
}

fn location(response: &Response<Body>) -> String {
    response.headers()[LOCATION].to_str().unwrap().to_owned()
}

fn query(url: &str) -> HashMap<String, String> {
    Url::parse(url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

/// The path and query of `url`.
fn local(url: &str) -> String {
    let url = Url::parse(url).unwrap();
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}

/// A bare MCP request, as a client sends first.
async fn mcp_post(router: &Router, bearer: Option<&str>) -> Response<Body> {
    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": "test", "version": "1" } } });
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header(CONTENT_TYPE, "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(bearer) = bearer {
        request = request.header(AUTHORIZATION, format!("Bearer {bearer}"));
    }
    call(router, request, Body::from(initialize.to_string())).await
}

/// What the client holds after signing in, and what it held on the way.
struct SignedIn {
    issuer: String,
    client: String,
    code: String,
    verifier: String,
    access_token: String,
    refresh_token: String,
}

/// What a local MCP client does to sign in behind IAP, checking each step: discovery from
/// IAP's bare 401, registration, the browser's round trip through Google, the exchange.
async fn sign_in(world: &World, google: &Issuer) -> SignedIn {
    let router = &world.router;
    let bare = mcp_post(router, None).await;
    assert_eq!(bare.status(), StatusCode::UNAUTHORIZED);
    let resource = get(router, "/.well-known/oauth-protected-resource/api/mcp", "").await;
    let resource = json_of(resource).await;
    let issuer = resource["authorization_servers"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let server = json_of(get(router, "/.well-known/oauth-authorization-server", "").await).await;
    assert_eq!(server["issuer"], issuer.as_str());
    let registration = json!({
        "client_name": "Claude Code (cairn)",
        "redirect_uris": [REDIRECT],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });
    let request = Request::builder()
        .method("POST")
        .uri(local(server["registration_endpoint"].as_str().unwrap()))
        .header(CONTENT_TYPE, "application/json");
    let registered = call(router, request, Body::from(registration.to_string())).await;
    assert_eq!(registered.status(), StatusCode::CREATED);
    let client = json_of(registered).await["client_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let code = round_trip(world, google, &server, &client, challenge.as_str()).await;
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", REDIRECT),
        ("client_id", client.as_str()),
        ("code_verifier", verifier.secret().as_str()),
        ("resource", resource["resource"].as_str().unwrap()),
    ];
    let answered = post_form(router, &form).await;
    assert_eq!(answered.status(), StatusCode::OK);
    assert_eq!(answered.headers()[CACHE_CONTROL], "no-store");
    let answered = json_of(answered).await;
    assert_eq!(answered["token_type"], "Bearer");
    assert_eq!(answered["expires_in"], 3599);
    SignedIn {
        issuer,
        client,
        code,
        verifier: verifier.secret().clone(),
        access_token: answered["access_token"].as_str().unwrap().to_owned(),
        refresh_token: answered["refresh_token"].as_str().unwrap().to_owned(),
    }
}

/// The browser, signed in at IAP, authorizes the client; the person signs in at Google;
/// the browser brings Google's answer back to Cairn, which sends the code on to the
/// client, naming Cairn as the issuer.
async fn round_trip(
    world: &World,
    google: &Issuer,
    server: &Value,
    client: &str,
    challenge: &str,
) -> String {
    let router = &world.router;
    let authorize = Url::parse_with_params(
        server["authorization_endpoint"].as_str().unwrap(),
        [
            ("response_type", "code"),
            ("client_id", client),
            ("redirect_uri", REDIRECT),
            ("state", "client-state"),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ],
    )
    .unwrap();
    let browser = format!("{BROWSER_COOKIE}={BROWSER}");
    let to_google = get(router, &local(authorize.as_str()), &browser).await;
    assert_eq!(to_google.status(), StatusCode::SEE_OTHER);
    let at_google = location(&to_google);
    let asked = query(&at_google);
    assert_eq!(asked["client_id"], GOOGLE_CLIENT);
    assert_eq!(asked["code_challenge"], challenge);
    assert_eq!(
        asked["redirect_uri"],
        format!("http://{}/api/oauth/callback", world.address)
    );
    let relay_cookie = to_google.headers()[SET_COOKIE].to_str().unwrap();
    let relay_cookie = relay_cookie.split(';').next().unwrap();
    let from_google = google.sign_in(&at_google, &PAT);
    let cookies = format!("{browser}; {relay_cookie}");
    let back = get(router, &local(&from_google), &cookies).await;
    assert_eq!(back.status(), StatusCode::SEE_OTHER);
    let back = location(&back);
    assert!(back.starts_with(REDIRECT), "{back}");
    let answer = query(&back);
    assert_eq!(answer["state"], "client-state");
    assert_eq!(answer["iss"], format!("http://{}", world.address));
    answer["code"].clone()
}

/// Nothing durable: no user, login, or token (each would be in the auth log), and no
/// session or OAuth state, however long from now.
async fn stored_nothing(world: &World) -> bool {
    use cairn_store::{AuthLogQuery, AuthStore};
    let later = world.clock.now() + jiff::SignedDuration::from_hours(24 * 3650);
    let state = world.store.remove_expired(later).await.unwrap();
    let log = world
        .store
        .auth_log(&AuthLogQuery::default())
        .await
        .unwrap();
    log.items.is_empty() && state == 0
}

/// The relay's public token endpoint takes only a small form: a body past the body limit
/// (far under the API's request limit) is refused before Google is asked.
#[tokio::test]
async fn an_oversized_token_request_is_refused_before_google() {
    let (world, google) = World::behind_iap().await;
    let size = usize::try_from(cairn_schema::limits::BODY_BYTES_MAX).unwrap() + 1;
    let request = Request::builder()
        .method("POST")
        .uri("/api/oauth/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded");
    let refused = call(&world.router, request, Body::from(vec![b'a'; size])).await;
    assert_eq!(refused.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(google.token_forms().len(), 0, "Google was not asked");
}

#[cfg(test)]
mod mcp_in_process {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use cairn_store::AuthStore;
    use serde_json::json;
    use tracing_subscriber::util::SubscriberInitExt;

    use super::*;
    use crate::api::support::GOOGLE_SECRET;
    use crate::api::support::mcp::Agent;
    use crate::auth::support::issuer::Switches;

    /// Every log line written, at every level and target.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Bounded and quiet: a whole sign-in, an MCP call, a refresh, and an exchange that
    /// fails (Google leaves out the ID token) log no credential at any level: not the
    /// Google client secret, a code, a verifier, a refresh token, an ID token, nor Google's
    /// access token.
    #[tokio::test]
    async fn signing_in_logs_no_credential() {
        // With one dispatcher in the process, tracing works out a callsite's interest from
        // the default of whichever thread first reaches it, so another test's thread would
        // cache Cairn's callsites as unwanted. A second dispatcher held alongside makes
        // every callsite ask each dispatcher instead, this capture included.
        let _alongside = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let captured = Captured::default();
        let writer = captured.clone();
        let _logging = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish()
            .set_default();
        let (world, google) = World::behind_iap().await;
        // Callsites another thread was registering while the capture was installed.
        tracing::callsite::rebuild_interest_cache();
        let signed_in = sign_in(&world, &google).await;
        let agent = Agent::through(&world.router, Some(&signed_in.access_token)).await;
        assert_ne!(agent.unwrap().tools().await.len(), 0);
        let refresh = [
            ("grant_type", "refresh_token"),
            ("refresh_token", signed_in.refresh_token.as_str()),
            ("client_id", signed_in.client.as_str()),
        ];
        let refreshed = json_of(post_form(&world.router, &refresh).await).await;
        google.switch(Switches {
            omit_id_token: true,
            hang: false,
        });
        let failed = sign_in_failing(&world, &google, &signed_in).await;
        let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(
            logs.contains("answered") && logs.contains("/api/oauth/token"),
            "{logs}"
        );
        let mut credentials = vec![
            GOOGLE_SECRET.to_owned(),
            signed_in.code.clone(),
            signed_in.verifier.clone(),
            signed_in.refresh_token.clone(),
            signed_in.access_token.clone(),
            refreshed["access_token"].as_str().unwrap().to_owned(),
            "stub-access-".to_owned(),
        ];
        credentials.extend(failed);
        for credential in &credentials {
            assert!(
                !logs.contains(credential.as_str()),
                "{credential} is logged"
            );
        }
    }

    /// A second sign-in whose exchange fails: its code and verifier.
    async fn sign_in_failing(world: &World, google: &Issuer, signed_in: &SignedIn) -> Vec<String> {
        let server = json!({
            "authorization_endpoint": format!("{}/api/oauth/authorize", signed_in.issuer),
        });
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let code = round_trip(
            world,
            google,
            &server,
            &signed_in.client,
            challenge.as_str(),
        );
        let code = code.await;
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", REDIRECT),
            ("client_id", signed_in.client.as_str()),
            ("code_verifier", verifier.secret().as_str()),
        ];
        let failed = post_form(&world.router, &form).await;
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
        vec![code, verifier.secret().clone()]
    }

    /// I2, H1, H2 end to end: a local client signs in behind IAP with one browser sign-in
    /// and calls MCP with the Google ID token it was handed, as the person who signed in at
    /// Google; an hour later IAP refuses that token, and the refresh grant gets a new one.
    /// Cairn issued no token of its own and stored nothing for the sign-in.
    #[tokio::test]
    async fn a_local_client_signs_in_through_iap_and_calls_mcp() {
        let (world, google) = World::behind_iap().await;
        let signed_in = sign_in(&world, &google).await;
        assert_eq!(signed_in.issuer, format!("http://{}", world.address));
        assert!(stored_nothing(&world).await, "the sign-in stored nothing");
        assert_eq!(
            google.id_tokens(),
            std::slice::from_ref(&signed_in.access_token)
        );

        let agent = Agent::through(&world.router, Some(&signed_in.access_token)).await;
        let agent = agent.unwrap();
        assert_ne!(agent.tools().await.len(), 0);
        let empty =
            json!({ "journey": "j_blank", "name": "A blank journey", "patch_id": "p_blank" });
        let created = agent.ok("create_journey", empty).await;
        let history = agent
            .ok("get_history", json!({ "journey": "j_blank" }))
            .await;
        let first_actor = history["patches"][0]["actor"].clone();
        let actor = &first_actor;
        assert!(actor.get("agent").is_none_or(Value::is_null), "{actor}");
        let user = actor["user"].as_str().unwrap().parse().unwrap();
        let logins = world.store.logins_of(&user).await.unwrap();
        let subject = format!("accounts.google.com:{}", PAT.subject);
        assert_eq!(logins[0].subject.as_str(), subject);
        let tokens = world.store.agent_tokens_of(&user).await.unwrap();
        assert_eq!(tokens.len(), 0, "Cairn minted no token");

        world
            .clock
            .set_to(world.clock.now() + jiff::SignedDuration::from_secs(3601));
        let expired = mcp_post(&world.router, Some(&signed_in.access_token)).await;
        assert_eq!(
            expired.status(),
            StatusCode::UNAUTHORIZED,
            "IAP refuses an expired token"
        );
        let refresh = [
            ("grant_type", "refresh_token"),
            ("refresh_token", signed_in.refresh_token.as_str()),
            ("client_id", signed_in.client.as_str()),
        ];
        let refreshed = json_of(post_form(&world.router, &refresh).await).await;
        let renewed = refreshed["access_token"].as_str().unwrap();
        assert_ne!(renewed, signed_in.access_token);
        let agent = Agent::through(&world.router, Some(renewed)).await.unwrap();
        let node = json!({ "op": "add_node", "node": {
            "key": "n_after", "id": "after", "kind": "action", "title": "After the refresh",
        }});
        let patch = json!({ "id": "p_after", "target": { "journey": "j_blank" },
            "base_revision": created["receipt"]["revision"], "mutations": [node] });
        let written = agent.ok("apply_patch", json!({ "patch": patch })).await;
        assert_eq!(written["status"], "applied");
        let history = agent
            .ok("get_history", json!({ "journey": "j_blank" }))
            .await;
        let patches = history["patches"].as_array().unwrap();
        assert_eq!(patches.len(), 2, "the fresh patch is in the history");
        let actor = &patches[1]["actor"];
        assert_eq!(
            actor["user"], first_actor["user"],
            "the same person, after a refresh"
        );
        assert!(actor.get("agent").is_none_or(Value::is_null), "{actor}");
    }
}
