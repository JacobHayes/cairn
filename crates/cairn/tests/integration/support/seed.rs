//! The fixtures seeded into a database file through the service, as the in-browser root
//! seeds them (crates/wasm: one deployment, in UTC), so the binary serves them. Shared by
//! the binary's tests.

use std::path::Path;
use std::sync::Arc;

use cairn_schema::RankConstants;
use cairn_service::{AuthKind, AuthMethod, Capabilities, DeploymentSettings, Parts, Service};
use cairn_store::InProcessNotifier;
use cairn_store_turso::TursoStore;
use jiff::tz::TimeZone;

/// Seeds every fixture into the database at `path`, which must be new.
pub async fn seed_fixtures(path: &Path) -> Result<(), String> {
    let store = Arc::new(
        TursoStore::open(path)
            .await
            .map_err(|error| error.to_string())?,
    );
    let settings = DeploymentSettings::new(
        "UTC".parse().map_err(|error: String| error)?,
        TimeZone::UTC,
        RankConstants::default(),
    );
    let service = Service::new(Parts {
        store,
        notifier: Arc::new(InProcessNotifier::new()),
        settings,
        capabilities: Capabilities::server(
            vec![AuthMethod {
                name: "dev".parse().map_err(|error| format!("{error}"))?,
                kind: AuthKind::Dev,
            }],
            false,
        ),
    });
    cairn_wasm::fixtures::seed(&service)
        .await
        .map_err(|error| format!("{error:?}"))
}
