//! The browser cookies auth sets: the session, and a login in progress. Both are
//! `HttpOnly` (no script reads them), `SameSite=Lax` (sent on a top-level navigation back
//! from a provider, not on another site's form post or fetch), and `Secure` whenever the
//! deployment is served over https.

use std::time::Duration;

use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use cookie::{Cookie, SameSite};

/// The session cookie.
pub const SESSION_COOKIE: &str = "cairn_session";

/// The value of the cookie `name`, if the request carries one.
#[must_use]
pub fn read(headers: &HeaderMap, name: &str) -> Option<String> {
    let values = headers.get_all(COOKIE).into_iter();
    let found = values
        .filter_map(|value| value.to_str().ok())
        .flat_map(Cookie::split_parse)
        .filter_map(Result::ok)
        .find(|cookie| cookie.name() == name);
    found.map(|cookie| cookie.value().to_owned())
}

/// `headers` without the cookie `name`: what a request would be had it not carried it.
#[must_use]
pub fn without(headers: &HeaderMap, name: &str) -> HeaderMap {
    let mut kept = headers.clone();
    kept.remove(COOKIE);
    let values = headers.get_all(COOKIE).into_iter();
    let others: Vec<String> = values
        .filter_map(|value| value.to_str().ok())
        .flat_map(Cookie::split_parse)
        .filter_map(Result::ok)
        .filter(|cookie| cookie.name() != name)
        .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
        .collect();
    if let Ok(value) = HeaderValue::from_str(&others.join("; "))
        && !others.is_empty()
    {
        kept.insert(COOKIE, value);
    }
    kept
}

/// A `Set-Cookie` header setting `name` to `value` under `path` for `lifetime`.
#[must_use]
pub fn set(name: &str, value: &str, path: &str, lifetime: Duration, secure: bool) -> HeaderValue {
    let max_age = i64::try_from(lifetime.as_secs()).unwrap_or(i64::MAX);
    let cookie = Cookie::build((name, value))
        .path(path)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(cookie::time::Duration::seconds(max_age));
    header(&cookie.build())
}

/// A `Set-Cookie` header removing `name` under `path`.
#[must_use]
pub fn clear(name: &str, path: &str, secure: bool) -> HeaderValue {
    let cookie = Cookie::build((name, ""))
        .path(path)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(cookie::time::Duration::ZERO);
    header(&cookie.build())
}

/// Appends a `Set-Cookie` header.
pub fn append(headers: &mut HeaderMap, value: HeaderValue) {
    headers.append(SET_COOKIE, value);
}

fn header(cookie: &Cookie<'_>) -> HeaderValue {
    match HeaderValue::from_str(&cookie.to_string()) {
        Ok(value) => value,
        Err(error) => unreachable!("cookie names and values are ASCII: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cookie_is_read_by_name_across_cookie_headers() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, "a=1; cairn_session=abc".parse().unwrap());
        headers.append(COOKIE, "b=2".parse().unwrap());
        assert_eq!(read(&headers, SESSION_COOKIE).as_deref(), Some("abc"));
        assert_eq!(read(&headers, "b").as_deref(), Some("2"));
        assert_eq!(read(&headers, "missing"), None);
    }

    #[test]
    fn a_request_without_one_cookie_keeps_the_others() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, "a=1; cairn_session=abc".parse().unwrap());
        headers.append(COOKIE, "b=2".parse().unwrap());
        let kept = without(&headers, SESSION_COOKIE);
        assert_eq!(read(&kept, SESSION_COOKIE), None);
        assert_eq!(
            (read(&kept, "a"), read(&kept, "b")),
            (Some("1".to_owned()), Some("2".to_owned()))
        );
        let mut only = HeaderMap::new();
        only.insert(COOKIE, "cairn_session=abc".parse().unwrap());
        assert!(!without(&only, SESSION_COOKIE).contains_key(COOKIE));
    }

    #[test]
    fn set_cookies_are_http_only_lax_and_secure_on_https() {
        for secure in [false, true] {
            let header = set("name", "value", "/auth", Duration::from_secs(60), secure);
            let cookie = Cookie::parse(header.to_str().unwrap().to_owned()).unwrap();
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.same_site(), Some(SameSite::Lax));
            assert_eq!(cookie.secure().unwrap_or(false), secure);
            assert_eq!(cookie.path(), Some("/auth"));
            assert_eq!(
                cookie.max_age().map(cookie::time::Duration::whole_seconds),
                Some(60)
            );
            let cleared = clear("name", "/auth", secure);
            let cleared = Cookie::parse(cleared.to_str().unwrap().to_owned()).unwrap();
            assert_eq!(
                cleared.max_age().map(cookie::time::Duration::whole_seconds),
                Some(0)
            );
        }
    }
}
