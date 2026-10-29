//! The fixtures (fixtures/) as the integration tests use them: where they live, a journey
//! scenario, and the patch that publishes a fixture's route.
#![cfg(test)]

use std::path::{Path, PathBuf};

use cairn_engine::from_file;
use cairn_schema::{
    DraftSource, Mutation, Mutations, Patch, PatchTarget, Revision, RouteFile, Scenario,
    SequentialKeys, from_yaml,
};

/// The repository's fixtures directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A fixture's journey scenario.
pub fn scenario(name: &str) -> Scenario {
    let path = fixtures_root().join(name).join("journey.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// The patch that creates a fixture's route and publishes its file as version 1, as every
/// scenario expects (fixtures/README.md).
pub fn publish_fixture_route(name: &str) -> Patch {
    let route = fixtures_root().join(name).join("route.yaml");
    let path = if route.exists() {
        route
    } else {
        fixtures_root().join(name).join("segment.yaml")
    };
    let file: RouteFile = from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let graph = from_file(&file, &mut SequentialKeys::default())
        .unwrap()
        .into_document();
    let mut mutations = vec![
        Mutation::CreateRoute {
            name: file.name.clone(),
            description: file.description.clone(),
            kind: file.kind,
        },
        Mutation::OpenDraft {
            source: DraftSource::Import,
        },
    ];
    mutations.extend(cairn_engine::format::content(&graph));
    mutations.push(Mutation::PublishDraft {});
    Patch {
        id: format!("p_seed_{}", name.replace('-', "_"))
            .parse()
            .unwrap(),
        target: PatchTarget::Route(file.route.clone()),
        base_revision: Revision::NONE,
        deployment_revision: None,
        mutations: Mutations::new(mutations).unwrap(),
    }
}
