//! How both OAuth servers answer (I2): back to a client's redirect URI with RFC 9207's
//! `iss`, as an RFC 6749 error body, as a registration, or as a page no other site can
//! frame. Every answer is `no-store`: each carries something that is good once.

use axum::http::header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, LOCATION, X_FRAME_OPTIONS};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Json, Response};
use serde_json::json;
use url::Url;

use super::client::{Client, ClientError, Policy};

/// The client's registered redirect URI, where an answer goes back.
pub(crate) struct Back<'a> {
    /// Who answers: the server's issuer.
    pub(crate) issuer: &'a str,
    /// The redirect URI, already checked against the client's registration.
    pub(crate) redirect_uri: &'a str,
    /// The client's `state`, returned as sent.
    pub(crate) state: Option<&'a str>,
}

impl Back<'_> {
    /// RFC 6749 section 4.1.2.1: an error, at the client.
    pub(crate) fn error(&self, error: &str) -> Response {
        self.send(&[("error", error)])
    }

    /// RFC 6749 section 4.1.2 with RFC 9207's `iss`: the answer, the client's `state`, and
    /// who answered.
    pub(crate) fn send(&self, pairs: &[(&str, &str)]) -> Response {
        let Ok(mut url) = Url::parse(self.redirect_uri) else {
            return page(StatusCode::BAD_REQUEST, "The redirect URI is not a URL.");
        };
        {
            let mut query = url.query_pairs_mut();
            query.extend_pairs(pairs);
            if let Some(state) = self.state {
                query.append_pair("state", state);
            }
            query.append_pair("iss", self.issuer);
        }
        redirect(url.as_str())
    }
}

/// A 303 to `location`.
pub(crate) fn redirect(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    if let Ok(location) = HeaderValue::from_str(location) {
        response.headers_mut().insert(LOCATION, location);
    }
    no_store(response)
}

/// RFC 6749 section 5.2: an error body with `status`.
pub(crate) fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    let body = json!({"error": error, "error_description": description});
    no_store((status, Json(body)).into_response())
}

/// RFC 7591 section 3.2: a registered public client and the grants `policy` offers, or why
/// it is refused.
pub(crate) fn registered(registered: Result<Client, ClientError>, policy: &Policy) -> Response {
    match registered {
        Ok(client) => {
            let body = json!({
                "client_id": client.id(),
                "client_name": client.name,
                "redirect_uris": client.redirect_uris,
                "token_endpoint_auth_method": "none",
                "grant_types": policy.grant_types,
                "response_types": ["code"],
            });
            (StatusCode::CREATED, Json(body)).into_response()
        }
        Err(error) => oauth_error(StatusCode::BAD_REQUEST, error.code(), error.description()),
    }
}

pub(crate) fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// A page that no other site can frame, so a click on it is the user's own. Its forms
/// post only here, and a post's redirect may go only to `answer_to`: browsers hold a
/// form's redirect to the page's `form-action` too, and the consent's answer is a redirect
/// to the client's registered URI.
pub(crate) fn framed_never(mut response: Response, answer_to: Option<&Url>) -> Response {
    let mut form_action = String::from("'self'");
    if let Some(origin) = answer_to.map(|url| url.origin().ascii_serialization()) {
        form_action.push(' ');
        form_action.push_str(&origin);
    }
    let policy = format!("default-src 'none'; form-action {form_action}; frame-ancestors 'none'");
    let headers = response.headers_mut();
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    if let Ok(policy) = HeaderValue::from_str(&policy) {
        headers.insert(CONTENT_SECURITY_POLICY, policy);
    }
    no_store(response)
}

/// A page saying `message`, with `status`.
pub(crate) fn page(status: StatusCode, message: &str) -> Response {
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Cairn</title><p>{}</p>",
        escape(message)
    );
    framed_never((status, Html(body)).into_response(), None)
}

/// Text made safe to place in HTML, inside an element or a quoted attribute.
pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_text_is_escaped() {
        let escaped = escape("<script>\"a\" & 'b'</script>");
        assert!(!escaped.contains(['<', '>', '"', '\'']), "{escaped}");
    }

    /// A redirect URI's own query survives beside the answer, which carries one issuer.
    #[test]
    fn an_answer_keeps_the_redirect_query_and_names_one_issuer() {
        let back = Back {
            issuer: "https://cairn.example",
            redirect_uri: "http://localhost:1/cb?next=x",
            state: Some("s"),
        };
        let response = back.send(&[("code", "c")]);
        let location = response.headers()[LOCATION].to_str().unwrap();
        let url = Url::parse(location).unwrap();
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let named = |name: &str| pairs.iter().filter(|(key, _)| key == name).count();
        assert_eq!(
            (named("next"), named("code"), named("state"), named("iss")),
            (1, 1, 1, 1)
        );
    }
}
