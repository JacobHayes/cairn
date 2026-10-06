//! The Turso store backend (ARCHITECTURE, Backends): the `turso` crate, embedded, in the
//! SQLite file format, with MVCC so a long write never blocks readers and writes to
//! different domains never queue behind each other.
//!
//! The schema is numbered SQL migrations embedded in the binary and applied in order
//! inside one transaction when the store opens. Connections come from a small pool of our
//! own within the limits. Reads run in a read transaction, so a load sees one commit's
//! state whole. The backend runs on tokio: the pool waits with tokio's timer, so the
//! runtime needs its time driver.

mod commit;
mod graph;
mod load;
mod migrate;
mod pool;
mod queries;
mod records;
mod sequence;
mod sql;
mod state;
mod write;
mod write_state;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cairn_schema::{
    ConversationId, Email, Entity, EntityKey, JourneyId, PatchId, PatchReceipt, Proposal,
    ProposalId, Revision, RevisionConflict, RouteId, Slug, Timestamp, Title, TouchedSet, UserId,
};
use cairn_store::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, AuthStore, Commit, CommitError, Committed,
    ConversationRecord, ConversationStore, ConversationSummary, Document, EventQuery, Faults,
    IdentityRecord, JourneyMatches, JourneyQuery, JourneySummary, LoadTarget, LoggedAuthEntry,
    LoggedEvent, OAuthStateRecord, Page, PageSize, Revisions, RouteDetail, SearchQuery, SecretHash,
    SessionRecord, Store, StoreError, UserRecord,
};

use pool::Pool;
use sql::execute;

/// The Turso store over one database file.
pub struct TursoStore {
    pool: Pool,
    faults: Faults,
    /// Positions in the event log and the auth log, in commit-visible order.
    events: sequence::Sequencer,
    auth_log: sequence::Sequencer,
}

impl std::fmt::Debug for TursoStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TursoStore").finish_non_exhaustive()
    }
}

/// Runs `$body` (an expression over `$connection`) in a read transaction on a pooled
/// connection, so every statement in it sees the same commit.
macro_rules! read {
    ($store:expr, $connection:ident => $body:expr) => {{
        let lease = $store.pool.acquire().await?;
        let $connection = lease.connection();
        execute($connection, "BEGIN", Vec::new()).await?;
        let result = $body.await;
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        if execute($connection, end, Vec::new()).await.is_ok() {
            lease.release();
        }
        result
    }};
}

/// Runs `$body` (an expression over `$connection`) as one write transaction on a pooled
/// connection: the record operations outside every domain, which carry no revision.
macro_rules! in_transaction {
    ($store:expr, $connection:ident => $body:expr) => {{
        let lease = $store.pool.acquire().await?;
        let $connection = lease.connection();
        execute($connection, "BEGIN CONCURRENT", Vec::new()).await?;
        let result = $body.await;
        let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
        let ended = execute($connection, end, Vec::new()).await;
        let result = match (result, ended) {
            (Ok(value), Ok(_)) => Ok(value),
            (Ok(_), Err(error)) => {
                // A failed commit has already ended its transaction; the rollback only
                // makes sure.
                let _ = execute($connection, "ROLLBACK", Vec::new()).await;
                Err(StoreError::from(error))
            }
            (Err(error), _) => Err(error),
        };
        lease.release();
        result
    }};
}

impl TursoStore {
    /// Opens (creating if absent) the database at `path`, in MVCC mode, and migrates it.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or a migration fails.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with_faults(path, Faults::default()).await
    }

    /// Opens the store with faults that tests can arm.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or a migration fails.
    pub async fn open_with_faults(path: &Path, faults: Faults) -> Result<Self, StoreError> {
        let path = path
            .to_str()
            .ok_or_else(|| StoreError::Backend(format!("{} is not UTF-8", path.display())))?;
        let database = turso::Builder::new_local(path)
            .build()
            .await
            .map_err(|error| StoreError::Backend(format!("open {path}: {error}")))?;
        let pool = Pool::new(database);
        let connection = pool.connect().await?;
        migrate::run(&connection).await?;
        let next = |table: &'static str| {
            let connection = &connection;
            async move {
                let select = format!("SELECT coalesce(max(seq), 0) + 1 FROM {table}");
                let row = sql::first(connection, &select, Vec::new()).await?;
                row.map_or(Ok(1), |row| row.int(0))
            }
        };
        let events = sequence::Sequencer::new(next("events").await?);
        let auth_log = sequence::Sequencer::new(next("auth_log").await?);
        Ok(Self {
            pool,
            faults,
            events,
            auth_log,
        })
    }
}

impl Store for TursoStore {
    async fn load(&self, target: &LoadTarget) -> Result<Option<Document>, StoreError> {
        read!(self, connection => load::document(connection, target))
    }

    async fn proposal(&self, id: &ProposalId) -> Result<Option<Proposal>, StoreError> {
        read!(self, connection => load::proposal(connection, id))
    }

    async fn receipt(&self, patch: &PatchId) -> Result<Option<PatchReceipt>, StoreError> {
        let stored = read!(self, connection => load::receipt(connection, patch))?;
        Ok(stored.map(|stored| stored.receipt))
    }

