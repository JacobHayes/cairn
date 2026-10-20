//! The [`Store`] trait (ARCHITECTURE, Store trait): small and whole-domain oriented; nothing
//! in it walks the graph.

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use cairn_schema::{
    ConversationId, Email, Identity, IdentityKey, JourneyId, PatchId, PatchReceipt, Proposal,
    ProposalId, Revision, RevisionConflict, RouteId, Slug, Timestamp, Title, TouchedSet, UserId,
};

use crate::commit::{Commit, CommitError, Committed, StoreError};
use crate::query::{
    EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoggedEvent, Page, ProposalPage,
    ProposalQuery, Revisions, RouteDetail, SearchQuery,
};
use crate::records::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationRecord, ConversationSummary,
    LoggedAuthEntry, LoginRecord, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};
use crate::target::{Document, LoadTarget};

/// Where domains, proposals, events, and the records outside any domain live. Async, naming
/// no runtime; the traits use `async_trait`, so every future is boxed and `Send` (a
/// multi-threaded host can hold one across awaits) and the trait is object-safe: the server
/// stack holds the store as `Arc<dyn Store>`.
///
/// Nothing is answered from a write that may not have reached the store's storage: not a
/// read, not a receipt, not a commit. A backend that cannot tell whether a failed commit
/// reached it settles that before it answers anything, or errs on every call until it is
/// reopened
/// (decisions/2026-10-07-the-log-sync-fix-nothing-is-answered-from-a-write-until.md).
#[async_trait]
pub trait Store: AuthStore + ConversationStore + Send + Sync {
    /// One read of one graph by typed target: a journey, a route with its draft, a published
    /// version, or the deployment. `None` when it does not exist; the deployment always
    /// does. Derive needs no events, so none are loaded.
    async fn load(&self, target: &LoadTarget) -> Result<Option<Document>, StoreError>;

    /// A proposal by id (I6), with its destination and editing revision.
    async fn proposal(&self, id: &ProposalId) -> Result<Option<Proposal>, StoreError>;

    /// C14, C16: the open proposals of a scope, newest first, with the scope's open count.
    async fn proposals(&self, query: &ProposalQuery) -> Result<ProposalPage, StoreError>;

    /// H5: the receipt of a committed patch id, which a resubmission is answered from
    /// before anything is loaded or applied.
    async fn receipt(&self, patch: &PatchId) -> Result<Option<PatchReceipt>, StoreError>;

    /// A17, J2: persists a change set in one transaction, or nothing. In order: recheck the
    /// receipt (the same patch id with the same content is answered from it; with other
    /// content it is rejected), check the target's base revision and every precondition
    /// (all that moved are listed, with the touched set of the intervening events), reject a
    /// create at a hard-deleted journey's id (A19), write the change set's records, check
    /// what only the commit can (an identity create riding in a journey or route patch takes a
    /// key that is neither an identity nor an alias, E6; emails stay unique, H3; every graph
    /// written stays within `graph_bytes_max`), append the events, store the receipt, and
    /// bump the revision. A domain that does not exist is at revision 0, and its first
    /// commit takes base revision 0 and produces revision 1 (A17). Identity creates riding in
    /// another domain's patch bump the deployment revision too (E6). A patch to a proposal
    /// bumps the proposal's revision, not its destination's (H5).
    async fn commit(&self, commit: Commit) -> Result<Committed, CommitError>;

    /// Whether the store still answers: an error once it has failed closed, when it answers
    /// nothing until it is reopened
    /// (decisions/2026-10-07-the-log-sync-fix-nothing-is-answered-from-a-write-until.md).
    /// It reads the store's own state and touches no storage, so a health check costs
    /// nothing.
    ///
    /// # Errors
    ///
    /// Once the store has failed closed.
    fn health(&self) -> Result<(), StoreError>;

    /// The current revisions of every domain and proposal, for a new subscriber (H6).
    async fn revisions(&self) -> Result<Revisions, StoreError>;

    /// H5: what the events of every commit that moved a revision past what `conflicts`
    /// expected touched, as [`commit`](Store::commit) reports in a stale rejection. The
    /// service completes the engine's own stale rejection (it sees no history) with it.
    async fn intervening(&self, conflicts: &[RevisionConflict]) -> Result<TouchedSet, StoreError>;

    /// C16: the journey index, by id, filtered and paged.
    async fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> Result<Page<JourneySummary, JourneyId>, StoreError>;

    /// C17: a route's published versions with the journeys on each.
    async fn route_detail(&self, route: &RouteId) -> Result<Option<RouteDetail>, StoreError>;

    /// E6: the journeys referring to any of `identities`, directly or through an alias, at
    /// their current revisions: what an identity merge is checked against.
    async fn journeys_referencing(
        &self,
        identities: &BTreeSet<IdentityKey>,
    ) -> Result<BTreeMap<JourneyId, Revision>, StoreError>;

