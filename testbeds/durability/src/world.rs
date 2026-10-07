//! The service on the Turso store in one data directory, and the client that drives it: it
//! submits each planned step until it is acknowledged, retrying a failed write with the
//! same patch id (H5), and reopens the store in-process when writes keep failing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cairn_schema::{Actor, RankConstants};
use cairn_service::{
    Call, Capabilities, DeploymentSettings, DomainPatch, Parts, Service, WriteError, Written,
};
use cairn_store::{CommitPoint, Faults, InProcessNotifier, PauseHook, StoreError};
use cairn_store_turso::TursoStore;
use jiff::tz::{Offset, TimeZone};

use crate::ledger::{Entry, Ledger};
use crate::plan::Plan;

/// Consecutive failed writes after which the store is reopened in-process.
const REOPEN_AFTER: u32 = 3;
/// Consecutive failed writes after which the run gives up.
const ATTEMPTS: u32 = 64;

/// Why a run stopped short of a verdict on the store.
#[derive(Debug)]
pub enum Stop {
    /// The testbed's own machinery failed (the ledger could not be written).
    Abort { label: &'static str, detail: String },
    /// The store kept failing: nothing wrong was observed, but the run could not finish.
    Liveness(String),
    /// The store did not open.
    Unavailable(String),
    /// An invariant broke; its verdict has been reported.
    Violated,
}

/// The store's database file in `directory`.
pub fn database(directory: &Path) -> PathBuf {
    directory.join("cairn.db")
}

/// The service, its store, and the client's ledger.
pub struct World {
    pub directory: PathBuf,
    pub plan: Plan,
    pub ledger: Arc<Mutex<Ledger>>,
    faults: Faults,
    /// The open store and the service over it; none only while it is being reopened.
    open: Option<Open>,
    /// Opens tried before the store counts as unavailable: 1 but to reproduce the Turso gap
    /// of an open retried in-process (`--open-attempts`).
    open_attempts: u32,
    /// The planted client bug (`--bug ack-before-commit`): each step is recorded as
    /// acknowledged, at the revision it will produce, before it is submitted.
    pub ack_before_commit: bool,
    /// In-process reopens, after failures or a buggify.
    pub reopened: u32,
    /// Writes that failed and were retried.
    pub failures: u32,
}

impl World {
    /// Opens the store in `directory` with the commit-point hook, which records in the
    /// ledger each point the in-flight step's commit reaches, so a crash-restart sweep can
    /// stop the process right after one (`--fs-crash-at sync:N`).
    pub async fn open(
        directory: PathBuf,
        plan: Plan,
        ledger: Ledger,
        open_attempts: u32,
    ) -> Result<World, Stop> {
        let ledger = Arc::new(Mutex::new(ledger));
        let faults = Faults::default();
        let hooked = Arc::clone(&ledger);
        let hook: PauseHook = Arc::new(move |point: CommitPoint| {
            let mut ledger = lock(&hooked);
            if let Some(index) = ledger.in_flight {
                let point = format!("{point:?}");
                // A failure here is the ledger's, and the next append reports it.
                let _ = ledger.append(Entry::Point { index, point });
            }
            Box::pin(std::future::ready(()))
        });
        faults.pause_with(hook);
        let open = Open::new(&directory, &faults, open_attempts).await?;
        Ok(World {
            directory,
            plan,
            ledger,
            faults,
            open: Some(open),
            open_attempts,
            ack_before_commit: false,
            reopened: 0,
            failures: 0,
        })
    }

    fn opened(&self) -> &Open {
        match &self.open {
            Some(open) => open,
            None => unreachable!("the store is open outside a reopen"),
        }
    }

    /// The store.
    pub fn store(&self) -> &TursoStore {
        &self.opened().store
    }

    /// The service over the store.
    pub fn service(&self) -> &Service<TursoStore> {
        &self.opened().service
    }

    /// Closes the store and opens it again from disk: a clean in-process restart, which
    /// loses nothing the store acknowledged.
    pub async fn reopen(&mut self) -> Result<(), Stop> {
        self.open = None;
        self.open = Some(Open::new(&self.directory, &self.faults, self.open_attempts).await?);
        self.reopened += 1;
        Ok(())
    }

    /// The ledger's entries so far.
    pub fn entries(&self) -> Vec<Entry> {
        lock(&self.ledger).entries().to_vec()
    }

    /// Appends to the ledger, stopping the run if it cannot be written.
    pub fn record(&self, entry: Entry) -> Result<(), Stop> {
        lock(&self.ledger)
            .append(entry)
            .map_err(|error| Stop::Abort {
                label: "durability-ledger-writable",
                detail: error.to_string(),
            })
    }

