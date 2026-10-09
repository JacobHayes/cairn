//! The serde types every Cairn crate shares (ARCHITECTURE, Repository layout: `schema`):
//! identifiers, bounded text and numbers, the value-level limits they enforce, and the
//! graph model in its two forms, the file form (as written: paths and ids) and the graph
//! form (resolved: keys).
//!
//! Shapes, serialization, and value-level limits only: a value that parses is well formed,
//! and graph-level validation (references resolve, the tree is a tree, the plan is
//! consistent) is the engine's.

pub mod attachment;
pub mod chain;
pub mod collections;
pub mod condition;
pub mod derived;
pub mod document;
pub mod domain;
pub mod event;
pub mod field;
pub mod graph;
pub mod id;
pub mod identity;
pub mod json_schema;
pub mod limits;
pub mod node;
mod node_schema;
pub mod number;
pub mod patch;
pub mod projection;
pub mod proposal;
pub mod record;
pub mod refs;
pub mod rejection;
pub mod scenario;
mod serde_util;
pub mod state;
#[cfg(feature = "testing")]
pub mod testing;
pub mod text;
pub mod touched;

pub use attachment::{
    Annotation, AnnotationBody, AnnotationContent, AttachmentScope, JourneyField, MessageTemplate,
    Resource, ResourceContent, Segment,
};
pub use chain::{
    Chain, ChainList, Constraint, ConstraintSource, DependencyVia, FixedBy, FixedDate, Instant,
    InstantPoint, ShortChain,
};
pub use collections::{BoundedSet, BoundedVec, CollectionError, HasKey, Keyed, OneOrMany};
pub use condition::{Clause, Comparison, Condition, ConditionValue, Membership};
pub use derived::{
    Blocker, Bound, Consequences, Contribution, DateOrigin, DeriveInputs, Derived, DisplayState,
    DomainDocument, EffectiveDate, EffectiveParticipation, EngineVersion, Explained, NodeDates,
    NodeDerived, OwnerFactor, ParticipationOrigin, RankConstants, Real, Relevance,
    RelevanceExplanation, Score, ShortfallConsequence, StaleConsequence, StallCause, Stalled,
    Thousandths, TimeZoneName, UndecidedConsequence, UndecidedDiscount,
};
pub use document::{
    ParseError, WriteError, from_json, from_yaml, to_json, to_json_pretty, to_yaml,
};
pub use domain::{
    Deployment, Domain, Entity, GraphId, Journey, JourneyHeader, JourneyStatus, Lineage, Route,
    RouteDraft, RouteHeader, RouteVersion,
};
pub use event::{Actor, ChangeSet, ContentHash, Event, EventType, PatchReceipt, Subject};
pub use field::{NodeField, NodeFieldValue};
pub use graph::{Edge, FormatVersion, Graph, ParticipationKind, RetiredKeys, Role, RouteFile};
pub use id::{
    AgentId, AttachmentKey, ConversationId, EntityKey, IdError, JourneyId, KeyAllocator, KindKey,
    NodeKey, PatchId, Path, Prefixed, ProposalId, RoleKey, RouteId, SequentialKeys, Slug, UserId,
    mint,
};
pub use identity::Identity;
pub use jiff::Timestamp;
pub use jiff::civil::Date;
pub use limits::{Limit, LimitExceeded};
pub use node::{
    Action, AnswerSpec, AnswerType, Choice, Choices, DateRule, DateSource, Decision, Deliverable,
    Direction, EntitySet, Group, KindField, LabeledChoice, Milestone, Node, NodeKind,
    NodeShapeError, ParticipationSource, Participations, Payload,
};
pub use number::{Days, NumberError, Revision, SignedDays, VersionNumber, Weight};
pub use patch::{
    ChangeClass, DraftSource, Mutation, Mutations, Override, ParticipationRef, Patch, PatchTarget,
    RecordedEnd, Removal, Transition,
};
pub use projection::{
    Cursor, DecisionEntry, DecisionView, EdgeOrigin, ExplainedField, ExplanationPage, GroupState,
    HistoryPage, Level, LevelEdge, LevelNode, ListFlag, ListPage, ListQuery, MineEntry, Next,
    NextQuery, NodeRow, OpenDecision, PatchEvents, RankTerms, RenderedDraft, RenderedSegment,
    RollUp, Snapshot, SnapshotCounts, SnapshotNode, SnapshotScope, SortBy, StatusSummary, Timeline,
    TimelineEntry, Trace, UnderlyingEdge, UpcomingMilestone,
};
pub use proposal::{
    Conflict, ConflictResolution, Kept, ParticipationMapping, Proposal, ProposalDraft,
    ProposalPreview, ProposalStatus, ReviewItem, RoleReference, UnresolvedItem, UnresolvedReason,
    removed_choices,
};
pub use record::{GraphKey, GraphRecord, Record, RecordKey, RetiredKey, Write};
pub use refs::{FileRefs, KeyRefs, KeySlot, Reference, References};
pub use rejection::{
    Location, Rejection, RevisionConflict, RevisionOf, Violation, ViolationCode, Violations,
};
pub use scenario::{Scenario, ScenarioStep};
pub use state::{
    AnswerText, AnswerValue, Bypass, Guard, GuardFailure, JourneyState, LocalEdit, NodeState,
    OverrideKind, Overrides, Provenance, SnoozeTarget, State,
};
pub use text::{Email, Markdown, Reason, TextError, Title, Url};
pub use touched::TouchedSet;