    /// H3: the identities holding any of `emails`, each email being held by at most one. An
    /// indexed lookup that loads no deployment, so every request can ask it.
    async fn identities_with_emails(
        &self,
        emails: &BTreeSet<Email>,
    ) -> Result<BTreeSet<IdentityKey>, StoreError>;

    /// E6: the identity a key names, resolving a merged identity's old key through its alias.
    async fn resolve_identity(&self, key: &IdentityKey) -> Result<Option<Identity>, StoreError>;

    /// J5: events by journey, node, user, type, patch, and time, in commit order, paged.
    async fn events(&self, query: &EventQuery) -> Result<Page<LoggedEvent, u64>, StoreError>;

    /// Text search across journeys (the MCP `search` tool and the journey index), by
    /// journey id, paged.
    async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Page<JourneyMatches, JourneyId>, StoreError>;
}

/// Auth state outside every domain (ARCHITECTURE, Auth; Schema outline): plain record
/// operations the auth crate builds on (3.2). A login, session, or agent token names a
/// user that exists; a record that does not is a malformed write.
#[async_trait]
pub trait AuthStore: Send + Sync {
    /// Stores a user, replacing any with its id.
    async fn put_user(&self, user: UserRecord) -> Result<(), StoreError>;

    /// A user by id.
    async fn user(&self, id: &UserId) -> Result<Option<UserRecord>, StoreError>;

    /// Stores a login, replacing any with its provider and subject.
    async fn put_login(&self, login: LoginRecord) -> Result<(), StoreError>;

    /// A login by provider and subject: who a sign-in is.
    async fn login(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> Result<Option<LoginRecord>, StoreError>;

    /// A user's logins, by provider and subject.
    async fn logins_of(&self, user: &UserId) -> Result<Vec<LoginRecord>, StoreError>;

    /// H3: the logins whose provider marked `email` verified, by provider and subject:
    /// what auto-linking a new login on a matching verified email looks for (3.2).
    async fn logins_with_email(&self, email: &Email) -> Result<Vec<LoginRecord>, StoreError>;

    /// Stores a session, replacing any with its token digest.
    async fn put_session(&self, session: SessionRecord) -> Result<(), StoreError>;

    /// A session by its token digest, if it has not expired at `now`.
    async fn session(
        &self,
        token_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<SessionRecord>, StoreError>;

    /// Ends a session; whether there was one.
    async fn remove_session(&self, token_hash: &SecretHash) -> Result<bool, StoreError>;

    /// Stores transient OAuth state, replacing any with its digest.
    async fn put_oauth_state(&self, state: OAuthStateRecord) -> Result<(), StoreError>;

    /// Takes transient OAuth state by its digest: it is removed, so it is used once, and
    /// none is returned once it has expired at `now`.
    async fn take_oauth_state(
        &self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<OAuthStateRecord>, StoreError>;

    /// Removes every session and piece of OAuth state expired at `now`; how many.
    async fn remove_expired(&self, now: Timestamp) -> Result<u64, StoreError>;

    /// Stores an agent token, replacing any with its agent id (a revocation is a put with
    /// `revoked_at`).
    async fn put_agent_token(&self, token: AgentTokenRecord) -> Result<(), StoreError>;

    /// An agent token by its digest: how a request carrying one is authenticated.
    async fn agent_token(
        &self,
        token_hash: &SecretHash,
    ) -> Result<Option<AgentTokenRecord>, StoreError>;

    /// A user's agent tokens, by agent id.
    async fn agent_tokens_of(&self, user: &UserId) -> Result<Vec<AgentTokenRecord>, StoreError>;

    /// Appends to the auth log; the entry's position.
    async fn append_auth_log(&self, entry: AuthLogEntry) -> Result<u64, StoreError>;

    /// The auth log, in append order, filtered and paged.
    async fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> Result<Page<LoggedAuthEntry, u64>, StoreError>;
}

/// Assistant conversations, outside every domain (ARCHITECTURE, Store trait), stored whole
/// within `graph_bytes_max`.
#[async_trait]
pub trait ConversationStore: Send + Sync {
    /// Stores a conversation, replacing any with its id.
    async fn put_conversation(&self, conversation: ConversationRecord) -> Result<(), StoreError>;

    /// A conversation by id.
    async fn conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationRecord>, StoreError>;

    /// A user's conversations, by id, paged.
    async fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: crate::query::PageSize,
    ) -> Result<Page<ConversationSummary, ConversationId>, StoreError>;

    /// Removes a conversation; whether there was one.
    async fn remove_conversation(&self, id: &ConversationId) -> Result<bool, StoreError>;
}
