//! The memory backend (ARCHITECTURE, Backends): the reference implementation the
//! conformance suite is written against, used by tests, fixtures, and the in-browser host.
//! Nothing persists. Every call completes at once, so it needs no runtime.
//!
//! A commit stages its writes on a copy of the domains and swaps the copy in only once every
//! check has passed, appending the events and the receipt in the same step: one critical
//! section, so the commit is all or nothing (A17, J2).

mod domains;
mod queries;
mod records;

use std::future::ready;
use std::sync::{Mutex, MutexGuard, PoisonError};

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    ConversationId, Domain, Email, Entity, EntityKey, JourneyId, PatchId, PatchReceipt, Proposal,
    ProposalId, Revision, RevisionConflict, RevisionOf, Slug, Timestamp, Title, TouchedSet, UserId,
};

use crate::backend::{self, Shape, StoredReceipt};
use crate::commit::{Commit, CommitError, Committed, Precondition, StoreError};
use crate::faults::{CommitPoint, Faults};
use crate::query::PageSize;
use crate::query::{
    EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoggedEvent, Page, Revisions,
    RouteDetail, SearchQuery,
};
use crate::records::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationRecord, ConversationSummary,
    IdentityRecord, LoggedAuthEntry, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};
use crate::store::{AuthStore, ConversationStore, Store, StoreFuture};
use crate::target::{Document, LoadTarget};

use domains::Domains;

#[derive(Debug, Default)]
struct State {
    domains: Domains,
    receipts: BTreeMap<PatchId, StoredReceipt>,
    events: Vec<LoggedEvent>,
    /// The nodes each logged event is about or wrote on, by position (J5).
    event_nodes: BTreeMap<u64, BTreeSet<cairn_schema::NodeKey>>,
    next_seq: u64,
    outside: records::Outside,
}

/// The in-memory store.
#[derive(Debug, Default)]
pub struct MemoryStore {
    state: Mutex<State>,
    faults: Faults,
}

impl MemoryStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty store with faults that tests can arm.
    #[must_use]
    pub fn with_faults(faults: Faults) -> Self {
        Self {
            state: Mutex::default(),
            faults,
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock happens before a commit swaps anything in, so the
        // state is whole.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn load_now(&self, target: &LoadTarget) -> Option<Document> {
        let state = self.state();
        let domains = &state.domains;
        match target {
            LoadTarget::Journey(id) => domains.journey(id).map(Document::Journey),
            LoadTarget::Route(id) => domains.route(id).map(Document::Route),
            LoadTarget::RouteVersion { route, version } => domains
                .route_version(route, *version)
                .map(Document::RouteVersion),
            LoadTarget::Deployment => Some(Document::Deployment(domains.deployment.clone())),
        }
    }

    fn commit_now(&self, commit: &Commit) -> Result<Committed, CommitError> {
        let shape = Shape::of(commit)?;
        let receipt = &commit.change_set.receipt;
        let mut state = self.state();
        if let Some(stored) = state.receipts.get(&receipt.patch_id) {
            return backend::resubmission(&stored.receipt, receipt);
        }
        state.check_revisions(commit, &shape)?;
        let mut violations = Vec::new();
        if let (Domain::Journey(journey), true) =
            (&shape.domain, commit.base_revision == Revision::NONE)
            && state.domains.deleted_journeys.contains_key(journey)
        {
            violations.push(backend::deleted_journey(journey));
        }
        violations.extend(backend::entity_create_violations(
            &shape,
            &state.domains.deployment,
        )?);
        let nodes: Vec<_> = commit
            .change_set
            .events
            .iter()
            .map(|event| state.domains.event_nodes(event))
            .collect();
        let taken_proposals = state.domains.proposals_of_deleted(&shape);
        let mut candidate = state.domains.clone();
        for write in commit.change_set.writes() {
            candidate.apply(write)?;
        }
        violations.extend(candidate.violations(&shape)?);
        backend::reject_violations(violations)?;
        candidate.advance(&shape)?;
        if self.faults.take_failure(CommitPoint::BetweenStateAndEvents) {
            return Err(injected(CommitPoint::BetweenStateAndEvents));
        }
        let deployment = candidate.deployment.revision;
        state.domains = candidate;
        if let Some(journey) = &shape.deletes_journey {
            let log = Domain::Journey(journey.clone());
            let State {
                events,
                event_nodes,
                ..
            } = &mut *state;
            events.retain(|logged| {
                let kept = logged.event.log != log;
                if !kept {
                    event_nodes.remove(&logged.seq);
                }
                kept
            });
        }
        for (event, nodes) in commit.change_set.events.iter().zip(nodes) {
            let seq = state.next_seq;
            state.next_seq += 1;
            state.event_nodes.insert(seq, nodes);
            state.events.push(LoggedEvent {
                seq,
                event: event.clone(),
            });
        }
        let mut stored = StoredReceipt::new(&commit.change_set, &shape, deployment);
        stored
            .proposals
            .extend(taken_proposals.into_iter().map(|id| (id, Revision::NONE)));
        state.receipts.insert(receipt.patch_id.clone(), stored);
        Ok(Committed::Applied(receipt.clone()))
    }

    fn revisions_now(&self) -> Revisions {
        let state = self.state();
        let domains = &state.domains;
        Revisions {
            deployment: domains.deployment.revision,
            journeys: domains
                .journeys
                .iter()
                .map(|(id, (_, revision))| (id.clone(), *revision))
                .collect(),
            routes: domains
                .routes
                .iter()
                .map(|(id, (_, revision))| (id.clone(), *revision))
                .collect(),
            proposals: domains
                .proposals
                .values()
                .map(|proposal| {
                    (
                        proposal.id.clone(),
                        (proposal.destination.clone(), proposal.revision),
                    )
                })
                .collect(),
        }
    }
}

