//! The durability invariants (PRACTICES, Simulation with patina: `durability`), read from
//! the store as it is after an open: every acknowledged commit is present, a commit is whole
//! (its state rows, receipt, and every event) or absent, the log holds only what the client
//! submitted, and the state the store loads equals the engine's replay of the log (J2, J3).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use cairn_engine::{Records, replay};
use cairn_schema::{Event, PatchId};
use cairn_service::{Service, ServiceError};
use cairn_store::{EventQuery, PageSize, Store, StoreError};
use cairn_store_turso::TursoStore;

use crate::ledger::{Entry, fnv};
use crate::plan::Plan;

/// The invariants; each broken one is a `violation` verdict under its label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Invariant {
    /// No acknowledged commit is missing, and each is at the revision it was acknowledged
    /// with.
    Acknowledged,
    /// A commit's receipt and all of its events are present together, or none of them.
    Whole,
    /// Every commit in the log is a planned patch the client submitted, once, and the
    /// committed patches are a prefix of the plan.
    OnlySubmitted,
    /// The state the store loads equals the replay of its log from empty (J3).
    ReplayEqualsState,
}

/// One broken invariant, with what broke it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub invariant: Invariant,
    pub detail: String,
}

/// What the store holds, judged against the plan and the client's ledger.
#[derive(Debug, Default)]
pub struct Audit {
    /// The steps whose commit is in the log.
    pub committed: BTreeSet<usize>,
    pub events: usize,
    /// A digest of the loaded state, the same in every run that commits the same steps.
    pub digest: u64,
    pub violations: Vec<Violation>,
}

impl Invariant {
    /// The verdict label.
    pub fn label(self) -> &'static str {
        match self {
            Invariant::Acknowledged => "durability-acknowledged-commit-present",
            Invariant::Whole => "durability-commit-whole-or-absent",
            Invariant::OnlySubmitted => "durability-log-holds-only-submitted",
            Invariant::ReplayEqualsState => "durability-state-equals-replayed-log",
        }
    }
}

impl Audit {
    fn broken(&mut self, invariant: Invariant, detail: String) {
        self.violations.push(Violation { invariant, detail });
    }
}

/// Failure to read what the audit needs: the store or service failed.
#[derive(Debug)]
pub enum Unreadable {
    Store(StoreError),
    Service(ServiceError),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unreadable::Store(error) => error.fmt(formatter),
            Unreadable::Service(error) => error.fmt(formatter),
        }
    }
}

impl From<StoreError> for Unreadable {
    fn from(error: StoreError) -> Self {
        Unreadable::Store(error)
    }
}

impl From<ServiceError> for Unreadable {
    fn from(error: ServiceError) -> Self {
        Unreadable::Service(error)
    }
}

/// Every event in the log, in order.
async fn all_events(store: &TursoStore) -> Result<Vec<(u64, Event)>, StoreError> {
    let mut events = Vec::new();
    let mut after = None;
    loop {
        let query = EventQuery {
            after,
            size: PageSize::MAX,
            ..EventQuery::default()
        };
        let page = store.events(&query).await?;
        events.extend(
            page.items
                .into_iter()
                .map(|logged| (logged.seq, logged.event)),
        );
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(events),
        }
    }
}

/// Audits the store against `plan` and the ledger's `entries`.
pub async fn audit(
    store: &TursoStore,
    service: &Service<TursoStore>,
    plan: &Plan,
    entries: &[Entry],
) -> Result<Audit, Unreadable> {
    let mut audit = Audit::default();
    let events = all_events(store).await?;
    audit.events = events.len();
    let intended: BTreeSet<usize> = entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Intent { index } => Some(*index),
            _ => None,
        })
        .collect();
    let acknowledged: BTreeMap<usize, u32> = entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Ack { index, revision } => Some((*index, *revision)),
            _ => None,
        })
        .collect();

    let whole_patches = log_shape(&mut audit, plan, &intended, &events);
    receipts(&mut audit, store, plan, &intended, &acknowledged).await?;
    for (index, revision) in &acknowledged {
        if !audit.committed.contains(index) {
            audit.broken(
                Invariant::Acknowledged,
                format!("step {index} was acknowledged at revision {revision} and has no events"),
            );
        }
    }
    let prefix = audit.committed.len();
    if audit.committed.iter().copied().ne(0..prefix) {
        audit.broken(
            Invariant::OnlySubmitted,
            format!(
                "the committed steps are not a prefix of the plan: {:?}",
                audit.committed
            ),
        );
    }
    if whole_patches {
        let events: Vec<Event> = events.into_iter().map(|(_, event)| event).collect();
        let replayed = replay(&Records::default(), &events);
        compare(&mut audit, service, &replayed).await?;
    }
    Ok(audit)
}

