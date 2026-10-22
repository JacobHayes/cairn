//! The SQLite store backend (ARCHITECTURE, Backends): SQLite, embedded, in WAL mode, with
//! one writer, so commits run one at a time and no read waits on one.
//!
//! The schema is numbered SQL migrations embedded in the binary and applied in order
//! inside one transaction when the store opens. Reads come from a small pool of our own
//! within the limits, and run in a read transaction, so a load sees one commit's state
//! whole. Each call waits for its connection on the runtime, then runs its whole
//! transaction on tokio's blocking pool, so a slow disk stalls that call and never a
//! runtime thread; every module below this one is plain synchronous code over a
//! connection. The backend runs on tokio: the waits for a connection use tokio's timer, so
//! the runtime needs its time driver.

mod commit;
mod conditions;
mod durable;
mod graph;
mod load;
mod migrate;
mod pool;
mod queries;
mod records;
mod sql;
mod state;
mod stored_json;
mod to_deleted_ids;
mod to_done;
mod to_reason;
mod write;
mod write_state;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use cairn_schema::{
    ConversationId, Email, GraphId, Identity, IdentityKey, JourneyId, PatchId, PatchReceipt,
    Proposal, ProposalId, Rejection, RevisionConflict, RouteId, Slug, Timestamp, Title, TouchedSet,
    UserId,
};
use cairn_store::{
    AgentTokenRecord, AuthLogEntry, AuthLogQuery, AuthStore, Commit, CommitError, Committed,
    ConversationRecord, ConversationStore, ConversationSummary, Document, EventQuery, Faults,
    JourneyMatches, JourneyQuery, JourneySummary, LoadTarget, LoggedAuthEntry, LoggedEvent,
    LoginRecord, OAuthStateRecord, Page, PageSize, ProposalPage, ProposalQuery, Revisions,
    RouteDetail, SearchQuery, SecretHash, SessionRecord, Store, StoreError, TourMark, TourRecord,
    UserRecord,
};

use pool::Pool;

/// Store commit latency in seconds, by outcome: `committed`, or `refused` (stale, a reused
/// patch id, or a failure) (ARCHITECTURE, Observability).
pub const COMMIT_DURATION: &str = "cairn_store_commit_duration_seconds";
use sql::execute;

/// The SQLite store over one database file.
pub struct SqliteStore {
    pool: Arc<Pool>,
    /// Whether the store still answers: a commit it cannot settle fails it closed.
    durability: Arc<durable::Durability>,
    faults: Faults,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteStore")
            .finish_non_exhaustive()
    }
}

