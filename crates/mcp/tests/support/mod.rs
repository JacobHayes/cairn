//! What the tool set's tests share: the tools over a fresh memory store, a clock the test
//! sets, actors, and set-up written through the service: the fixtures' route published and
//! the vendor evaluation started.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use cairn_auth::Clock;
use cairn_engine::from_file;
use cairn_mcp::{ToolError, ToolSet};
use cairn_schema::{
    Actor, DraftSource, Mutation, Mutations, Patch, PatchTarget, RankConstants, Revision,
    RouteFile, Scenario, SequentialKeys, Timestamp, from_yaml,
};
use cairn_service::{Call, Capabilities, DeploymentSettings, DomainPatch, Parts, Service};
use cairn_store::{InProcessNotifier, MemoryStore};
use jiff::tz::{Offset, TimeZone};
use serde_json::Value;

/// The tool set over a fresh memory store, the service beneath it, and the clock its calls
/// read.
pub struct World {
    pub tools: ToolSet<MemoryStore>,
    pub service: Service<MemoryStore>,
    now: Arc<AtomicI64>,
}

impl World {
    /// The tools over an empty deployment, at the vendor evaluation's first step.
    pub fn new() -> Self {
        let zone = TimeZone::fixed(Offset::constant(-5));
        let settings =
            DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone, RankConstants::default());
        let service = Service::new(Parts {
            store: Arc::new(MemoryStore::new()),
            notifier: Arc::new(InProcessNotifier::new()),
            settings,
            capabilities: Capabilities::server(Vec::new(), false),
        });
        let now = Arc::new(AtomicI64::new(0));
        let seconds = Arc::clone(&now);
        let clock =
            Clock::from_fn(move || Timestamp::from_second(seconds.load(Ordering::SeqCst)).unwrap());
        let tools = Self {
            tools: ToolSet::new(service.clone(), clock),
            service,
            now,
        };
        tools.set_clock("2026-10-01T14:00:00Z");
        tools
    }

    /// Sets the clock to `at`, an RFC 3339 time.
    pub fn set_clock(&self, at: &str) {
        let at: Timestamp = at.parse().unwrap();
        self.now.store(at.as_second(), Ordering::SeqCst);
    }

    /// Calls `name` as `actor`.
    pub async fn call(
        &self,
        actor: &Actor,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        self.tools.call(actor, name, arguments).await
    }

    /// Calls `name` as `actor`, which must succeed.
    pub async fn ok(&self, actor: &Actor, name: &str, arguments: Value) -> Value {
        match self.call(actor, name, arguments).await {
            Ok(output) => output,
            Err(error) => panic!("{name}: {error}"),
        }
    }

    /// Applies `patch` as `actor` through the service: set-up that is not what a test checks.
    pub async fn patch(&self, actor: &Actor, patch: Patch) {
        let call = Call {
            actor: actor.clone(),
            now: Timestamp::from_second(self.now.load(Ordering::SeqCst)).unwrap(),
        };
        let patch = DomainPatch::new(patch, None).unwrap();
        self.service.patch(&call, &patch).await.unwrap();
    }

    /// Publishes a fixture's route as version 1.
    pub async fn publish(&self, actor: &Actor, fixture: &str) {
        self.patch(actor, publish_fixture_route(fixture)).await;
    }

    /// Publishes the vendor evaluation and starts `j_vendor_eval` from its version 1 with
    /// its people (the scenario's first step): the journey at revision 1.
    pub async fn vendor_journey(&self, actor: &Actor) {
        self.publish(actor, "vendor-evaluation").await;
        let path = fixtures_root()
            .join("vendor-evaluation")
            .join("journey.yaml");
        let scenario: Scenario = from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let first = scenario.steps.as_slice()[0].patch.clone();
        self.patch(actor, first).await;
    }
}

/// A user acting directly.
pub fn user(name: &str) -> Actor {
    Actor {
        user: name.parse().unwrap(),
        agent: None,
    }
}

/// An agent acting for a user (H2, I7).
pub fn agent(name: &str, user: &str) -> Actor {
    Actor {
        user: user.parse().unwrap(),
        agent: Some(name.parse().unwrap()),
    }
}

/// The repository's fixtures directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// The patch that creates a fixture's route and publishes its file as version 1, as every
/// scenario expects (fixtures/README.md).
pub fn publish_fixture_route(name: &str) -> Patch {
    let path = fixtures_root().join(name).join("route.yaml");
    let file: RouteFile = from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let graph = from_file(&file, &mut SequentialKeys::default())
        .unwrap()
        .into_document();
    let mut mutations = vec![
        Mutation::CreateRoute {
            name: file.name.clone(),
            description: file.description.clone(),
        },
        Mutation::OpenDraft {
            source: DraftSource::Import,
        },
    ];
    mutations.extend(
        graph
            .roles
            .values()
            .cloned()
            .map(|role| Mutation::AddRole { role }),
    );
    let kinds = graph.participation_kinds.values().cloned();
    mutations.extend(kinds.map(|kind| Mutation::AddParticipationKind { kind }));
    if let Some(role) = &graph.default_owner {
        mutations.push(Mutation::SetDefaultOwner {
            role: Some(role.clone()),
        });
    }
    mutations.extend(
        graph
            .nodes
            .values()
            .cloned()
            .map(|node| Mutation::AddNode { node }),
    );
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
