//! The [`Store`] trait (ARCHITECTURE, Store trait): small and whole-domain oriented; nothing
//! in it walks the graph.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;

use cairn_schema::{
    ConversationId, Entity, EntityKey, JourneyId, PatchId, PatchReceipt, Proposal, ProposalId,
    Revision, RouteId, Slug, Timestamp, Title, UserId,
};

use crate::commit::{Commit, CommitError, Committed, StoreError};
use crate::query::{
    EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoggedEvent, Page, Revisions,
    RouteDetail, SearchQuery,
};
use crate::records::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, ConversationRecord, ConversationSummary,
    IdentityRecord, LoggedAuthEntry, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};
use crate::target::{Document, LoadTarget};

/// Where domains, proposals, events, and the records outside any domain live. Async, naming
/// no runtime; every future is `Send` so a multi-threaded host can hold one across awaits.
pub trait Store: AuthStore + ConversationStore + Send + Sync {
    /// One read of one graph by typed target: a journey, a route with its draft, a published
    /// version, or the deployment. `None` when it does not exist; the deployment always
    /// does. Derive needs no events, so none are loaded.
    fn load(
        &self,
        target: &LoadTarget,
    ) -> impl Future<Output = Result<Option<Document>, StoreError>> + Send;

    /// A proposal by id (I6), with its destination and editing revision.
    fn proposal(
        &self,
        id: &ProposalId,
    ) -> impl Future<Output = Result<Option<Proposal>, StoreError>> + Send;

    /// H5: the receipt of a committed patch id, which a resubmission is answered from
    /// before anything is loaded or applied.
    fn receipt(
        &self,
        patch: &PatchId,
    ) -> impl Future<Output = Result<Option<PatchReceipt>, StoreError>> + Send;

    /// A17, J2: persists a change set in one transaction, or nothing. In order: recheck the
    /// receipt (the same patch id with the same content is answered from it; with other
    /// content it is rejected), check the target's base revision and every precondition
    /// (all that moved are listed, with the touched set of the intervening events), reject a
    /// create at a hard-deleted journey's id (A19), write the change set's records, check
    /// what only the commit can (an entity create riding in a journey or route patch takes a
    /// key that is neither an entity nor an alias, E6; emails stay unique, H3; every graph
    /// written stays within `graph_bytes_max`), append the events, store the receipt, and
    /// bump the revision. A domain that does not exist is at revision 0, and its first
    /// commit takes base revision 0 and produces revision 1 (A17). Entity creates riding in
    /// another domain's patch bump the deployment revision too (E6). A patch to a proposal
    /// bumps the proposal's revision, not its destination's (H5).
    fn commit(&self, commit: Commit)
    -> impl Future<Output = Result<Committed, CommitError>> + Send;

    /// The current revisions of every domain and proposal, for a new subscriber (H6).
    fn revisions(&self) -> impl Future<Output = Result<Revisions, StoreError>> + Send;

    /// C16: the journey index, by id, filtered and paged.
    fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> impl Future<Output = Result<Page<JourneySummary, JourneyId>, StoreError>> + Send;

    /// C17: a route's published versions with the journeys on each.
    fn route_detail(
        &self,
        route: &RouteId,
    ) -> impl Future<Output = Result<Option<RouteDetail>, StoreError>> + Send;

    /// E6: the journeys referring to any of `entities`, directly or through an alias, at
    /// their current revisions: what an entity merge is checked against.
    fn journeys_referencing(
        &self,
        entities: &BTreeSet<EntityKey>,
    ) -> impl Future<Output = Result<BTreeMap<JourneyId, Revision>, StoreError>> + Send;

    /// E6: the entity a key names, resolving a merged entity's old key through its alias.
    fn resolve_entity(
        &self,
        key: &EntityKey,
    ) -> impl Future<Output = Result<Option<Entity>, StoreError>> + Send;

    /// J5: events by journey, node, user, type, patch, and time, in commit order, paged.
    fn events(
        &self,
        query: &EventQuery,
    ) -> impl Future<Output = Result<Page<LoggedEvent, u64>, StoreError>> + Send;

