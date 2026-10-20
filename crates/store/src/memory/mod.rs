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

use std::sync::{Mutex, MutexGuard, PoisonError};

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use cairn_schema::{
    ConversationId, Domain, Email, GraphId, Identity, IdentityKey, JourneyId, PatchId,
    PatchReceipt, Proposal, ProposalId, Revision, RevisionConflict, RevisionOf, Slug, Timestamp,
    Title, TouchedSet, UserId,
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
    LoggedAuthEntry, LoginRecord, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};
use crate::store::{AuthStore, ConversationStore, Store};
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
        violations.extend(backend::identity_create_violations(
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
            let (identities, allowed) = match precondition {
                Precondition::ReferencingJourneys {
                    identities,
                    journeys,
                } => (identities, Some(journeys)),
                Precondition::Unreferenced { identities } => (identities, None),
                Precondition::Revision { .. } => continue,
            };
            for graph in self.domains.graphs_referencing(identities) {
                let outside = match (&graph, allowed) {
                    (GraphId::Journey(journey), Some(named)) => !named.contains(journey),
                    (_, Some(_)) => false,
                    (_, None) => true,
                };
                if outside {
                    conflicts.push(RevisionConflict {
                        of: referencing_domain(&graph),
                        expected: Revision::NONE,
                        current: self.domains.graph_revision(&graph),
                    });
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
    /// deployment's when identity creates rode in (E6).
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

#[async_trait]
impl Store for MemoryStore {
    async fn load(&self, target: &LoadTarget) -> Result<Option<Document>, StoreError> {
        Ok(self.load_now(target))
    }

    async fn proposal(&self, id: &ProposalId) -> Result<Option<Proposal>, StoreError> {
        Ok(self.state().domains.proposals.get(id).cloned())
    }

    fn health(&self) -> Result<(), StoreError> {
        if self.faults.failed_closed() {
            return Err(StoreError::Backend(
                "the store failed closed (an armed fault)".to_owned(),
            ));
        }
        Ok(())
    }

    async fn receipt(&self, patch: &PatchId) -> Result<Option<PatchReceipt>, StoreError> {
        let receipt = self
            .state()
            .receipts
            .get(patch)
            .map(|stored| stored.receipt.clone());
        Ok(receipt)
    }

    async fn commit(&self, commit: Commit) -> Result<Committed, CommitError> {
        let wait = self.faults.pause_at(CommitPoint::BeforeBegin);
        if let Some(wait) = wait {
            wait.await;
        }
        self.commit_now(&commit)
    }

    async fn revisions(&self) -> Result<Revisions, StoreError> {
        Ok(self.revisions_now())
    }

    async fn intervening(&self, conflicts: &[RevisionConflict]) -> Result<TouchedSet, StoreError> {
        Ok(self.state().intervening(conflicts))
    }

    async fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> Result<Page<JourneySummary, JourneyId>, StoreError> {
        Ok(self.state().journeys(query))
    }

    async fn proposals(
        &self,
        query: &crate::query::ProposalQuery,
    ) -> Result<crate::query::ProposalPage, StoreError> {
        Ok(self.state().proposals(query))
    }

    async fn route_detail(
        &self,
        route: &cairn_schema::RouteId,
    ) -> Result<Option<RouteDetail>, StoreError> {
        Ok(self.state().route_detail(route))
    }

    async fn graphs_referencing(
        &self,
        identities: &BTreeSet<IdentityKey>,
    ) -> Result<BTreeSet<GraphId>, StoreError> {
        Ok(queries::referencing(&self.state(), identities))
    }

    async fn identities_with_emails(
        &self,
        emails: &BTreeSet<Email>,
    ) -> Result<BTreeSet<IdentityKey>, StoreError> {
        Ok(self.state().identities_with_emails(emails))
    }

    async fn resolve_identity(&self, key: &IdentityKey) -> Result<Option<Identity>, StoreError> {
        Ok(self.state().resolve_identity(key))
    }

    async fn events(&self, query: &EventQuery) -> Result<Page<LoggedEvent, u64>, StoreError> {
        Ok(self.state().events(query))
    }

    async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Page<JourneyMatches, JourneyId>, StoreError> {
        Ok(self.state().search(query))
    }
}

#[async_trait]
impl AuthStore for MemoryStore {
    async fn put_user(&self, user: UserRecord) -> Result<(), StoreError> {
        self.state().outside.put_user(user);
        Ok(())
    }

    async fn user(&self, id: &UserId) -> Result<Option<UserRecord>, StoreError> {
        Ok(self.state().outside.user(id))
    }

    async fn put_login(&self, login: LoginRecord) -> Result<(), StoreError> {
        self.state().outside.put_login(login)
    }

    async fn login(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> Result<Option<LoginRecord>, StoreError> {
        Ok(self.state().outside.login(provider, subject))
    }

    async fn logins_of(&self, user: &UserId) -> Result<Vec<LoginRecord>, StoreError> {
        Ok(self.state().outside.logins_of(user))
    }

    async fn logins_with_email(&self, email: &Email) -> Result<Vec<LoginRecord>, StoreError> {
        Ok(self.state().outside.logins_with_email(email))
    }

    async fn put_session(&self, session: SessionRecord) -> Result<(), StoreError> {
        self.state().outside.put_session(session)
    }

    async fn session(
        &self,
        token_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<SessionRecord>, StoreError> {
        Ok(self.state().outside.session(token_hash, now))
    }

    async fn remove_session(&self, token_hash: &SecretHash) -> Result<bool, StoreError> {
        Ok(self.state().outside.remove_session(token_hash))
    }

    async fn put_oauth_state(&self, state: OAuthStateRecord) -> Result<(), StoreError> {
        self.state().outside.put_oauth_state(state);
        Ok(())
    }

    async fn take_oauth_state(
        &self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<OAuthStateRecord>, StoreError> {
        Ok(self.state().outside.take_oauth_state(key_hash, now))
    }

    async fn remove_expired(&self, now: Timestamp) -> Result<u64, StoreError> {
        Ok(self.state().outside.remove_expired(now))
    }

    async fn put_agent_token(&self, token: AgentTokenRecord) -> Result<(), StoreError> {
        self.state().outside.put_agent_token(token)
    }

    async fn agent_token(
        &self,
        token_hash: &SecretHash,
    ) -> Result<Option<AgentTokenRecord>, StoreError> {
        Ok(self.state().outside.agent_token(token_hash))
    }

    async fn agent_tokens_of(&self, user: &UserId) -> Result<Vec<AgentTokenRecord>, StoreError> {
        Ok(self.state().outside.agent_tokens_of(user))
    }

    async fn append_auth_log(&self, entry: AuthLogEntry) -> Result<u64, StoreError> {
        Ok(self.state().outside.append_auth_log(entry))
    }

    async fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> Result<Page<LoggedAuthEntry, u64>, StoreError> {
        Ok(self.state().outside.auth_log(query))
    }
}

#[async_trait]
impl ConversationStore for MemoryStore {
    async fn put_conversation(&self, conversation: ConversationRecord) -> Result<(), StoreError> {
        self.state().outside.put_conversation(conversation)
    }

    async fn conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationRecord>, StoreError> {
        Ok(self.state().outside.conversation(id))
    }

    async fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: PageSize,
    ) -> Result<Page<ConversationSummary, ConversationId>, StoreError> {
        Ok(self.state().outside.conversations_of(user, after, size))
    }

    async fn remove_conversation(&self, id: &ConversationId) -> Result<bool, StoreError> {
        Ok(self.state().outside.remove_conversation(id))
    }
}

/// The domain a graph belongs to, as a revision's address.
fn referencing_domain(graph: &GraphId) -> RevisionOf {
    RevisionOf::Domain(match graph {
        GraphId::Journey(journey) => Domain::Journey(journey.clone()),
        GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. } => {
            Domain::Route(route.clone())
        }
    })
}