/// Checks the log's order and that it holds whole, planned, submitted patches, each once;
/// fills in the committed steps. Answers whether replay can run (it asserts whole patches).
fn log_shape(
    audit: &mut Audit,
    plan: &Plan,
    intended: &BTreeSet<usize>,
    events: &[(u64, Event)],
) -> bool {
    let mut whole = true;
    if events.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        audit.broken(
            Invariant::Whole,
            "the log's positions do not increase".to_owned(),
        );
        whole = false;
    }
    let mut groups: Vec<(&PatchId, Vec<&Event>)> = Vec::new();
    for (_, event) in events {
        match groups.last_mut() {
            Some((id, group)) if **id == event.patch_id && event.ordinal != 0 => group.push(event),
            _ => groups.push((&event.patch_id, vec![event])),
        }
    }
    for (id, group) in groups {
        let Some(index) = plan.index_of(id) else {
            audit.broken(
                Invariant::OnlySubmitted,
                format!("{id} is in the log and was never planned"),
            );
            whole = false;
            continue;
        };
        if !intended.contains(&index) {
            audit.broken(
                Invariant::OnlySubmitted,
                format!("step {index} is in the log and was never submitted"),
            );
        }
        if !audit.committed.insert(index) {
            audit.broken(
                Invariant::OnlySubmitted,
                format!("step {index} is in the log twice"),
            );
            whole = false;
        }
        let expected = plan.steps[index].event_count();
        let ordinals_in_order = group
            .iter()
            .enumerate()
            .all(|(at, event)| usize::try_from(event.ordinal).is_ok_and(|ordinal| ordinal == at));
        if group.len() != expected || !ordinals_in_order {
            audit.broken(
                Invariant::Whole,
                format!(
                    "step {index} has {} of its {expected} events, in order: {ordinals_in_order}",
                    group.len()
                ),
            );
            whole = false;
        }
    }
    whole
}

/// Checks each submitted step's receipt against its events and acknowledgement.
async fn receipts(
    audit: &mut Audit,
    store: &TursoStore,
    plan: &Plan,
    intended: &BTreeSet<usize>,
    acknowledged: &BTreeMap<usize, u32>,
) -> Result<(), StoreError> {
    for &index in intended.union(&audit.committed.clone()) {
        let patch = &plan.steps[index].patch;
        let receipt = store.receipt(&patch.id).await?;
        let logged = audit.committed.contains(&index);
        match (&receipt, logged) {
            (Some(_), false) => audit.broken(
                Invariant::Whole,
                format!("step {index} has a receipt and no events"),
            ),
            (None, true) => audit.broken(
                Invariant::Whole,
                format!("step {index} has events and no receipt"),
            ),
            _ => {}
        }
        let Some(receipt) = receipt else { continue };
        if receipt.content_hash != patch.content_hash() {
            audit.broken(
                Invariant::Whole,
                format!("step {index}'s receipt has another content hash"),
            );
        }
        if let Some(&revision) = acknowledged.get(&index)
            && receipt.revision.get() != revision
        {
            audit.broken(
                Invariant::Acknowledged,
                format!(
                    "step {index} was acknowledged at revision {revision}; its receipt says {}",
                    receipt.revision.get()
                ),
            );
        }
    }
    Ok(())
}

/// Compares what the service loads with the replayed records, journey by journey and the
/// deployment, and digests the loaded state.
async fn compare(
    audit: &mut Audit,
    service: &Service<TursoStore>,
    replayed: &Records,
) -> Result<(), ServiceError> {
    let mut digest = String::new();
    for id in Plan::journeys() {
        let loaded = service.journey(&id).await?;
        if loaded.as_ref() != replayed.journeys.get(&id) {
            let revision = |journey: Option<&cairn_schema::Journey>| {
                journey.map(|journey| journey.revision.get())
            };
            audit.broken(
                Invariant::ReplayEqualsState,
                format!(
                    "journey {id} loads at revision {:?} and replays to revision {:?}, and they differ",
                    revision(loaded.as_ref()),
                    revision(replayed.journeys.get(&id)),
                ),
            );
        }
        let _ = write!(digest, "{loaded:?}");
    }
    let deployment = service.deployment().await?;
    if deployment != replayed.deployment {
        audit.broken(
            Invariant::ReplayEqualsState,
            format!(
                "the deployment loads at revision {} and replays to revision {}, and they differ",
                deployment.revision.get(),
                replayed.deployment.revision.get()
            ),
        );
    }
    let _ = write!(digest, "{deployment:?}");
    audit.digest = fnv(digest.as_bytes());
    Ok(())
}
