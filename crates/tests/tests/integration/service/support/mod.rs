//! Shared helpers for the service's tests: services over each store, the
//! deployment's settings, and an executor.
#![allow(dead_code)]

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cairn_engine::{ApplyInputs, Records};
use cairn_schema::{Actor, Patch, ScenarioStep, Timestamp, from_yaml};
use cairn_service::{
    Call, Capabilities, DeploymentSettings, DomainPatch, McpAccess, Parts, Service, WriteError,
    Written,
};
use cairn_store::{InProcessNotifier, MemoryStore, Store};
use cairn_store_sqlite::SqliteStore;
use jiff::tz::{Offset, TimeZone};

/// The test deployment's time zone: five hours behind UTC all year (`Etc/GMT+5`), so a
/// midnight in it is 05:00 UTC.
pub fn settings() -> DeploymentSettings {
    let zone = TimeZone::fixed(Offset::constant(-5));
    DeploymentSettings::new("Etc/GMT+5".parse().unwrap(), zone)
}

/// A service over `store`, with its notifier.
pub fn service_over(store: Arc<dyn Store>) -> (Service, Arc<InProcessNotifier>) {
    let notifier = Arc::new(InProcessNotifier::new());
    let service = Service::new(Parts {
        store,
        notifier: notifier.clone(),
        settings: settings(),
        capabilities: Capabilities {
            auth: Vec::new(),
            assistant: false,
            mcp: McpAccess::closed("http://127.0.0.1/api/mcp"),
            version: "dev".to_owned(),
        },
    });
    (service, notifier)
}

/// A service over a fresh memory store.
pub fn memory() -> (Service, Arc<InProcessNotifier>) {
    service_over(Arc::new(MemoryStore::new()))
}

/// A fresh directory for an SQLite database under `root`, one per call in this process.
pub fn sqlite_path(root: &Path) -> PathBuf {
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

/// A service over a fresh SQLite store under `root`.
pub fn sqlite(root: &Path) -> (Service, Arc<InProcessNotifier>) {
    let store = SqliteStore::open(&sqlite_path(root)).unwrap();
    service_over(Arc::new(store))
}

/// Runs a future on a current-thread tokio runtime with its timer, as the SQLite backend
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
pub async fn vendor_after(service: &Service, steps: usize) {
    let (people_call, people) = crate::people::people_write();
    applied(service.patch(&people_call, &people).await);
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

/// A fresh SQLite store under `root`, shared.
pub fn sqlite_store(root: &Path) -> Arc<SqliteStore> {
    Arc::new(SqliteStore::open(&sqlite_path(root)).unwrap())
}

pub use crate::fixtures::{fixtures_root, publish_fixture_route, scenario};

/// A proposal drafted the way a test states it: a draft (title, base revision, mutations) or
/// a save as route, written through the one entry a patch with `proposal` takes.
pub trait Drafts {
    /// Creates proposal `id` for `destination` from `draft`, which holds no items.
    fn create_proposal<'a>(
        &'a self,
        call: &'a Call,
        patch_id: cairn_schema::PatchId,
        destination: &'a cairn_schema::Domain,
        id: &'a cairn_schema::ProposalId,
        draft: cairn_schema::ProposalDraft,
    ) -> impl Future<Output = Result<cairn_service::ProposalWritten, WriteError>> + Send + 'a;

    /// Proposes saving `journey` as a draft of `route`, named `name` when it is created.
    fn propose_save_as_route<'a>(
        &'a self,
        call: &'a Call,
        patch_id: cairn_schema::PatchId,
        id: &'a cairn_schema::ProposalId,
        journey: &'a cairn_schema::JourneyId,
        route: &'a cairn_schema::RouteId,
        name: &'a cairn_schema::Title,
    ) -> impl Future<Output = Result<cairn_service::ProposalWritten, cairn_service::ProposeError>>
    + Send
    + 'a;
}

impl Drafts for Service {
    async fn create_proposal(
        &self,
        call: &Call,
        patch_id: cairn_schema::PatchId,
        destination: &cairn_schema::Domain,
        id: &cairn_schema::ProposalId,
        draft: cairn_schema::ProposalDraft,
    ) -> Result<cairn_service::ProposalWritten, WriteError> {
        assert!(draft.items.is_empty(), "a patch drafts no review items");
        let patch = Patch {
            id: patch_id,
            target: destination.clone().into(),
            base_revision: draft.destination_revision,
            deployment_revision: None,
            mutations: cairn_schema::Mutations::new(draft.mutations.into_vec()).unwrap(),
        };
        let title: cairn_schema::Markdown = draft.title.as_str().parse().unwrap();
        let submitted = DomainPatch::new(patch, Some(title)).unwrap();
        let proposing = cairn_service::Proposing::named(Some(id.clone()), None)
            .unwrap()
            .unwrap();
        match self.propose(call, &submitted, &proposing).await {
            Ok(written) => Ok(written),
            Err(cairn_service::ProposeError::Write(error)) => Err(error),
            Err(other) => panic!("a proposal that should draft: {other}"),
        }
    }

    async fn propose_save_as_route(
        &self,
        call: &Call,
        patch_id: cairn_schema::PatchId,
        id: &cairn_schema::ProposalId,
        journey: &cairn_schema::JourneyId,
        route: &cairn_schema::RouteId,
        name: &cairn_schema::Title,
    ) -> Result<cairn_service::ProposalWritten, cairn_service::ProposeError> {
        let held = self.route(route).await?;
        let mut mutations = Vec::new();
        if held.is_none() {
            mutations.push(cairn_schema::Mutation::CreateRoute {
                name: name.clone(),
                description: None,
            });
        }
        mutations.push(cairn_schema::Mutation::OpenDraft {
            source: cairn_schema::DraftSource::SaveAsRoute {
                journey: journey.clone(),
            },
        });
        let patch = Patch {
            id: patch_id,
            target: cairn_schema::PatchTarget::Route(route.clone()),
            base_revision: held.map_or(cairn_schema::Revision::NONE, |found| found.revision),
            deployment_revision: None,
            mutations: cairn_schema::Mutations::new(mutations).unwrap(),
        };
        let submitted = DomainPatch::new(patch, None).unwrap();
        let proposing = cairn_service::Proposing::named(Some(id.clone()), None)
            .unwrap()
            .unwrap();
        self.propose(call, &submitted, &proposing).await
    }
}
