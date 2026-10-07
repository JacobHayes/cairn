//! The server the web shell's server host is tested against (brief 4.6): the HTTP API
//! (`cairn_api::router`) over a memory store seeded with every fixture as the in-browser root
//! is (one deployment, in UTC), every request signed in as one dev user by the dev provider
//! (H1), on loopback at the port given (0 picks one). It prints `listening on <address>` once
//! it serves. It is a test host, not the binary (4.7): no configuration, nothing persists.
//!
//! An OIDC provider named `stub` signs in against 3.2's stub issuer, started beside it on
//! loopback, so a browser test links an identity to the dev user (brief 5.5). The second
//! argument, when given, is the origin the browser reaches the API at (the app's dev server,
//! which proxies `/auth`), where the issuer sends the browser back.
//!
//! `cargo run -p cairn-wasm --example fixture_server -- <port> [<public origin>]`

#[path = "../../auth/tests/integration/support/issuer.rs"]
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
use std::sync::Arc;

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
            false,
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
    let accounts = Accounts::new(store, Clock::system());
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
    let app = cairn_api::router(service, &auth);
    println!("listening on {address}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