    /// Text search across journeys (the MCP `search` tool and the journey index), by
    /// journey id, paged.
    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = Result<Page<JourneyMatches, JourneyId>, StoreError>> + Send;
}

/// Auth state outside every domain (ARCHITECTURE, Auth; Schema outline): plain record
/// operations the auth crate builds on (3.2). An identity, session, or agent token names a
/// user that exists; a record that does not is a malformed write.
pub trait AuthStore: Send + Sync {
    /// Stores a user, replacing any with its id.
    fn put_user(&self, user: UserRecord) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// A user by id.
    fn user(
        &self,
        id: &UserId,
    ) -> impl Future<Output = Result<Option<UserRecord>, StoreError>> + Send;

    /// Stores an identity, replacing any with its provider and subject.
    fn put_identity(
        &self,
        identity: IdentityRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// An identity by provider and subject: who a sign-in is.
    fn identity(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> impl Future<Output = Result<Option<IdentityRecord>, StoreError>> + Send;

    /// A user's identities, by provider and subject.
    fn identities_of(
        &self,
        user: &UserId,
    ) -> impl Future<Output = Result<Vec<IdentityRecord>, StoreError>> + Send;

    /// Removes an identity; whether there was one.
    fn remove_identity(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Stores a session, replacing any with its token digest.
    fn put_session(
        &self,
        session: SessionRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// A session by its token digest, if it has not expired at `now`.
    fn session(
        &self,
        token_hash: &SecretHash,
        now: Timestamp,
    ) -> impl Future<Output = Result<Option<SessionRecord>, StoreError>> + Send;

    /// Ends a session; whether there was one.
    fn remove_session(
        &self,
        token_hash: &SecretHash,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Stores transient OAuth state, replacing any with its digest.
    fn put_oauth_state(
        &self,
        state: OAuthStateRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Takes transient OAuth state by its digest: it is removed, so it is used once, and
    /// none is returned once it has expired at `now`.
    fn take_oauth_state(
        &self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> impl Future<Output = Result<Option<OAuthStateRecord>, StoreError>> + Send;

    /// Removes every session and piece of OAuth state expired at `now`; how many.
    fn remove_expired(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<u64, StoreError>> + Send;

    /// Stores an agent token, replacing any with its agent id (a revocation is a put with
    /// `revoked_at`).
    fn put_agent_token(
        &self,
        token: AgentTokenRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// An agent token by its digest: how a request carrying one is authenticated.
    fn agent_token(
        &self,
        token_hash: &SecretHash,
    ) -> impl Future<Output = Result<Option<AgentTokenRecord>, StoreError>> + Send;

    /// A user's agent tokens, by agent id.
    fn agent_tokens_of(
        &self,
        user: &UserId,
    ) -> impl Future<Output = Result<Vec<AgentTokenRecord>, StoreError>> + Send;

    /// Appends to the auth log; the entry's position.
    fn append_auth_log(
        &self,
        entry: AuthLogEntry,
    ) -> impl Future<Output = Result<u64, StoreError>> + Send;

    /// The auth log, in append order, filtered and paged.
    fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> impl Future<Output = Result<Page<LoggedAuthEntry, u64>, StoreError>> + Send;
}

/// Assistant conversations, outside every domain (ARCHITECTURE, Store trait), stored whole
/// within `graph_bytes_max`.
pub trait ConversationStore: Send + Sync {
    /// Stores a conversation, replacing any with its id.
    fn put_conversation(
        &self,
        conversation: ConversationRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// A conversation by id.
    fn conversation(
        &self,
        id: &ConversationId,
    ) -> impl Future<Output = Result<Option<ConversationRecord>, StoreError>> + Send;

    /// A user's conversations, by id, paged.
    fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: crate::query::PageSize,
    ) -> impl Future<Output = Result<Page<ConversationSummary, ConversationId>, StoreError>> + Send;

    /// Removes a conversation; whether there was one.
    fn remove_conversation(
        &self,
        id: &ConversationId,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;
}