/// Runs `work` on tokio's blocking pool and waits for it. A panic in it panics here; a
/// runtime shutting down answers a store failure.
async fn blocking<T, E>(work: impl FnOnce() -> Result<T, E> + Send + 'static) -> Result<T, E>
where
    T: Send + 'static,
    E: From<StoreError> + Send + 'static,
{
    match tokio::task::spawn_blocking(work).await {
        Ok(result) => result,
        Err(error) if error.is_panic() => std::panic::resume_unwind(error.into_panic()),
        Err(error) => {
            Err(StoreError::Backend(format!("a store call did not finish: {error}")).into())
        }
    }
}

/// Runs `$body` (an expression over `$connection` and the `$owned` copies of the arguments
/// it reads) in a read transaction on a pooled reader, on the blocking pool, so every
/// statement in it sees the same commit. It answers only while the store has not failed
/// closed (`durable`), so nothing it read can be a write that may not be on disk.
macro_rules! read {
    ($store:expr, [$($owned:ident),*] $connection:ident => $body:expr) => {{
        $(let $owned = $owned.clone();)*
        let lease = $store.pool.reader().await?;
        let durability = Arc::clone(&$store.durability);
        blocking(move || {
            let $connection = lease.connection();
            execute($connection, "BEGIN", Vec::new())?;
            let result = $body;
            let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
            match execute($connection, end, Vec::new()) {
                Ok(_) => {
                    lease.release();
                    durability.still_open().and(result)
                }
                // Dropping the lease ends what the connection held.
                Err(error) => result.and(Err(StoreError::from(error))),
            }
        })
        .await
    }};
}

/// Runs `$body` (an expression over `$connection` and the `$owned` copies of the arguments
/// it reads) as one write transaction on the writer, on the blocking pool: the record
/// operations outside every domain, which carry no revision.
macro_rules! in_transaction {
    ($store:expr, [$($owned:ident),*] $connection:ident => $body:expr) => {{
        $(let $owned = $owned.clone();)*
        let writer = $store.pool.writer().await?;
        let durability = Arc::clone(&$store.durability);
        blocking(move || {
            // After the wait for the writer, so a call queued when the store closed writes
            // nothing.
            durability.still_open()?;
            let $connection = &*writer;
            execute($connection, "BEGIN IMMEDIATE", Vec::new())?;
            match $body {
                Ok(value) => {
                    let settled = execute($connection, "COMMIT", Vec::new());
                    commit::ended($connection, &durability, settled)
                        .map(|()| value)
                        .map_err(StoreError::from)
                }
                Err(error) => {
                    commit::rollback($connection);
                    Err(error)
                }
            }
        })
        .await
    }};
}

impl SqliteStore {
    /// Opens (creating if absent) the database at `path`, in WAL mode, and migrates it.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or a migration fails.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::open_with_faults(path, Faults::default())
    }

    /// Opens the store with faults that tests can arm.
    ///
    /// # Errors
    ///
    /// When the file cannot be opened or a migration fails.
    pub fn open_with_faults(path: &Path, faults: Faults) -> Result<Self, StoreError> {
        // Before SQLite reads anything, so a log record a failed sync left off the disk is
        // not read back from the operating system's cache (`durable`).
        #[cfg(target_os = "linux")]
        durable::evict_cached_pages(path)?;
        let writer = pool::connect(path)?;
        migrate::run(&writer)?;
        Ok(Self {
            pool: Pool::new(path, writer),
            durability: Arc::default(),
            faults,
        })
    }

    /// A commit (the trait's), untimed: the trait's `commit` records its duration.
    async fn commit_timed(&self, commit: Commit) -> Result<Committed, CommitError> {
        if let Some(wait) = self.faults.pause_at(cairn_store::CommitPoint::BeforeBegin) {
            wait.await;
        }
        let shape = cairn_store::backend::Shape::of(&commit)?;
        let writer = self.pool.writer().await?;
        let durability = Arc::clone(&self.durability);
        let faults = self.faults.clone();
        let committed = blocking(move || {
            // After the wait for the writer, so a commit queued behind one that closed the
            // store neither writes nor answers from what that one left. The store fails
            // closed only while a call holds the writer, so it cannot close after this
            // commit's own answer.
            durability.still_open()?;
            commit::commit(&writer, &durability, &faults, &commit, &shape)
        })
        .await;
        // H5: a stale commit's intervening edits, read with the writer free. A commit that
        // landed since moved a conflicting revision past what this one expected too, so it
        // intervened as truly as those before.
        match committed {
            Err(CommitError::Rejected(Rejection::Stale {
                conflicts, touched, ..
            })) => {
                let intervening = self.intervening(&conflicts).await?;
                Err(CommitError::Rejected(Rejection::Stale {
                    conflicts,
                    intervening,
                    touched,
                }))
            }
            other => other,
        }
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn load(&self, target: &LoadTarget) -> Result<Option<Document>, StoreError> {
        read!(self, [target] connection => load::document(connection, &target))
    }

    async fn proposal(&self, id: &ProposalId) -> Result<Option<Proposal>, StoreError> {
        read!(self, [id] connection => load::proposal(connection, &id))
    }

    async fn proposals(&self, query: &ProposalQuery) -> Result<ProposalPage, StoreError> {
        read!(self, [query] connection => queries::proposals(connection, &query))
    }

    fn health(&self) -> Result<(), StoreError> {
        self.durability.still_open()
    }

    async fn receipt(&self, patch: &PatchId) -> Result<Option<PatchReceipt>, StoreError> {
        let stored = read!(self, [patch] connection => load::receipt(connection, &patch))?;
        Ok(stored.map(|stored| stored.receipt))
    }

    async fn commit(&self, commit: Commit) -> Result<Committed, CommitError> {
        let started = std::time::Instant::now();
        let result = self.commit_timed(commit).await;
        let outcome = if result.is_ok() {
            "committed"
        } else {
            "refused"
        };
        metrics::histogram!(COMMIT_DURATION, "outcome" => outcome)
            .record(started.elapsed().as_secs_f64());
        result
    }

    async fn revisions(&self) -> Result<Revisions, StoreError> {
        read!(self, [] connection => load::revisions(connection))
    }

    async fn intervening(&self, conflicts: &[RevisionConflict]) -> Result<TouchedSet, StoreError> {
        let conflicts = conflicts.to_vec();
        read!(self, [] connection => commit::intervening(connection, &conflicts))
    }

    async fn journeys(
        &self,
        query: &JourneyQuery,
    ) -> Result<Page<JourneySummary, JourneyId>, StoreError> {
        read!(self, [query] connection => queries::journeys(connection, &query))
    }

    async fn route_detail(&self, route: &RouteId) -> Result<Option<RouteDetail>, StoreError> {
        read!(self, [route] connection => queries::route_detail(connection, &route))
    }

    async fn graphs_using(&self, route: &RouteId) -> Result<BTreeSet<GraphId>, StoreError> {
        read!(self, [route] connection => queries::using(connection, &route))
    }

    async fn graphs_referencing(
        &self,
        identities: &BTreeSet<IdentityKey>,
    ) -> Result<BTreeSet<GraphId>, StoreError> {
        read!(self, [identities] connection => queries::referencing(connection, &identities))
    }

    async fn identities_with_emails(
        &self,
        emails: &BTreeSet<Email>,
    ) -> Result<BTreeSet<IdentityKey>, StoreError> {
        read!(self, [emails] connection => queries::identities_with_emails(connection, &emails))
    }

    async fn resolve_identity(&self, key: &IdentityKey) -> Result<Option<Identity>, StoreError> {
        read!(self, [key] connection => queries::resolve_identity(connection, &key))
    }

    async fn events(&self, query: &EventQuery) -> Result<Page<LoggedEvent, u64>, StoreError> {
        read!(self, [query] connection => queries::events(connection, &query))
    }

    async fn search(
        &self,
        query: &SearchQuery,
    ) -> Result<Page<JourneyMatches, JourneyId>, StoreError> {
        read!(self, [query] connection => queries::search(connection, &query))
    }
}

#[async_trait]
impl AuthStore for SqliteStore {
    async fn put_user(&self, user: UserRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_user(connection, &user))
    }

    async fn user(&self, id: &UserId) -> Result<Option<UserRecord>, StoreError> {
        read!(self, [id] connection => records::user(connection, &id))
    }

    async fn put_login(&self, login: LoginRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_login(connection, &login))
    }

    async fn login(
        &self,
        provider: &Slug,
        subject: &Title,
    ) -> Result<Option<LoginRecord>, StoreError> {
        read!(self, [provider, subject] connection => records::login(connection, &provider, &subject))
    }

    async fn logins_of(&self, user: &UserId) -> Result<Vec<LoginRecord>, StoreError> {
        read!(self, [user] connection => records::logins_of(connection, &user))
    }

    async fn logins_with_email(&self, email: &Email) -> Result<Vec<LoginRecord>, StoreError> {
        read!(self, [email] connection => records::logins_with_email(connection, &email))
    }

    async fn put_session(&self, session: SessionRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_session(connection, &session))
    }

    async fn session(
        &self,
        token_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<SessionRecord>, StoreError> {
        read!(self, [token_hash] connection => records::session(connection, &token_hash, now))
    }

    async fn remove_session(&self, token_hash: &SecretHash) -> Result<bool, StoreError> {
        in_transaction!(self, [token_hash] connection => records::remove_session(connection, &token_hash))
    }

    async fn put_oauth_state(&self, state: OAuthStateRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_oauth_state(connection, &state))
    }

    async fn take_oauth_state(
        &self,
        key_hash: &SecretHash,
        now: Timestamp,
    ) -> Result<Option<OAuthStateRecord>, StoreError> {
        in_transaction!(self, [key_hash] connection => records::take_oauth_state(connection, &key_hash, now))
    }

    async fn remove_expired(&self, now: Timestamp) -> Result<u64, StoreError> {
        in_transaction!(self, [] connection => records::remove_expired(connection, now))
    }

    async fn put_agent_token(&self, token: AgentTokenRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_agent_token(connection, &token))
    }

    async fn agent_token(
        &self,
        token_hash: &SecretHash,
    ) -> Result<Option<AgentTokenRecord>, StoreError> {
        read!(self, [token_hash] connection => records::agent_token_by_hash(connection, &token_hash))
    }

    async fn agent_tokens_of(&self, user: &UserId) -> Result<Vec<AgentTokenRecord>, StoreError> {
        read!(self, [user] connection => records::agent_tokens_of(connection, &user))
    }

    async fn tour(&self, user: &UserId) -> Result<Option<TourRecord>, StoreError> {
        read!(self, [user] connection => records::tour(connection, &user))
    }

    async fn put_tour(
        &self,
        record: TourRecord,
        expect: Option<TourMark>,
    ) -> Result<Result<(), Option<TourRecord>>, StoreError> {
        in_transaction!(self, [] connection => records::put_tour(connection, &record, expect.as_ref()))
    }

    async fn append_auth_log(&self, entry: AuthLogEntry) -> Result<u64, StoreError> {
        in_transaction!(self, [] connection => records::append_auth_log(connection, &entry))
    }

    async fn auth_log(
        &self,
        query: &AuthLogQuery,
    ) -> Result<Page<LoggedAuthEntry, u64>, StoreError> {
        read!(self, [query] connection => records::auth_log(connection, &query))
    }
}

#[async_trait]
impl ConversationStore for SqliteStore {
    async fn put_conversation(&self, conversation: ConversationRecord) -> Result<(), StoreError> {
        in_transaction!(self, [] connection => records::put_conversation(connection, &conversation))
    }

    async fn conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationRecord>, StoreError> {
        read!(self, [id] connection => records::conversation(connection, &id))
    }

    async fn conversations_of(
        &self,
        user: &UserId,
        after: Option<&ConversationId>,
        size: PageSize,
    ) -> Result<Page<ConversationSummary, ConversationId>, StoreError> {
        let after = after.cloned();
        read!(self, [user, after] connection => records::conversations_of(connection, &user, after.as_ref(), size))
    }

    async fn remove_conversation(&self, id: &ConversationId) -> Result<bool, StoreError> {
        in_transaction!(self, [id] connection => records::remove_conversation(connection, &id))
    }
}
