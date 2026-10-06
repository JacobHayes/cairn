//! The server the web shell's server host is tested against (brief 4.6): the HTTP API
//! (`cairn_api::router`) over a memory store seeded with every fixture as the in-browser root
//! is (one deployment, in UTC), every request signed in as one dev user by the dev provider
//! (H1), on loopback at the port given (0 picks one). It prints `listening on <address>` once
//! it serves. It is a test host, not the binary (4.7): no configuration, nothing persists.
//!
//! `cargo run -p cairn-wasm --example fixture_server -- <port>`

use std::collections::BTreeSet;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

use cairn_auth::{Accounts, Auth, AuthProvider, Clock, DevConfig, DevProvider, Listener};
use cairn_schema::RankConstants;
use cairn_service::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts, Service};
use cairn_store::{InProcessNotifier, MemoryStore};
use jiff::tz::TimeZone;

fn main() -> Result<(), Box<dyn Error>> {
    let port: u16 = match std::env::args().nth(1) {
        Some(text) => text.parse()?,
        None => 0,
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(serve(port))
}

async fn serve(port: u16) -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let address = listener.local_addr()?;
    let store = Arc::new(MemoryStore::new());
    let settings = DeploymentSettings::new("UTC".parse()?, TimeZone::UTC, RankConstants::default());
    let service = Service::new(Parts {
        store: Arc::clone(&store),
        notifier: Arc::new(InProcessNotifier::new()),
        settings,
        capabilities: Capabilities::server(
            vec![AuthMethod {
                name: "dev".parse()?,
                kind: AuthKind::Dev,
            }],
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
    let providers: Vec<Arc<dyn AuthProvider>> = vec![Arc::new(dev)];
    let base = format!("http://{address}").parse()?;
    let accounts = Accounts::new(store, Clock::system());
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
