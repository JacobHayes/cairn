//! The fixtures seeded into a database file through the service, as `cairn demo`
//! seeds them (one deployment, in UTC), so the binary serves them. Shared by
//! the binary's tests.

use std::path::Path;
use std::sync::Arc;

use cairn_service::{
    AuthKind, AuthMethod, Capabilities, DeploymentSettings, McpAccess, Parts, Service,
};
use cairn_store::InProcessNotifier;
use cairn_store_sqlite::SqliteStore;
use jiff::tz::TimeZone;

/// Seeds every fixture into the database at `path`, which must be new.
pub async fn seed_fixtures(path: &Path) -> Result<(), String> {
    let store = Arc::new(SqliteStore::open(path).map_err(|error| error.to_string())?);
    let settings =
        DeploymentSettings::new("UTC".parse().map_err(|error: String| error)?, TimeZone::UTC);
    let service = Service::new(Parts {
        store,
        notifier: Arc::new(InProcessNotifier::new()),
        settings,
        capabilities: Capabilities {
            auth: vec![AuthMethod {
                name: "dev".parse().map_err(|error| format!("{error}"))?,
                kind: AuthKind::Dev,
            }],
            assistant: false,
            mcp: McpAccess::closed("http://127.0.0.1/api/mcp"),
            version: "dev".to_owned(),
        },
    });
    cairn::fixtures::seed(&service)
        .await
        .map_err(|error| format!("{error:?}"))
}
