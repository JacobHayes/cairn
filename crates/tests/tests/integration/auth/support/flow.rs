//! What the two OAuth servers' tests share: plain requests through a router, the
//! registration body of a local MCP client, and the authorization request.

use std::collections::HashMap;

use axum::Router;
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, LOCATION};
use axum::http::{Response, StatusCode};
use serde_json::{Value, json};
use url::Url;

use super::{LOCAL, body, request, send, with_cookie};

pub const REDIRECT: &str = "http://127.0.0.1:33418/callback";

pub async fn get(router: &Router, uri: &str, cookies: &str) -> Response<Body> {
    let request = with_cookie(request(uri, LOCAL), cookies).body(Body::empty());
    send(router, request.unwrap()).await
}

pub async fn post_form(
    router: &Router,
    uri: &str,
    cookies: &str,
    form: &[(&str, &str)],
) -> Response<Body> {
    let encoded = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(form)
        .finish();
    let request = with_cookie(request(uri, LOCAL), cookies)
        .method("POST")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(encoded));
    send(router, request.unwrap()).await
}

pub async fn json_of(response: Response<Body>) -> Value {
    serde_json::from_slice(&body(response).await).unwrap()
}

pub fn location(response: &Response<Body>) -> String {
    response.headers()[LOCATION].to_str().unwrap().to_owned()
}

/// The query of a URL, absolute or a path on this deployment.
pub fn query(location: &str) -> HashMap<String, String> {
    let url = Url::parse(location).or_else(|_| Url::parse(&format!("http://cairn{location}")));
    url.unwrap().query_pairs().into_owned().collect()
}

/// The path and query of an absolute URL on this deployment.
pub fn local(url: &str) -> String {
    let url = Url::parse(url).unwrap();
    format!("{}?{}", url.path(), url.query().unwrap_or_default())
}

/// What a local MCP client sends to register (Claude Code's body).
pub fn registration(redirect: &str) -> Value {
    json!({
        "client_name": "Claude Code",
        "redirect_uris": [redirect],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    })
}

pub async fn register(router: &Router, redirect: &str) -> Response<Body> {
    let request = request("/api/oauth/register", LOCAL)
        .method("POST")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(registration(redirect).to_string()));
    send(router, request.unwrap()).await
}

pub async fn client_id(router: &Router) -> String {
    let response = register(router, REDIRECT).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    json_of(response).await["client_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

pub fn authorize_uri(
    client_id: &str,
    redirect: &str,
    challenge: &str,
    extra: &[(&str, &str)],
) -> String {
    let mut pairs = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("state", "client-state"),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ];
    pairs.extend_from_slice(extra);
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    format!("/api/oauth/authorize?{query}")
}

pub async fn exchange(router: &Router, client: &str, verifier: &str, code: &str) -> Response<Body> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", REDIRECT),
        ("client_id", client),
        ("code_verifier", verifier),
    ];
    post_form(router, "/api/oauth/token", "", &form).await
}
