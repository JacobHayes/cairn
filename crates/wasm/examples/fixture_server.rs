//! The server the web shell's server host is tested against (brief 4.6): the HTTP API
//! (`cairn_api::router`) over a memory store seeded with every fixture as the in-browser root
//! is (one deployment, in UTC), every request signed in as one dev user by the dev provider
//! (H1), on loopback at the port given (0 picks one). It prints `listening on <address>` once
//! it serves. It is a test host, not the binary (4.7): no configuration, nothing persists.
//!
//! An OIDC provider named `stub` signs in against 3.2's stub issuer, started beside it on
//! loopback, so a browser test links an identity to the dev user (brief 5.5). The second
//! argument, when given, is the origin the browser reaches the API at (the app's dev server,
//! which proxies `/api/auth`), where the issuer sends the browser back.
//!
//! The assistant is mounted over 4.4's scripted provider (brief 5.8), so the capabilities
//! offer it and a browser test drives the panel with no model and no credential: the test
//! sets what the model will answer next with `PUT /fixture/assistant/script`, a list of
//! steps, each `{"say": text}` or `{"calls": [{"name", "arguments"}]}`, optionally with
//! `"after_ms"` to answer slowly. Each PUT replaces whatever the last one left unplayed.
//! The route sits outside auth: this is a test host on loopback.
//!
//! `cargo run -p cairn-wasm --example fixture_server -- <port> [<public origin>]`

#[path = "../../tests/tests/integration/auth/support/issuer.rs"]
#[allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    reason = "3.2's test issuer, written as test code"
)]
mod issuer;

use std::collections::BTreeSet;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::put;
use cairn_assistant::scripted::{ScriptedProvider, Step};
use cairn_assistant::{Assistant, Exchange, Provider, Reply, ToolCall};
use serde::Deserialize;
use serde_json::Value;

use cairn_auth::{
    Accounts, Auth, AuthProvider, Clock, DevConfig, DevProvider, Listener, OidcConfig, OidcProvider,
};
use cairn_schema::RankConstants;
use cairn_service::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts, Service};
use cairn_store::{InProcessNotifier, MemoryStore};
use jiff::tz::TimeZone;

fn main() -> Result<(), Box<dyn Error>> {
    let port: u16 = match std::env::args().nth(1) {
        Some(text) => text.parse()?,
        None => 0,
    };
    let public = std::env::args().nth(2);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(serve(port, public))
}

async fn serve(port: u16, public: Option<String>) -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let base: url::Url = match public {
        Some(origin) => format!("{origin}/").parse()?,
        None => format!("http://{address}/").parse()?,
    };
    let store = Arc::new(MemoryStore::new());
    let settings = DeploymentSettings::new("UTC".parse()?, TimeZone::UTC, RankConstants::default());
    let service = Service::new(Parts {
        store: Arc::clone(&store),
        notifier: Arc::new(InProcessNotifier::new()),
        settings,
        capabilities: Capabilities::server(
            vec![
                AuthMethod {
                    name: "dev".parse()?,
                    kind: AuthKind::Dev,
                },
                AuthMethod {
                    name: "stub".parse()?,
                    kind: AuthKind::Oidc,
                },
            ],
            true,
        ),
    });
    cairn_wasm::fixtures::seed(&service).await?;
    let dev = DevProvider::new(
        DevConfig {
            name: "dev".parse()?,
            user: "dev".parse()?,
            verified_emails: BTreeSet::new(),
            token: None,
            auto_link: false,
            allow_off_loopback: false,
        },
        Listener::Tcp(address),
    )?;
    let issuer = issuer::Issuer::start(Clock::system()).await;
    let accounts = Accounts::new(Arc::clone(&store), Clock::system());
    let stub = OidcProvider::new(
        OidcConfig {
            name: "stub".parse()?,
            issuer: issuer.url(),
            client_id: "cairn".to_owned(),
            client_secret: None,
            auto_link: false,
        },
        accounts.clone(),
        &base,
    )?;
    let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(dev), Arc::new(stub)];
    let auth = Auth::new(accounts, Listener::Tcp(address), &base, providers);
    let script = Arc::new(Script::default());
    let provider: Arc<dyn Provider> = Arc::clone(&script) as Arc<dyn Provider>;
    let assistant = Assistant::new(service.clone(), store, Clock::system(), provider);
    let scripting = axum::Router::new()
        .route("/fixture/assistant/script", put(set_script))
        .with_state(script);
    let app = cairn_api::router_with_assistant(service, &auth, Some(assistant)).merge(scripting);
    println!("listening on {address}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

/// The scripted provider the assistant asks, replaced whole by each `PUT` of a script.
#[derive(Default)]
struct Script(Mutex<Arc<ScriptedProvider>>);

impl Provider for Script {
    fn send<'a>(&'a self, exchange: &'a Exchange) -> cairn_assistant::provider::Answer<'a> {
        let current = Arc::clone(&self.0.lock().unwrap_or_else(PoisonError::into_inner));
        Box::pin(async move { current.send(exchange).await })
    }
}

/// One step of a script as a browser test writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptStep {
    #[serde(default)]
    say: Option<String>,
    #[serde(default)]
    calls: Vec<ScriptCall>,
    #[serde(default)]
    after_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptCall {
    name: String,
    arguments: Value,
}

/// `PUT /fixture/assistant/script`: what the model answers next, in order.
async fn set_script(
    State(script): State<Arc<Script>>,
    Json(steps): Json<Vec<ScriptStep>>,
) -> StatusCode {
    let steps = steps.into_iter().map(|step| {
        let calls = step
            .calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| ToolCall {
                id: format!("call_{index}"),
                name: call.name,
                arguments: call.arguments,
            })
            .collect();
        let reply = Reply {
            text: step.say,
            calls,
            raw: None,
        };
        match step.after_ms {
            Some(after) => Step::Slow(Duration::from_millis(after), reply),
            None => Step::Reply(reply),
        }
    });
    let played = Arc::new(ScriptedProvider::new(steps));
    *script.0.lock().unwrap_or_else(PoisonError::into_inner) = played;
    StatusCode::NO_CONTENT
}
