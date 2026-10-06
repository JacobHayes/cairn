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

/// Expands to one `#[test]` per conformance case, each running the case against
/// `$backend` on the executor `$run` (a function from a future to its output).
#[macro_export]
macro_rules! conformance_suite {
    ($backend:expr, $run:path) => {
        $crate::conformance_suite!(@cases $backend, $run;
            commits::a_first_commit_creates_the_domain_and_a_second_from_zero_conflicts,
            commits::a_revision_conflict_is_rejected_and_leaves_state_untouched,
            commits::a_failure_between_state_and_events_leaves_nothing,
            commits::a_commit_fails_whole_when_either_precondition_is_stale,
            commits::two_commits_to_one_journey_yield_one_success_and_one_conflict,
            commits::a_resubmitted_patch_id_answers_from_its_receipt_or_is_rejected,
            commits::an_oversize_commit_names_the_limit,
            commits::a_route_with_many_versions_loads_its_draft_under_the_cap,
            commits::a_journey_round_trips_every_record,
            commits::writes_follow_the_reference_semantics,
            commits::sibling_ids_may_swap_within_a_commit_but_not_end_duplicated,
            commits::emails_may_move_between_entities_within_a_commit_but_stay_unique,
            commits::an_entity_create_in_a_journey_patch_bumps_the_deployment_revision,
            commits::an_entity_create_whose_key_is_an_entity_or_alias_is_rejected,
            commits::a_hard_delete_removes_events_and_keeps_the_deployment_event,
            commits::a_create_at_a_deleted_journey_id_is_rejected,
            commits::a_proposal_revision_is_independent_of_its_destination,
            commits::an_entity_merge_is_stale_when_another_journey_references_its_entities,
            queries::aliases_resolve_to_their_entity,
            queries::current_revisions_list_every_domain_and_proposal,
            queries::the_journey_index_filters_by_status_route_version_reference_and_upgrade,
            queries::the_journey_index_events_and_search_page,
            queries::route_detail_lists_versions_with_their_journeys,
            queries::events_filter_by_journey_node_user_type_patch_and_time,
            queries::text_search_hits_names_nodes_notes_and_resources,
            records::auth_records_round_trip,
            records::identities_are_found_by_verified_email,
            records::oauth_state_is_taken_once_and_expired_records_go,
            records::agent_tokens_are_found_by_digest_and_revoked_by_a_put,
            records::the_auth_log_appends_in_order_and_pages_by_user,
            records::conversations_round_trip_and_list_by_user,
        );
    };
    (@cases $backend:expr, $run:path; $($module:ident :: $case:ident),* $(,)?) => {
        $(
            #[test]
            fn $case() {
                $run($crate::conformance::$module::$case(&$backend));
            }
        )*
    };
}

/// Opens a store with no faults armed.
pub async fn open<B: Backend>(backend: &B) -> B::Store {
    backend.open(Faults::default()).await
}

/// Commits, expecting it to apply.
pub async fn applied<S: Store>(store: &S, commit: Commit) -> PatchReceipt {
    match store.commit(commit).await {
        Ok(Committed::Applied(receipt)) => receipt,
        other => panic!("expected the commit to apply, got {other:?}"),
    }
}

/// Commits, expecting it to fail.
pub async fn failed<S: Store>(store: &S, commit: Commit) -> CommitError {
    match store.commit(commit).await {
        Err(error) => error,
        Ok(committed) => panic!("expected the commit to fail, got {committed:?}"),
    }
}

/// A journey as loaded.
pub async fn journey<S: Store>(store: &S, id: &str) -> Option<Journey> {
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
pub async fn route<S: Store>(store: &S, id: &str) -> Option<Route> {
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
pub async fn deployment<S: Store>(store: &S) -> Deployment {
    match store.load(&LoadTarget::Deployment).await.unwrap() {
        Some(Document::Deployment(deployment)) => deployment,
        other => panic!("the deployment load returned {other:?}"),
    }
}

/// Every event in the log, in order.
pub async fn all_events<S: Store>(store: &S) -> Vec<LoggedEvent> {
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
pub async fn snapshot<S: Store>(store: &S) -> Snapshot {
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
