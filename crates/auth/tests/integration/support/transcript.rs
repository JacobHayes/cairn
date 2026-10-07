//! Transcripts for the proof (`briefs/proof/3.2/prove.sh`): when `CAIRN_AUTH_TRANSCRIPT`
//! names a directory, every request a test sends and the response it gets, and anything a
//! test shows, are appended as Markdown to a file named after the test. Secrets are cut
//! short, so a transcript never holds a usable token. Without the variable, nothing is
//! written.

use std::fmt::{Debug, Write as _};
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::header::{
    AUTHORIZATION, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE, WWW_AUTHENTICATE,
};
use axum::http::{Request, Response};

fn file() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("CAIRN_AUTH_TRANSCRIPT")?);
    let test = std::thread::current().name()?.replace("::", "__");
    Some(directory.join(format!("{test}.md")))
}

fn append(text: &str) {
    let Some(path) = file() else { return };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

/// Shows a value the test reached, under `label`.
pub fn show(label: &str, value: &impl Debug) {
    let value = redact(&format!("{value:#?}"));
    append(&format!("{label}:\n\n```\n{value}\n```\n\n"));
}

/// The request line, its peer, and the headers the proof cares about.
pub fn request(request: &Request<Body>) {
    if file().is_none() {
        return;
    }
    let uri = shorten_url(&request.uri().to_string());
    let mut text = format!("```http\n{} {uri}\n", request.method());
    if let Some(ConnectInfo(peer)) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        writeln!(text, "(from {peer})").unwrap();
    }
    for (name, value) in request.headers() {
        let value = value.to_str().unwrap_or("");
        let shown = match *name {
            AUTHORIZATION => redact(value),
            COOKIE if value.is_empty() => continue,
            COOKIE => cookie_names(value),
            _ if name.as_str().starts_with("tailscale-") => value.to_owned(),
            _ => continue,
        };
        writeln!(text, "{name}: {shown}").unwrap();
    }
    append(&text);
}

/// The response's status, the headers the proof cares about, and its body.
pub fn response(response: &Response<Body>, body: &[u8]) {
    if file().is_none() {
        return;
    }
    let mut text = format!("\n=> {}\n", response.status());
    for (name, value) in response.headers() {
        let value = value.to_str().unwrap_or("");
        let shown = match *name {
            LOCATION => shorten_url(value),
            WWW_AUTHENTICATE => value.to_owned(),
            SET_COOKIE => set_cookie(value),
            _ => continue,
        };
        writeln!(text, "{name}: {shown}").unwrap();
    }
    let content_type = response.headers().get(CONTENT_TYPE);
    let content_type = content_type.and_then(|value| value.to_str().ok());
    let body = String::from_utf8_lossy(body);
    match content_type {
        Some(kind) if kind.starts_with("application/json") => {
            let json: serde_json::Value = serde_json::from_str(&body).unwrap();
            let json = serde_json::to_string_pretty(&json).unwrap();
            writeln!(text, "\n{}", redact(&json)).unwrap();
        }
        Some(kind) if kind.starts_with("text/html") => {
            writeln!(text, "\n{}", redact(&strip_tags(&body))).unwrap();
        }
        _ if !body.is_empty() => writeln!(text, "\n{body}").unwrap(),
        _ => {}
    }
    text.push_str("```\n\n");
    append(&text);
}

fn cookie_names(value: &str) -> String {
    let pairs = value.split(';').filter_map(|pair| pair.split_once('='));
    let names = pairs.map(|(name, _)| format!("{}=…", name.trim()));
    names.collect::<Vec<_>>().join("; ")
}

fn set_cookie(value: &str) -> String {
    let cookie = cookie::Cookie::parse(value.to_owned()).unwrap();
    let removed = cookie
        .max_age()
        .is_some_and(cookie::time::Duration::is_zero);
    let attributes = value.split_once(';').map_or("", |(_, rest)| rest.trim());
    let shown = if removed { "= (removed)" } else { "=…" };
    format!("{}{shown}; {attributes}", cookie.name())
}

/// A URL with every long query value cut short: states, nonces, challenges, client ids.
fn shorten_url(text: &str) -> String {
    let (path, query) = text.split_once('?').unwrap_or((text, ""));
    if query.is_empty() {
        return redact(path);
    }
    let pairs = query.split('&').map(|pair| match pair.split_once('=') {
        Some((name, value)) if value.len() > 24 => format!("{name}={}…", &value[..12]),
        _ => pair.to_owned(),
    });
    redact(&format!("{path}?{}", pairs.collect::<Vec<_>>().join("&")))
}

/// Text with every secret Cairn mints, and every client id, cut short.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("cairn_").or_else(|| rest.find("client_ey")) {
        out.push_str(&rest[..start]);
        let token: String = rest[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if token.len() > 40 {
            out.push_str(&token[..20]);
            out.push('…');
        } else {
            out.push_str(&token);
        }
        rest = &rest[start + token.len()..];
    }
    out.push_str(rest);
    out
}

fn strip_tags(html: &str) -> String {
    let mut text = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                text.push(' ');
            }
            _ if !inside => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