fn injected(point: CommitPoint) -> CommitError {
    CommitError::Failed(StoreError::Backend(format!("injected fault at {point:?}")))
}

impl State {
    /// H5: every revision the commit names that has moved, with the touched set of the
    /// events that moved them.
    fn check_revisions(&self, commit: &Commit, shape: &Shape) -> Result<(), CommitError> {
        let mut conflicts = Vec::new();
        for (of, expected) in backend::expected_revisions(commit, shape) {
            let current = self.domains.revision(&of);
            if current != expected {
                conflicts.push(RevisionConflict {
                    of,
                    expected,
                    current,
                });
            }
        }
        for precondition in &commit.preconditions {
            if let Precondition::ReferencingJourneys { entities, journeys } = precondition {
                for (journey, current) in self.domains.journeys_referencing(entities) {
                    if !journeys.contains(&journey) {
                        conflicts.push(RevisionConflict {
                            of: RevisionOf::Domain(Domain::Journey(journey)),
                            expected: Revision::NONE,
                            current,
                        });
                    }
                }
            }
        }
        if conflicts.is_empty() {
            return Ok(());
        }
        let intervening = self.intervening(&conflicts);
        Err(backend::stale(conflicts, intervening))
    }

    fn intervening(&self, conflicts: &[RevisionConflict]) -> TouchedSet {
        let mut touched = TouchedSet::default();
        for stored in self.receipts.values() {
            let moved = conflicts
                .iter()
                .any(|conflict| stored.moved_past(&conflict.of, conflict.expected));
            if !moved {
                continue;
            }
            for logged in &self.events {
                if logged.event.patch_id == stored.receipt.patch_id {
                    touched.extend(logged.event.touched());
                }
            }
            touched.extend(stored.deployment_touched.clone());
        }
        touched
    }
}

impl Domains {
    /// The checks only the commit can make, on the records it produced.
    fn violations(&self, shape: &Shape) -> Result<Vec<cairn_schema::Violation>, StoreError> {
        let mut violations = Vec::new();
        for graph in &shape.graphs {
            let content = self.graph(graph);
            if content != cairn_schema::Graph::default() && !self.owns(graph) {
                return Err(StoreError::Malformed(format!(
                    "{graph:?} has content but no journey, draft, or version holds it"
                )));
            }
            violations.extend(backend::graph_violations(graph, &content));
        }
        if shape.writes_deployment {
            violations.extend(backend::deployment_violations(&self.deployment));
            violations.extend(backend::deployment_size_violation(&self.deployment));
        }
        Ok(violations)
    }

    fn owns(&self, graph: &cairn_schema::GraphId) -> bool {
        use cairn_schema::GraphId;
        match graph {
            GraphId::Journey(journey) => self.journeys.contains_key(journey),
            GraphId::RouteDraft(route) => self.drafts.contains_key(route),
            GraphId::RouteVersion { route, version } => {
                self.versions.contains_key(&(route.clone(), *version))
            }
        }
    }

    /// Moves the revisions the commit advances: its domain's or proposal's, and the
    /// deployment's when entity creates rode in (E6).
    fn advance(&mut self, shape: &Shape) -> Result<(), StoreError> {
        let deleted = shape.deletes_journey.is_some();
        match &shape.advances {
            RevisionOf::Domain(Domain::Journey(id)) => match self.journeys.get_mut(id) {
                Some(row) => row.1 = shape.revision,
                None if deleted => {}
                None => return missing_header(&shape.domain),
            },
            RevisionOf::Domain(Domain::Route(id)) => match self.routes.get_mut(id) {
                Some(row) => row.1 = shape.revision,
                None => return missing_header(&shape.domain),
            },
            RevisionOf::Domain(Domain::Deployment) => self.deployment.revision = shape.revision,
            RevisionOf::Proposal(id) => {
                let revision = self.proposals.get(id).map(|proposal| proposal.revision);
                if revision != Some(shape.revision) {
                    return Err(StoreError::Malformed(format!(
                        "a patch to proposal {id} leaves it at {revision:?}, not {}",
                        shape.revision
                    )));
                }
            }
        }
        if shape.bumps_deployment {
            self.deployment.revision = self.deployment.revision.next();
        }
        Ok(())
    }
}

