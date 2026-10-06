//! The store (ARCHITECTURE, Storage): the [`Store`] trait the service layer loads domains
//! and commits change sets through, the in-memory backend that is its reference
//! implementation, and the conformance suite every backend passes (feature `conformance`).
//!
//! The store depends on `cairn-schema`, never on the engine: it persists the change set an
//! accepted patch produced (its events' writes) and checks only what a commit must check
//! atomically: revisions (H5), patch receipts, deleted journey ids (A19), entity keys and
//! emails riding in from other domains (E6), and the per-graph size cap.
//!
//! The trait names no runtime. The memory backend completes every call at once, so it runs
//! on any executor, including the browser's event loop.

pub mod backend;
#[cfg(any(test, feature = "conformance"))]
pub mod build;
pub mod commit;
#[cfg(feature = "conformance")]
pub mod conformance;
pub mod faults;
pub mod limits;
pub mod memory;
pub mod query;
pub mod records;
pub mod store;
pub mod target;

pub use commit::{Commit, CommitError, Committed, Precondition, StoreError};
pub use faults::{CommitPoint, Faults, PauseHook};
pub use memory::MemoryStore;
pub use query::{
    EventQuery, JourneyMatches, JourneyQuery, JourneySummary, LoggedEvent, Page, PageSize,
    Revisions, RouteDetail, SearchHit, SearchQuery, VersionJourneys,
};
pub use records::{
    AgentTokenRecord, AuthEvent, AuthLogEntry, AuthLogQuery, ConversationMessage,
    ConversationRecord, ConversationSummary, IdentityRecord, LoggedAuthEntry, MessageAuthor,
    OAuthStateKind, OAuthStateRecord, SecretHash, SessionRecord, UserRecord,
};
pub use store::{AuthStore, ConversationStore, Store};
pub use target::{Document, LoadTarget};
