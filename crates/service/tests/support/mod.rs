//! Shared helpers for the service's tests and its proof: services over each store, the
//! deployment's settings, and an executor.
#![allow(dead_code)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cairn_engine::{ApplyInputs, Records, from_file};
use cairn_schema::{
    Actor, DraftSource, Mutation, Mutations, Patch, PatchTarget, RankConstants, Revision,
    RouteFile, Scenario, ScenarioStep, SequentialKeys, Timestamp, from_yaml,
};
use cairn_service::{
    Call, Capabilities, DeploymentSettings, DomainPatch, Parts, Service, WriteError, Written,
};
use cairn_store::{InProcessNotifier, MemoryStore, Store};
use cairn_store_turso::TursoStore;
use jiff::tz::{Offset, TimeZone};

/// The test deployment's time zone: five hours behind UTC all year (`Etc/GMT+5`), so a
/// midnight in it is 05:00 UTC.
pub fn settings() -> DeploymentSettings {
    let zone = TimeZone::fixed(Offset::constant(-5));
    DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone, RankConstants::default())
}

/// A service over `store`, with its notifier.
pub fn service_over<S: Store>(store: Arc<S>) -> (Service<S>, Arc<InProcessNotifier>) {
    let notifier = Arc::new(InProcessNotifier::new());
    let service = Service::new(Parts {
        store,
        notifier: notifier.clone(),
        settings: settings(),
        capabilities: Capabilities::server(Vec::new(), false),
    });
    (service, notifier)
}

/// A service over a fresh memory store.
pub fn memory() -> (Service<MemoryStore>, Arc<InProcessNotifier>) {
    service_over(Arc::new(MemoryStore::new()))
}

/// A fresh directory for a Turso database under `root`, one per call in this process.
pub fn turso_path(root: &Path) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = root.join(format!(
        "service-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("cairn.db")
}

/// A service over a fresh Turso store under `root`.
pub async fn turso(root: &Path) -> (Service<TursoStore>, Arc<InProcessNotifier>) {
    let store = TursoStore::open(&turso_path(root)).await.unwrap();
    service_over(Arc::new(store))
}

/// Runs a future on a current-thread tokio runtime with its timer, as the Turso backend
/// needs; the memory backend needs none, but runs the same way.
pub fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

/// A user acting directly at `now`.
pub fn call(user: &str, now: &str) -> Call {
    Call {
        actor: Actor {
            user: user.parse().unwrap(),
            agent: None,
        },
        now: now.parse::<Timestamp>().unwrap(),
    }
}

/// An agent acting for `user` at `now`.
pub fn agent_call(user: &str, agent: &str, now: &str) -> Call {
    let mut call = call(user, now);
    call.actor.agent = Some(agent.parse().unwrap());
    call
}

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
/// scenario expects (fixtures/README.md): the route, a draft opened for an import, its
/// roles, kinds, default owner, and nodes, and the publish.
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
            .map(|role| Mutation::AddRole { role: role.clone() }),
    );
    mutations.extend(
        graph
            .participation_kinds
            .values()
            .map(|kind| Mutation::AddParticipationKind { kind: kind.clone() }),
    );
    if let Some(role) = &graph.default_owner {
        mutations.push(Mutation::SetDefaultOwner {
            role: Some(role.clone()),
        });
    }
    mutations.extend(
        graph
            .nodes
            .values()
            .map(|node| Mutation::AddNode { node: node.clone() }),
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

/// The call a scenario step is submitted with: its actor, at its time.
pub fn step_call(step: &ScenarioStep) -> Call {
    Call {
        actor: step.actor.clone(),
        now: step.at,
    }
}

/// A scenario step as a domain patch with its note.
pub fn step_patch(step: &ScenarioStep) -> DomainPatch {
    DomainPatch::new(step.patch.clone(), Some(step.note.clone())).unwrap()
}

/// The same step applied by the engine alone to `records`, as the store-free reference.
pub fn engine_step(records: &Records, patch: &Patch, call: &Call, note: Option<&str>) -> Records {
    let inputs = ApplyInputs {
        today: settings().today(call.now),
        at: call.now,
        actor: call.actor.clone(),
        note: note.map(|text| text.parse().unwrap()),
    };
    match cairn_engine::apply(records, patch, &inputs) {
        Ok(applied) => applied.records().clone(),
        Err(rejection) => panic!("{}: {rejection:#?}", patch.id),
    }
}

/// A patch from YAML: `id`, `target` (as YAML), the base revision, and the mutations.
pub fn patch(id: &str, target: &str, base: u32, mutations: &str) -> Patch {
    let yaml =
        format!("id: {id}\ntarget: {target}\nbase_revision: {base}\nmutations:\n{mutations}");
    from_yaml(&yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"))
}

/// A domain patch with no note.
pub fn domain(patch: Patch) -> DomainPatch {
    DomainPatch::new(patch, None).unwrap()
}

/// The receipt of an applied write, or a panic naming what came back.
pub fn applied(written: Result<Written, WriteError>) -> Written {
    match written {
        Ok(written @ Written::Applied { .. }) => written,
        other => panic!("expected an applied write, got {other:#?}"),
    }
}

/// The rejection of a write, or a panic naming what came back.
pub fn rejected(written: Result<Written, WriteError>) -> cairn_schema::Rejection {
    match written {
        Err(WriteError::Rejected(rejection)) => rejection,
        other => panic!("expected a rejection, got {other:#?}"),
    }
}

/// Seeds the vendor evaluation's route and runs its scenario's first `steps` steps.
pub async fn vendor_after<S: Store>(service: &Service<S>, steps: usize) {
    let seed = publish_fixture_route("vendor-evaluation");
    applied(
        service
            .patch(&call("u_author", "2026-09-01T12:00:00Z"), &domain(seed))
            .await,
    );
    for step in scenario("vendor-evaluation")
        .steps
        .as_slice()
        .iter()
        .take(steps)
    {
        applied(service.patch(&step_call(step), &step_patch(step)).await);
    }
}

/// A fresh Turso store under `root`, shared.
pub async fn turso_store(root: &Path) -> Arc<TursoStore> {
    Arc::new(TursoStore::open(&turso_path(root)).await.unwrap())
}
