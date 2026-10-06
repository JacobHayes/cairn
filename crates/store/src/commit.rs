//! A commit (ARCHITECTURE, Store trait: `commit`): one change set, the revisions it was
//! checked against, and what came of it.

use std::collections::BTreeSet;
use std::fmt;

use cairn_schema::{
    ChangeSet, EntityKey, JourneyId, PatchReceipt, PatchTarget, Rejection, Revision, RevisionOf,
};

/// What a commit checks besides its target's base revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Precondition {
    /// A revision that must not have moved: the deployment revision a journey patch that
    /// writes an entity reference was validated against (E6), the proposal revision a
    /// reviewer applied (I6), or a journey an entity merge was checked against (E6). A
    /// domain or proposal that does not exist is at revision 0.
    Revision {
        /// Whose revision.
        of: RevisionOf,
        /// The revision the patch was checked against.
        expected: Revision,
    },
    /// E6: an entity merge's referencing journeys are unchanged, so no journey outside
    /// `journeys` references any of `entities` (directly or through an alias). The
    /// journeys in the set are held by their own revision preconditions.
    ReferencingJourneys {
        /// The entities the merge touches.
        entities: BTreeSet<EntityKey>,
        /// The journeys it was checked against.
        journeys: BTreeSet<JourneyId>,
    },
}

/// One accepted patch to persist (A17: one transaction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    /// What the patch targets: a domain, or a proposal within its destination.
    pub target: PatchTarget,
    /// The target's revision the patch was applied to; 0 creates the target.
    pub base_revision: Revision,
    /// Every other revision the patch depends on.
    pub preconditions: Vec<Precondition>,
    /// The engine's result: the receipt and the events, whose writes are the change.
    pub change_set: ChangeSet,
}

/// A commit that went through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Committed {
    /// Persisted now.
    Applied(PatchReceipt),
    /// The patch id had already been committed with the same content (H5): nothing was
    /// written, and the receipt is the original one.
    AlreadyApplied(PatchReceipt),
}

impl Committed {
    /// The receipt, whichever way it went.
    #[must_use]
    pub fn receipt(&self) -> &PatchReceipt {
        match self {
            Committed::Applied(receipt) | Committed::AlreadyApplied(receipt) => receipt,
        }
    }
}

/// A commit that did not go through. Either way nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitError {
    /// The patch is stale, reuses a patch id, or breaks a rule only the commit can check
    /// (a deleted journey id, an entity key or email taken meanwhile, the graph size cap):
    /// the same rejection shapes the engine returns (A15, H5).
    Rejected(Rejection),
    /// The store failed or the change set was malformed.
    Failed(StoreError),
}

impl From<StoreError> for CommitError {
    fn from(error: StoreError) -> Self {
        CommitError::Failed(error)
    }
}

impl From<Rejection> for CommitError {
    fn from(rejection: Rejection) -> Self {
        CommitError::Rejected(rejection)
    }
}

/// A store failure: not an answer about the patch, but about the store or its caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The change set breaks a rule only a caller bug could break: it writes another
    /// domain, rewrites a published version, edits a field of a node that is not there.
    Malformed(String),
    /// No connection came free within `store_connection_acquire` (PRACTICES, Explicit
    /// limits): contention, which at this scale is a bug.
    AcquireTimeout,
    /// The backend failed: I/O, SQL, a stored row that no longer parses, or an injected
    /// fault.
    Backend(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Malformed(reason) => write!(formatter, "malformed change set: {reason}"),
            StoreError::AcquireTimeout => {
                formatter.write_str("no store connection came free within the acquire limit")
            }
            StoreError::Backend(reason) => write!(formatter, "store backend failed: {reason}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitError::Rejected(rejection) => write!(formatter, "commit rejected: {rejection:?}"),
            CommitError::Failed(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CommitError {}