    async fn commit(&self, commit: Commit) -> Result<Committed, CommitError> {
        let lease = self.pool.acquire().await?;
        // Boxed: a commit's state machine is tens of kilobytes, too large to move by value.
        let result = Box::pin(commit::commit(
            lease.connection(),
            &self.faults,
            &self.events,
            &commit,
        ))
        .await;
        // Every path out of a commit ends its transaction.
        lease.release();
        result
    }

    async fn revisions(&self) -> Result<Revisions, StoreError> {
        read!(self, connection => load::revisions(connection))
    }

    async fn intervening(&self, conflicts: &[RevisionConflict]) -> Result<TouchedSet, StoreError> {
        read!(self, connection => commit::intervening(connection, conflicts))
    }

    async fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> Result<Page<JourneySummary, JourneyId>, StoreError> {
        read!(self, connection => queries::journeys(connection, query))
    }

    async fn route_detail(&self, route: &RouteId) -> Result<Option<RouteDetail>, StoreError> {
        read!(self, connection => queries::route_detail(connection, route))
    }

    async fn journeys_referencing(
        &self,
        entities: &BTreeSet<EntityKey>,
    ) -> Result<BTreeMap<JourneyId, Revision>, StoreError> {
        read!(self, connection => queries::referencing(connection, entities))
    }

    async fn resolve_entity(&self, key: &EntityKey) -> Result<Option<Entity>, StoreError> {
        read!(self, connection => queries::resolve_entity(connection, key))
    }

    async fn events(&self, query: &EventQuery) -> Result<Page<LoggedEvent, u64>, StoreError> {
        let horizon = self.events.horizon();
        read!(self, connection => queries::events(connection, query, horizon))
    }

    async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Page<JourneyMatches, JourneyId>, StoreError> {
        read!(self, connection => queries::search(connection, query))
    }
}

impl AuthStore for TursoStore {
    async fn put_user(&self, user: UserRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_user(connection, &user))
    }

    async fn user(&self, id: &UserId) -> Result<Option<UserRecord>, StoreError> {
        read!(self, connection => records::user(connection, id))
    }

    async fn put_identity(&self, identity: IdentityRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_identity(connection, &identity))
    }

    async fn identity(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> Result<Option<IdentityRecord>, StoreError> {
        read!(self, connection => records::identity(connection, provider, subject))
    }

    async fn identities_of(&self, user: &UserId) -> Result<Vec<IdentityRecord>, StoreError> {
        read!(self, connection => records::identities_of(connection, user))
    }

    async fn identities_with_email(
        &self,
        email: &Email,
    ) -> Result<Vec<IdentityRecord>, StoreError> {
        read!(self, connection => records::identities_with_email(connection, email))
    }

    async fn remove_identity(&self, provider: &Slug, subject: &Title) -> Result<bool, StoreError> {
        in_transaction!(self, connection => records::remove_identity(connection, provider, subject))
    }

    async fn put_session(&self, session: SessionRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_session(connection, &session))
    }

    async fn session(
        &self,
        token_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<SessionRecord>, StoreError> {
        read!(self, connection => records::session(connection, token_hash, now))
    }

    async fn remove_session(&self, token_hash: &SecretHash) -> Result<bool, StoreError> {
        in_transaction!(self, connection => records::remove_session(connection, token_hash))
    }

    async fn put_oauth_state(&self, state: OAuthStateRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_oauth_state(connection, &state))
    }

    async fn take_oauth_state(
        &self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<OAuthStateRecord>, StoreError> {
        in_transaction!(self, connection => records::take_oauth_state(connection, key_hash, now))
    }

    async fn remove_expired(&self, now: Timestamp) -> Result<u64, StoreError> {
        in_transaction!(self, connection => records::remove_expired(connection, now))
    }

    async fn put_agent_token(&self, token: AgentTokenRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_agent_token(connection, &token))
    }

    async fn agent_token(
        &self,
        token_hash: &SecretHash,
    ) -> Result<Option<AgentTokenRecord>, StoreError> {
        read!(self, connection => records::agent_token_by_hash(connection, token_hash))
    }

    async fn agent_tokens_of(&self, user: &UserId) -> Result<Vec<AgentTokenRecord>, StoreError> {
        read!(self, connection => records::agent_tokens_of(connection, user))
    }

    async fn append_auth_log(&self, entry: AuthLogEntry) -> Result<u64, StoreError> {
        let position = self.auth_log.reserve(1);
        let seq = position.first();
        let appended =
            in_transaction!(self, connection => records::append_auth_log(connection, &entry, seq));
        drop(position);
        appended
    }

    async fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> Result<Page<LoggedAuthEntry, u64>, StoreError> {
        let horizon = self.auth_log.horizon();
        read!(self, connection => records::auth_log(connection, query, horizon))
    }
}

impl ConversationStore for TursoStore {
    async fn put_conversation(&self, conversation: ConversationRecord) -> Result<(), StoreError> {
        in_transaction!(self, connection => records::put_conversation(connection, &conversation))
    }

    async fn conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationRecord>, StoreError> {
        read!(self, connection => records::conversation(connection, id))
    }

    async fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: PageSize,
    ) -> Result<Page<ConversationSummary, ConversationId>, StoreError> {
        read!(self, connection => records::conversations_of(connection, user, after, size))
    }

    async fn remove_conversation(&self, id: &ConversationId) -> Result<bool, StoreError> {
        in_transaction!(self, connection => records::remove_conversation(connection, id))
    }
}
