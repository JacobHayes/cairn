//! Transcripts for the proof (`briefs/proof/4.2/prove.sh`): when `CAIRN_API_TRANSCRIPT`
//! names a directory, every exchange a test's clients make is appended to a Markdown file
//! named after the test, as the curl command that would send it and the response. Agent
//! tokens are cut short and request ids masked, so a transcript holds no usable secret and
//! reads the same on every run. Without the variable, nothing is written.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER, WWW_AUTHENTICATE};
use cairn_api::client::{Exchange, Observer};

/// The host transcripts show requests going to; the test server's port varies per run.
const HOST: &str = "http://cairn.test";

/// Lines of a body shown before the rest is summarized.
const BODY_LINE_COUNT_SHOWN: usize = 60;

fn file() -> Option<PathBuf> {
    let directory = PathBuf::from(std::env::var_os("CAIRN_API_TRANSCRIPT")?);
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

/// Writes `text` into the transcript as it is: a heading or a sentence between exchanges.
pub fn note(text: &str) {
    append(&format!("{text}\n\n"));
}

/// The observer the support's clients carry.
pub fn observer() -> Observer {
    Arc::new(|exchange: &Exchange| {
        if file().is_some() {
            append(&render(exchange));
        }
    })
}

fn render(exchange: &Exchange) -> String {
    let mut text = format!(
        "```console\n$ curl -X {} '{HOST}{}'",
        exchange.method, exchange.target
    );
    for (name, value) in &exchange.request_headers {
        let value = value.to_str().unwrap_or("");
        let shown = match *name {
            AUTHORIZATION => redact(value),
            CONTENT_TYPE => value.to_owned(),
            _ => continue,
        };
        write!(text, " \\\n    -H '{name}: {shown}'").unwrap();
    }
    if !exchange.request_body.is_empty() {
        let body = shown_body(&exchange.request_body, true);
        write!(text, " \\\n    --data-binary @- <<'JSON'\n{body}\nJSON").unwrap();
    }
    write!(text, "\nHTTP/1.1 {}\n", exchange.status).unwrap();
    for (name, value) in &exchange.response_headers {
        let value = value.to_str().unwrap_or("");
        let shown = match name.as_str() {
            "x-request-id" => "rq_…".to_owned(),
            _ if *name == CONTENT_TYPE || *name == RETRY_AFTER || *name == WWW_AUTHENTICATE => {
                value.to_owned()
            }
            _ => continue,
        };
        writeln!(text, "{name}: {shown}").unwrap();
    }
    if !exchange.response_body.is_empty() {
        let json = exchange
            .response_headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json"));
        writeln!(text, "\n{}", shown_body(&exchange.response_body, json)).unwrap();
    }
    text.push_str("```\n\n");
    text
}

/// A body pretty-printed when it is JSON, with secrets cut short and long ones summarized.
fn shown_body(body: &[u8], json: bool) -> String {
    let text = String::from_utf8_lossy(body);
    let text = match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) if json => serde_json::to_string_pretty(&value).unwrap(),
        _ => text.trim_end().to_owned(),
    };
    let text = redact(&text);
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= BODY_LINE_COUNT_SHOWN {
        return text;
    }
    let mut shown = lines[..BODY_LINE_COUNT_SHOWN].join("\n");
    let rest = lines.len() - BODY_LINE_COUNT_SHOWN;
    write!(shown, "\n… ({rest} more lines)").unwrap();
    shown
}

/// Text with every secret Cairn mints cut short.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("cairn_") {
        out.push_str(&rest[..start]);
        let token: String = rest[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if token.len() > 24 {
            out.push_str(&token[..16]);
            out.push('…');
        } else {
            out.push_str(&token);
        }
        rest = &rest[start + token.len()..];
    }
    out.push_str(rest);
    out
}
