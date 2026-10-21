//! The store conformance suite (PRACTICES, Testing > Shell: conformance and integration):
//! one function per case over a [`Backend`] that opens fresh stores, so every backend runs
//! the same cases. A backend's test file expands [`conformance_suite!`](crate::conformance_suite)
//! with its backend and an executor, giving one test per case:
//!
//! ```ignore
//! mod conformance {
//!     cairn_store::conformance_suite!(cairn_store::conformance::Memory, cairn_store::conformance::block_on);
//! }
//! ```
//!
//! The suite tests persistence, not validation: it builds the change sets an engine would
//! produce directly ([`crate::build`]), and the memory backend is the reference it was
//! written against.
//!
//! Test code: a failed expectation is a failed case, so the suite unwraps and panics.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

pub mod commits;
pub mod every;
pub mod queries;
pub mod records;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use cairn_schema::{
    Deployment, Journey, JourneyId, PatchReceipt, Proposal, ProposalId, Route, RouteId,
};

use crate::commit::{Commit, CommitError, Committed};
use crate::faults::Faults;
use crate::memory::MemoryStore;
use crate::query::{EventQuery, LoggedEvent, PageSize, Revisions};
use crate::store::Store;
use crate::target::{Document, LoadTarget};

/// Opens fresh, empty stores of one backend.
pub trait Backend {
    /// The store.
    type Store: Store;

    /// A fresh, empty store whose commits pass through `faults`.
    fn open(&self, faults: Faults) -> impl Future<Output = Self::Store>;
}

/// The memory backend, the reference.
#[derive(Clone, Copy, Debug, Default)]
pub struct Memory;

impl Backend for Memory {
    type Store = MemoryStore;

    fn open(&self, faults: Faults) -> impl Future<Output = MemoryStore> {
        std::future::ready(MemoryStore::with_faults(faults))
    }
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Runs a future to completion on this thread: enough for a backend whose futures need no
/// runtime, such as the memory backend.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/conformance_suite.rs"));

/// Opens a store with no faults armed.
pub async fn open<B: Backend>(backend: &B) -> B::Store {
    backend.open(Faults::default()).await
}

/// Commits, expecting it to apply.
pub async fn applied(store: &dyn Store, commit: Commit) -> PatchReceipt {
    match store.commit(commit).await {
        Ok(Committed::Applied { receipt, .. }) => receipt,
        other => panic!("expected the commit to apply, got {other:?}"),
    }
}

/// Commits, expecting it to fail.
pub async fn failed(store: &dyn Store, commit: Commit) -> CommitError {
    match store.commit(commit).await {
        Err(error) => error,
        Ok(committed) => panic!("expected the commit to fail, got {committed:?}"),
    }
}

/// A journey as loaded.
pub async fn journey(store: &dyn Store, id: &str) -> Option<Journey> {
    match store
        .load(&LoadTarget::Journey(crate::build::id(id)))
        .await
        .unwrap()
    {
        Some(Document::Journey(journey)) => Some(journey),
        None => None,
        Some(other) => panic!("a journey load returned {other:?}"),
    }
}

/// A route as loaded.
pub async fn route(store: &dyn Store, id: &str) -> Option<Route> {
    match store
        .load(&LoadTarget::Route(crate::build::id(id)))
        .await
        .unwrap()
    {
        Some(Document::Route(route)) => Some(route),
        None => None,
        Some(other) => panic!("a route load returned {other:?}"),
    }
}

/// The deployment as loaded.
pub async fn deployment(store: &dyn Store) -> Deployment {
    match store.load(&LoadTarget::Deployment).await.unwrap() {
        Some(Document::Deployment(deployment)) => deployment,
        other => panic!("the deployment load returned {other:?}"),
    }
}

/// Every event in the log, in order.
pub async fn all_events(store: &dyn Store) -> Vec<LoggedEvent> {
    let mut events = Vec::new();
    let mut after = None;
    loop {
        let query = EventQuery {
            after,
            size: PageSize::MAX,
            ..EventQuery::default()
        };
        let page = store.events(&query).await.unwrap();
        events.extend(page.items);
        match page.next {
            Some(next) => after = Some(next),
            None => return events,
        }
    }
}

/// Everything a store holds that a commit could change, for "left untouched" checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The revisions.
    pub revisions: Revisions,
    /// The deployment.
    pub deployment: Deployment,
    /// Every journey.
    pub journeys: BTreeMap<JourneyId, Journey>,
    /// Every route.
    pub routes: BTreeMap<RouteId, Route>,
    /// Every proposal.
    pub proposals: BTreeMap<ProposalId, Proposal>,
    /// The log.
    pub events: Vec<LoggedEvent>,
}

/// Takes a snapshot of everything a commit could change.
pub async fn snapshot(store: &dyn Store) -> Snapshot {
    let revisions = store.revisions().await.unwrap();
    let mut journeys = BTreeMap::new();
    for id in revisions.journeys.keys() {
        if let Some(loaded) = journey(store, id.as_str()).await {
            journeys.insert(id.clone(), loaded);
        }
    }
    let mut routes = BTreeMap::new();
    for id in revisions.routes.keys() {
        if let Some(loaded) = route(store, id.as_str()).await {
            routes.insert(id.clone(), loaded);
        }
    }
    let mut proposals = BTreeMap::new();
    for id in revisions.proposals.keys() {
        if let Some(loaded) = store.proposal(id).await.unwrap() {
            proposals.insert(id.clone(), loaded);
        }
    }
    Snapshot {
        deployment: deployment(store).await,
        events: all_events(store).await,
        revisions,
        journeys,
        routes,
        proposals,
    }
}

/// Runs two futures at once, polling each in turn until both finish: two commits in flight
/// together, on any executor.
pub async fn both<A: Future, B: Future>(first: A, second: B) -> (A::Output, B::Output) {
    let mut first = pin!(first);
    let mut second = pin!(second);
    let (mut first_output, mut second_output) = (None, None);
    std::future::poll_fn(|context| {
        if first_output.is_none()
            && let Poll::Ready(output) = first.as_mut().poll(context)
        {
            first_output = Some(output);
        }
        if second_output.is_none()
            && let Poll::Ready(output) = second.as_mut().poll(context)
        {
            second_output = Some(output);
        }
        if first_output.is_some() && second_output.is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    (first_output.unwrap(), second_output.unwrap())
}
