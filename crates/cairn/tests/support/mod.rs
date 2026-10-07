//! What the binary's tests share: configurations, a stand-in web build, the fixtures seeded
//! into a store, and plain HTTP/1.1 over a socket, so each test sets the `Host` it sends.

#![allow(dead_code)]

pub mod seed;

use std::borrow::Cow;
use std::fmt::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use cairn::assets::Asset;
use cairn::config::{self, Config, Environment};
use cairn::listener;
use cairn::root::Assembly;
use cairn_auth::Clock;
use cairn_store::{InProcessNotifier, MemoryStore};
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The public URL the test deployments are configured with.
pub const PUBLIC_HOST: &str = "cairn.example.com";

/// A deployment on a loopback listener at `https://cairn.example.com/`, in UTC, signed in by
/// one dev provider, with `extra` appended (more keys or tables).
pub fn config(extra: &str) -> Config {
    let text = format!(
        r#"
database = "unused.db"
listen = "127.0.0.1:0"
public_url = "https://{PUBLIC_HOST}/"
timezone = "UTC"
{extra}
"#
    );
    config::parse(&text, Path::new("."), &Environment::default()).unwrap()
}

/// The dev provider: every request is the dev user, or, with a token, every request bearing
/// it.
pub fn dev_provider(token: Option<&str>) -> String {
    let token = token.map_or(String::new(), |token| format!("token = \"{token}\""));
    format!("[[auth]]\nkind = \"dev\"\nname = \"dev\"\nuser = \"Dev\"\n{token}\n")
}

/// A stand-in web build: a page and one bundle.
pub fn assets() -> Vec<Asset> {
    vec![
        Asset {
            path: "index.html".to_owned(),
            bytes: Cow::Borrowed(b"<!doctype html><title>Cairn</title>"),
            media_type: "text/html".to_owned(),
        },
        Asset {
            path: "assets/index-test.js".to_owned(),
            bytes: Cow::Borrowed(b"export {};"),
            media_type: "text/javascript".to_owned(),
        },
    ]
}

/// The parts of a server over a memory store and `notifier`.
pub fn assembly(notifier: Arc<InProcessNotifier>) -> Assembly<MemoryStore> {
    Assembly {
        store: Arc::new(MemoryStore::new()),
        notifier,
        assets: assets(),
        metrics: PrometheusBuilder::new().build_recorder().handle(),
        clock: Clock::system(),
    }
}

/// Serves `app` on a fresh loopback port through the binary's listener, on this runtime.
pub async fn serve(app: Router) -> SocketAddr {
    let bound = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = bound.local_addr().unwrap();
    tokio::spawn(listener::serve(bound, app, std::future::pending()));
    address
}

/// An HTTP answer.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub head: String,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!("{error}: {}", self.text());
        })
    }
}

/// One request naming `host`, on its own connection, answered whole.
pub async fn send(
    address: SocketAddr,
    method: &str,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Reply {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\nhost: {host}\r\nconnection: close\r\n");
    for (name, value) in headers {
        write!(request, "{name}: {value}\r\n").unwrap();
    }
    let body = body.unwrap_or("");
    write!(request, "content-length: {}\r\n\r\n{body}", body.len()).unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).await.unwrap();
    parse(&answer)
}

/// A GET naming `host`.
pub async fn get(address: SocketAddr, host: &str, path: &str) -> Reply {
    send(address, "GET", host, path, &[], None).await
}

fn parse(answer: &[u8]) -> Reply {
    let split = answer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no head in {:?}", String::from_utf8_lossy(answer)));
    let head = String::from_utf8_lossy(&answer[..split]).into_owned();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    let raw = &answer[split + 4..];
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(raw)
    } else {
        raw.to_vec()
    };
    Reply { status, head, body }
}

fn dechunk(mut raw: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let line = raw.windows(2).position(|window| window == b"\r\n").unwrap();
        let size =
            usize::from_str_radix(std::str::from_utf8(&raw[..line]).unwrap().trim(), 16).unwrap();
        if size == 0 {
            return body;
        }
        body.extend_from_slice(&raw[line + 2..line + 2 + size]);
        raw = &raw[line + 2 + size + 2..];
    }
}