fn missing_header(domain: &Domain) -> Result<(), StoreError> {
    Err(StoreError::Malformed(format!(
        "{domain} has no fields after its commit: its first commit writes them"
    )))
}

/// A call that has already completed: the memory backend answers at once.
fn done<'a, T: Send + 'a>(value: T) -> StoreFuture<'a, T> {
    Box::pin(ready(value))
}

impl Store for MemoryStore {
    fn load<'a>(
        &'a self,
        target: &'a LoadTarget,
    ) -> StoreFuture<'a, Result<Option<Document>, StoreError>> {
        done(Ok(self.load_now(target)))
    }

    fn proposal<'a>(
        &'a self,
        id: &'a ProposalId,
    ) -> StoreFuture<'a, Result<Option<Proposal>, StoreError>> {
        done(Ok(self.state().domains.proposals.get(id).cloned()))
    }

    fn health(&self) -> Result<(), StoreError> {
        if self.faults.failed_closed() {
            return Err(StoreError::Backend(
                "the store failed closed (an armed fault)".to_owned(),
            ));
        }
        Ok(())
    }

    fn receipt<'a>(
        &'a self,
        patch: &'a PatchId,
    ) -> StoreFuture<'a, Result<Option<PatchReceipt>, StoreError>> {
        let receipt = self
            .state()
            .receipts
            .get(patch)
            .map(|stored| stored.receipt.clone());
        done(Ok(receipt))
    }

    fn commit(&self, commit: Commit) -> StoreFuture<'_, Result<Committed, CommitError>> {
        let wait = self.faults.pause_at(CommitPoint::BeforeBegin);
        Box::pin(async move {
            if let Some(wait) = wait {
                wait.await;
            }
            self.commit_now(&commit)
        })
    }

    fn revisions(&self) -> StoreFuture<'_, Result<Revisions, StoreError>> {
        done(Ok(self.revisions_now()))
    }

    fn intervening<'a>(
        &'a self,
        conflicts: &'a [RevisionConflict],
    ) -> StoreFuture<'a, Result<TouchedSet, StoreError>> {
        done(Ok(self.state().intervening(conflicts)))
    }

    fn journeys<'a>(
        &'a self,
        query: &'a JourneyQuery,
    ) -> StoreFuture<'a, Result<Page<JourneySummary, JourneyId>, StoreError>> {
        done(Ok(self.state().journeys(query)))
    }

    fn proposals<'a>(
        &'a self,
        query: &'a crate::query::ProposalQuery,
    ) -> StoreFuture<'a, Result<crate::query::ProposalPage, StoreError>> {
        done(Ok(self.state().proposals(query)))
    }

    fn route_detail<'a>(
        &'a self,
        route: &'a cairn_schema::RouteId,
    ) -> StoreFuture<'a, Result<Option<RouteDetail>, StoreError>> {
        done(Ok(self.state().route_detail(route)))
    }

    fn journeys_referencing<'a>(
        &'a self,
        entities: &'a BTreeSet<EntityKey>,
    ) -> StoreFuture<'a, Result<BTreeMap<JourneyId, Revision>, StoreError>> {
        done(Ok(queries::referencing(&self.state(), entities)))
    }

    fn resolve_entity<'a>(
        &'a self,
        key: &'a EntityKey,
    ) -> StoreFuture<'a, Result<Option<Entity>, StoreError>> {
        done(Ok(self.state().resolve_entity(key)))
    }

    fn events<'a>(
        &'a self,
        query: &'a EventQuery,
    ) -> StoreFuture<'a, Result<Page<LoggedEvent, u64>, StoreError>> {
        done(Ok(self.state().events(query)))
    }

    fn search<'a>(
        &'a self,
        query: &'a SearchQuery,
    ) -> StoreFuture<'a, Result<Page<JourneyMatches, JourneyId>, StoreError>> {
        done(Ok(self.state().search(query)))
    }
}

