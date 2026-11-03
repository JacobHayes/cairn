//! The fixtures (fixtures/) as the integration tests use them: where they live, a journey
//! scenario, the patch that publishes a fixture's route, and playing a fixture into a service.
#![cfg(test)]

use std::path::{Path, PathBuf};

use cairn_schema::{Patch, Scenario, Timestamp, from_yaml};
use cairn_service::Service;

/// The repository's fixtures directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A fixture's journey scenario.
pub fn scenario(name: &str) -> Scenario {
    let path = fixtures_root().join(name).join("journey.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// A fixture's route file.
fn route_text(name: &str) -> String {
    std::fs::read_to_string(fixtures_root().join(name).join("route.yaml")).unwrap()
}

/// The patch that creates a fixture's route and publishes its file as version 1, as every
/// scenario expects (fixtures/README.md).
pub fn publish_fixture_route(name: &str) -> Patch {
    cairn_service::route_patch(name, &route_text(name)).unwrap()
}

/// Plays fixture `name` into `service`: the shared people, its route when it has one, and its
/// scenario's first `first` steps or all of them, each as its actor at its time. Answers the
/// time of the last step played, the clock a test continues from.
pub async fn play_fixture(
    service: &Service,
    name: &str,
    first: Option<usize>,
) -> Option<Timestamp> {
    let people = crate::people::people_step();
    cairn_service::play(service, &[people], None).await.unwrap();
    if fixtures_root().join(name).join("route.yaml").exists() {
        cairn_service::publish_route(service, name, &route_text(name))
            .await
            .unwrap();
    }
    let scenario = scenario(name);
    let steps = scenario.steps.as_slice();
    cairn_service::play(service, steps, first).await.unwrap();
    steps
        .iter()
        .take(first.unwrap_or(steps.len()))
        .next_back()
        .map(|step| step.at)
}
