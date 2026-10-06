//! What the assistant's tests share: the assistant over a fresh memory store with a scripted
//! provider, the tool set beneath it, a clock the test sets, actors, and set-up written
//! through the service: the fixtures' route published and the vendor evaluation started.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use cairn_assistant::scripted::{ScriptedProvider, Step};
use cairn_assistant::{Assistant, Target, TurnReply, assistant_agent};
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
use serde_json::{Value, json};

/// The assistant over a fresh memory store, its provider's script, and what is beneath it.
pub struct World {
    pub assistant: Assistant<MemoryStore>,
    pub provider: Arc<ScriptedProvider>,
    pub tools: ToolSet<MemoryStore>,
    pub service: Service<MemoryStore>,
    pub store: Arc<MemoryStore>,
    now: Arc<AtomicI64>,
}

impl World {
    /// An empty deployment with the assistant, whose provider plays nothing yet.
    pub fn new() -> Self {
        let zone = TimeZone::fixed(Offset::constant(-5));
        let settings =
            DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone, RankConstants::default());
        let store = Arc::new(MemoryStore::new());
        let service = Service::new(Parts {
            store: Arc::clone(&store),
            notifier: Arc::new(InProcessNotifier::new()),
            settings,
            capabilities: Capabilities::server(Vec::new(), true),
        });
        let now = Arc::new(AtomicI64::new(0));
        let seconds = Arc::clone(&now);
        let clock =
            Clock::from_fn(move || Timestamp::from_second(seconds.load(Ordering::SeqCst)).unwrap());
        let provider = Arc::new(ScriptedProvider::default());
        let assistant = Assistant::new(
            service.clone(),
            Arc::clone(&store),
            clock.clone(),
            Arc::clone(&provider) as Arc<dyn cairn_assistant::Provider>,
        );
        let world = Self {
            assistant,
            provider,
            tools: ToolSet::new(service.clone(), clock),
            service,
            store,
            now,
        };
        world.set_clock("2026-10-01T14:00:00Z");
        world
    }

    /// Sets the clock to `at`, an RFC 3339 time.
    pub fn set_clock(&self, at: &str) {
        let at: Timestamp = at.parse().unwrap();
        self.now.store(at.as_second(), Ordering::SeqCst);
    }

    /// The clock's now.
    pub fn now(&self) -> Timestamp {
        Timestamp::from_second(self.now.load(Ordering::SeqCst)).unwrap()
    }

    /// Appends `steps` to the provider's script.
    pub fn script(&self, steps: impl IntoIterator<Item = Step>) {
        self.provider.push(steps);
    }

    /// One turn of `actor`'s conversation about `target`, which must run.
    pub async fn turn(&self, actor: &Actor, target: &Target, message: &str) -> TurnReply {
        let message = message.parse().unwrap();
        match self.assistant.turn(actor, target.clone(), message).await {
            Ok(reply) => reply,
            Err(error) => panic!("the turn did not run: {error}"),
        }
    }

    /// Calls the tool `name` as `actor`, which must succeed.
    pub async fn ok(&self, actor: &Actor, name: &str, arguments: Value) -> Value {
        match self.tools.call(actor, name, arguments).await {
            Ok(output) => output,
            Err(error) => panic!("{name}: {error}"),
        }
    }

    /// Calls the tool `name` as `actor`.
    pub async fn call(
        &self,
        actor: &Actor,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        self.tools.call(actor, name, arguments).await
    }

    /// The journey's current revision.
    pub async fn journey_revision(&self, journey: &str) -> u64 {
        let snapshot = self
            .ok(
                &user("u_reader"),
                "get_snapshot",
                json!({ "journey": journey }),
            )
            .await;
        snapshot["revision"].as_u64().unwrap()
    }

    /// The route's current revision, its draft included (0 when it does not exist).
    pub async fn route_revision(&self, route: &str) -> u64 {
        let route = self.service.route(&route.parse().unwrap()).await.unwrap();
        route.map_or(0, |route| u64::from(route.revision.get()))
    }

    /// Applies `patch` as `actor` through the service: set-up that is not what a test checks.
    pub async fn patch(&self, actor: &Actor, patch: Patch) {
        let call = Call {
            actor: actor.clone(),
            now: self.now(),
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

    /// Starts the bake-off's empty journey, as its owner (its scenario's first step): the
    /// journey at revision 1, with no route.
    pub async fn empty_bakeoff(&self) {
        let first = scenario("bake-off").steps.as_slice()[0].patch.clone();
        self.patch(&user("u_owner"), first).await;
    }

    /// Opens the vendor evaluation's draft for editing, as `actor`: the route's revision
    /// after it.
    pub async fn open_vendor_draft(&self, actor: &Actor) -> u64 {
        let base = self.route_revision("vendor-evaluation").await;
        let open = json!({ "route": "vendor-evaluation", "patch_id": "p_open_vendor_draft",
            "base_revision": base });
        let opened = self.ok(actor, "open_draft", open).await;
        opened["receipt"]["revision"].as_u64().unwrap()
    }
}

/// A fixture's journey scenario.
pub fn scenario(fixture: &str) -> Scenario {
    let path = fixtures_root().join(fixture).join("journey.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// The mutations of a scenario step's patch, as JSON.
pub fn step_mutations(scenario: &Scenario, step: usize) -> Vec<Value> {
    let patch = &scenario.steps.as_slice()[step].patch;
    patch
        .mutations
        .as_slice()
        .iter()
        .map(|mutation| serde_json::to_value(mutation).unwrap())
        .collect()
}

/// The mutations a scenario step's proposal drafts, as JSON.
pub fn proposed_mutations(scenario: &Scenario, step: usize) -> Vec<Value> {
    let patch = &scenario.steps.as_slice()[step].patch;
    let Mutation::CreateProposal { proposal } = &patch.mutations.as_slice()[0] else {
        panic!("step {step} drafts no proposal");
    };
    proposal
        .mutations
        .as_slice()
        .iter()
        .map(|mutation| serde_json::to_value(mutation).unwrap())
        .collect()
}

/// A user acting directly.
pub fn user(name: &str) -> Actor {
    Actor {
        user: name.parse().unwrap(),
        agent: None,
    }
}

/// The assistant acting for `name` (H2).
pub fn assisting(name: &str) -> Actor {
    Actor {
        user: name.parse().unwrap(),
        agent: Some(assistant_agent()),
    }
}

/// The vendor evaluation journey as a target.
pub fn vendor_target() -> Target {
    Target::Journey("j_vendor_eval".parse().unwrap())
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
    mutations.push(Mutation::PublishDraft);
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