impl AuthStore for MemoryStore {
    fn put_user(&self, user: UserRecord) -> StoreFuture<'_, Result<(), StoreError>> {
        self.state().outside.put_user(user);
        done(Ok(()))
    }

    fn user<'a>(
        &'a self,
        id: &'a UserId,
    ) -> StoreFuture<'a, Result<Option<UserRecord>, StoreError>> {
        done(Ok(self.state().outside.user(id)))
    }

    fn put_identity(&self, identity: IdentityRecord) -> StoreFuture<'_, Result<(), StoreError>> {
        done(self.state().outside.put_identity(identity))
    }

    fn identity<'a>(
        &'a self,
        provider: &'a Slug,
        subject: &'a Title,
    ) -> StoreFuture<'a, Result<Option<IdentityRecord>, StoreError>> {
        done(Ok(self.state().outside.identity(provider, subject)))
    }

    fn identities_of<'a>(
        &'a self,
        user: &'a UserId,
    ) -> StoreFuture<'a, Result<Vec<IdentityRecord>, StoreError>> {
        done(Ok(self.state().outside.identities_of(user)))
    }

    fn identities_with_email<'a>(
        &'a self,
        email: &'a Email,
    ) -> StoreFuture<'a, Result<Vec<IdentityRecord>, StoreError>> {
        done(Ok(self.state().outside.identities_with_email(email)))
    }

    fn remove_identity<'a>(
        &'a self,
        provider: &'a Slug,
        subject: &'a Title,
    ) -> StoreFuture<'a, Result<bool, StoreError>> {
        done(Ok(self.state().outside.remove_identity(provider, subject)))
    }

    fn put_session(&self, session: SessionRecord) -> StoreFuture<'_, Result<(), StoreError>> {
        done(self.state().outside.put_session(session))
    }

    fn session<'a>(
        &'a self,
        token_hash: &'a SecretHash,
        now: Timestamp,
    ) -> StoreFuture<'a, Result<Option<SessionRecord>, StoreError>> {
        done(Ok(self.state().outside.session(token_hash, now)))
    }

    fn remove_session<'a>(
        &'a self,
        token_hash: &'a SecretHash,
    ) -> StoreFuture<'a, Result<bool, StoreError>> {
        done(Ok(self.state().outside.remove_session(token_hash)))
    }

    fn put_oauth_state(&self, state: OAuthStateRecord) -> StoreFuture<'_, Result<(), StoreError>> {
        self.state().outside.put_oauth_state(state);
        done(Ok(()))
    }

    fn take_oauth_state<'a>(
        &'a self,
        key_hash: &'a SecretHash,
        now: Timestamp,
    ) -> StoreFuture<'a, Result<Option<OAuthStateRecord>, StoreError>> {
        done(Ok(self.state().outside.take_oauth_state(key_hash, now)))
    }

    fn remove_expired(&self, now: Timestamp) -> StoreFuture<'_, Result<u64, StoreError>> {
        done(Ok(self.state().outside.remove_expired(now)))
    }

    fn put_agent_token(&self, token: AgentTokenRecord) -> StoreFuture<'_, Result<(), StoreError>> {
        done(self.state().outside.put_agent_token(token))
    }

    fn agent_token<'a>(
        &'a self,
        token_hash: &'a SecretHash,
    ) -> StoreFuture<'a, Result<Option<AgentTokenRecord>, StoreError>> {
        done(Ok(self.state().outside.agent_token(token_hash)))
    }

    fn agent_tokens_of<'a>(
        &'a self,
        user: &'a UserId,
    ) -> StoreFuture<'a, Result<Vec<AgentTokenRecord>, StoreError>> {
        done(Ok(self.state().outside.agent_tokens_of(user)))
    }

    fn append_auth_log(&self, entry: AuthLogEntry) -> StoreFuture<'_, Result<u64, StoreError>> {
        done(Ok(self.state().outside.append_auth_log(entry)))
    }

    fn auth_log<'a>(
        &'a self,
        query: &'a AuthLogQuery,
    ) -> StoreFuture<'a, Result<Page<LoggedAuthEntry, u64>, StoreError>> {
        done(Ok(self.state().outside.auth_log(query)))
    }
}

impl ConversationStore for MemoryStore {
    fn put_conversation(
        &self,
        conversation: ConversationRecord,
    ) -> StoreFuture<'_, Result<(), StoreError>> {
        done(self.state().outside.put_conversation(conversation))
    }

    fn conversation<'a>(
        &'a self,
        id: &'a ConversationId,
    ) -> StoreFuture<'a, Result<Option<ConversationRecord>, StoreError>> {
        done(Ok(self.state().outside.conversation(id)))
    }

    fn conversations_of<'a>(
        &'a self,
        user: &'a UserId,
        after: Option<&'a ConversationId>,
        size: PageSize,
    ) -> StoreFuture<'a, Result<Page<ConversationSummary, ConversationId>, StoreError>> {
        done(Ok(self.state().outside.conversations_of(user, after, size)))
    }

    fn remove_conversation<'a>(
        &'a self,
        id: &'a ConversationId,
    ) -> StoreFuture<'a, Result<bool, StoreError>> {
        done(Ok(self.state().outside.remove_conversation(id)))
    }
}