    /// Submits step `index` until the service answers it, and records the acknowledgement.
    pub async fn submit(&mut self, index: usize) -> Result<Written, Stop> {
        self.record(Entry::Intent { index })?;
        let step = &self.plan.steps[index];
        let patch = match DomainPatch::new(step.patch.clone(), None) {
            Ok(patch) => patch,
            Err(error) => unreachable!("the plan writes only domain patches: {error}"),
        };
        let call = Call {
            actor: Actor {
                user: parse("u_client"),
                agent: None,
            },
            now: step.at,
        };
        if self.ack_before_commit {
            let revision = step.patch.base_revision.get() + 1;
            self.record(Entry::Ack { index, revision })?;
        }
        let mut failed = 0;
        let mut lost: Option<Written> = None;
        loop {
            match self.service().patch(&call, &patch).await {
                Ok(written) => {
                    // H5: a client that loses the answer submits the same patch again, and
                    // the store answers it from the receipt it kept.
                    if lost.is_none()
                        && patina_dst::buggify!("durability-client-loses-acknowledgement")
                    {
                        lost = Some(written);
                        continue;
                    }
                    if let Some(first) = lost.take() {
                        judge_resent(&first, &written)?;
                    }
                    if !self.ack_before_commit {
                        let revision = written.receipt().revision.get();
                        self.record(Entry::Ack { index, revision })?;
                    }
                    return Ok(written);
                }
                Err(WriteError::Rejected(rejection)) => {
                    // The plan is valid by construction (its tests), so the service or the
                    // store answered wrongly.
                    let detail = format!("step {index} was rejected: {rejection:?}");
                    report_violation("durability-planned-patch-accepted", &detail);
                    return Err(Stop::Violated);
                }
                Err(WriteError::Failed(error)) => {
                    eprintln!("DURABILITY_RETRY step={index} error={error}");
                    failed += 1;
                    self.failures += 1;
                    patina_dst::reachable!("durability-failed-write-retried");
                    if failed >= ATTEMPTS {
                        return Err(Stop::Liveness(format!(
                            "step {index} failed {failed} times: {error}"
                        )));
                    }
                    if failed % REOPEN_AFTER == 0 {
                        self.reopen().await?;
                    }
                }
            }
        }
    }
}

/// The store and the service over it, as a composition root assembles them.
struct Open {
    store: Arc<TursoStore>,
    service: Service<TursoStore>,
}

impl Open {
    async fn new(directory: &Path, faults: &Faults, attempts: u32) -> Result<Open, Stop> {
        let mut opened = Err(Stop::Unavailable("no open was attempted".to_owned()));
        for _ in 0..attempts.max(1) {
            opened = open_store(directory, faults).await;
            if opened.is_ok() {
                break;
            }
        }
        let store = Arc::new(opened?);
        let zone = TimeZone::fixed(Offset::constant(-5));
        let settings = DeploymentSettings::new(parse("Etc/GMT+5"), zone, RankConstants::default());
        let service = Service::new(Parts {
            store: Arc::clone(&store),
            notifier: Arc::new(InProcessNotifier::new()),
            settings,
            capabilities: Capabilities::server(Vec::new(), false),
        });
        Ok(Open { store, service })
    }
}

/// H5: a patch submitted again after its answer was lost is answered from its receipt, with
/// the receipt the first answer carried.
fn judge_resent(first: &Written, again: &Written) -> Result<(), Stop> {
    let from_receipt = matches!(again, Written::AlreadyApplied { .. });
    if from_receipt && first.receipt() == again.receipt() {
        return Ok(());
    }
    let detail = format!("a resent patch was answered {again:?} after {first:?}");
    report_violation("durability-resent-patch-answered-from-receipt", &detail);
    Err(Stop::Violated)
}

/// Reports a broken invariant: a `violation` verdict, echoed as a `DURABILITY_VIOLATION`
/// line.
pub fn report_violation(label: &str, detail: &str) {
    patina_dst::verdict(patina_dst::VerdictKind::Violation, label, detail);
    eprintln!("DURABILITY_VIOLATION {label} {detail}");
}

/// Turso panics, rather than answering an error, when an injected I/O error reaches these
/// points of an open
/// (decisions/2026-10-07-turso-an-open-under-injected-i-o-faults-panics-or-fails.md). An open
/// that panics with one of them is a store that did not open; any other panic is a finding
/// and goes on unwinding.
const TURSO_OPEN_PANICS: [&str; 1] = ["failed to get file size"];

/// Opens the store once. The world does not retry a failed open unless told to: a server
/// whose store does not open exits and is restarted by its supervisor, and Turso can panic
/// when an open is retried in the process a failed one ran in
/// (decisions/2026-10-07-turso-an-open-under-injected-i-o-faults-panics-or-fails.md).
async fn open_store(directory: &Path, faults: &Faults) -> Result<TursoStore, Stop> {
    let path = database(directory);
    let faults = faults.clone();
    let opening = tokio::spawn(async move { TursoStore::open_with_faults(&path, faults).await });
    match opening.await {
        Ok(opened) => opened.map_err(|error: StoreError| Stop::Unavailable(error.to_string())),
        Err(error) if error.is_panic() => {
            let payload = error.into_panic();
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&str>()
                        .map(|text| (*text).to_owned())
                })
                .unwrap_or_default();
            if TURSO_OPEN_PANICS
                .iter()
                .any(|known| message.contains(known))
            {
                return Err(Stop::Unavailable(format!("turso panicked: {message}")));
            }
            std::panic::resume_unwind(payload)
        }
        Err(error) => Err(Stop::Unavailable(format!(
            "the open was cancelled: {error}"
        ))),
    }
}

fn lock(ledger: &Mutex<Ledger>) -> MutexGuard<'_, Ledger> {
    ledger.lock().unwrap_or_else(PoisonError::into_inner)
}

fn parse<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    match text.parse() {
        Ok(value) => value,
        Err(error) => unreachable!("{text} parses: {error:?}"),
    }
}
