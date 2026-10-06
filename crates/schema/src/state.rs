//! Journey state (PRD, How the pieces fit; D1, B4 to B6, E3, F1, G1): what has happened in
//! a journey, attached to nodes by key and to the journey as a whole. Nothing here is
//! derived (D3, D6): relevance, blocking, dates, and rank are computed on read.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::Annotation;
use crate::collections::{BoundedSet, ByDocumentSize, ChoiceCountPerDecision, Keyed};
use crate::field::NodeField;
use crate::id::{AttachmentKey, EntityKey, KindKey, NodeKey, RoleKey, Slug};
use crate::node::{AnswerType, EntitySet, NodeKind};
use crate::text::Reason;
use jiff::civil::Date;

/// A node's stored state (D1). Each kind has its own machine; [`State::legal_for`] says
/// which values belong to which kind. A group has no performed state: it is `derived`
/// unless explicitly `skipped`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Deliverable or action, not started.
    Todo,
    /// Deliverable or action, started.
    Active,
    /// Deliverable or action, completed.
    Done,
    /// Any kind, skipped with a reason.
    Skipped,
    /// Decision, not answered.
    Open,
    /// Decision, answered.
    Decided,
    /// Milestone, not reached.
    Pending,
    /// Milestone, reached.
    Reached,
    /// Group, not skipped: its display state is derived from its contents.
    Derived,
}

impl State {
    /// D1: whether `kind`'s machine has this state.
    #[must_use]
    pub const fn legal_for(self, kind: NodeKind) -> bool {
        match self {
            State::Todo | State::Active | State::Done => {
                matches!(kind, NodeKind::Deliverable | NodeKind::Action)
            }
            State::Skipped => true,
            State::Open | State::Decided => matches!(kind, NodeKind::Decision),
            State::Pending | State::Reached => matches!(kind, NodeKind::Milestone),
            State::Derived => matches!(kind, NodeKind::Group),
        }
    }

    /// B1: the state a node of `kind` starts in, on create, copy, and breakdown.
    #[must_use]
    pub const fn initial(kind: NodeKind) -> State {
        match kind {
            NodeKind::Deliverable | NodeKind::Action => State::Todo,
            NodeKind::Decision => State::Open,
            NodeKind::Milestone => State::Pending,
            NodeKind::Group => State::Derived,
        }
    }

    /// PRD glossary, Terminal: `done`, `skipped`, `decided`, `reached`.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        match self {
            State::Done | State::Skipped | State::Decided | State::Reached => true,
            State::Todo | State::Active | State::Open | State::Pending | State::Derived => false,
        }
    }
}

/// A node's origin in a journey (PRD glossary, Provenance).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Copied from the route version the journey follows.
    FromRoute,
    /// Created in the journey.
    Local,
    /// Copied from a route version that no longer has it (B7).
    Orphaned,
}

/// What a journey changed on a route-copied node (B4), so an upgrade leaves it alone. A
/// marker carries no reason (PRD glossary, Local edit).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalEdit {
    /// A field of the node.
    Field(NodeField),
    /// The node's explicit edge to the named requirement (added or removed).
    Requires(NodeKey),
    /// The node's participation of a kind.
    Participation(KindKey),
    /// One of the node's resources.
    Resource(AttachmentKey),
}

/// One node's stored state in a journey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeState {
    /// The machine state (D1).
    pub state: State,
    /// Where the node came from.
    pub provenance: Provenance,
    /// A placeholder marked atomic, so it can complete without children (B10).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub atomic: bool,
    /// When a deliverable or action was started (F2); editable like an actual date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_on: Option<Date>,
    /// When the node finished: the actual date a milestone was reached (F1), the day a
    /// decision was decided or work was done (F2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_on: Option<Date>,
}

impl NodeState {
    /// B1: a node's state when it is created or copied into a journey.
    #[must_use]
    pub fn initial(kind: NodeKind, provenance: Provenance) -> Self {
        Self {
            state: State::initial(kind),
            provenance,
            atomic: false,
            started_on: None,
            finished_on: None,
        }
    }
}

/// A decision's submitted answer (A4; PRD glossary, Answer). Submitted empty text or an
/// empty list is answered.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AnswerValue {
    /// A boolean answer.
    Boolean(bool),
    /// A choice id.
    SingleChoice(Slug),
    /// Choice ids.
    MultiChoice(BoundedSet<Slug, ChoiceCountPerDecision>),
    /// Text, possibly empty.
    Text(AnswerText),
    /// A date.
    Date(Date),
    /// An entity.
    Entity(EntityKey),
    /// Entities, possibly none.
    EntityList(EntitySet),
}

impl AnswerValue {
    /// The answer type this value has; a decision accepts it only when they match (an
    /// invariant, D4).
    #[must_use]
    pub fn answer_type(&self) -> AnswerType {
        match self {
            AnswerValue::Boolean(_) => AnswerType::Boolean,
            AnswerValue::SingleChoice(_) => AnswerType::SingleChoice,
            AnswerValue::MultiChoice(_) => AnswerType::MultiChoice,
            AnswerValue::Text(_) => AnswerType::Text,
            AnswerValue::Date(_) => AnswerType::Date,
            AnswerValue::Entity(_) => AnswerType::Entity,
            AnswerValue::EntityList(_) => AnswerType::EntityList,
        }
    }
}

/// A text answer: any text up to the body limit, empty included (an empty submission is an
/// answer).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct AnswerText(String);

impl AnswerText {
    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AnswerText {
    type Error = crate::limits::LimitExceeded;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        crate::limits::Limit::BodyBytes.check(text.len())?;
        Ok(Self(text))
    }
}

impl From<AnswerText> for String {
    fn from(text: AnswerText) -> String {
        text.0
    }
}

/// What a snooze waits for (B6): exactly one of a date or a node.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SnoozeTarget {
    /// Holds while today is before the date.
    Date(Date),
    /// Holds while the node neither satisfies dependencies nor is not relevant.
    Node(NodeKey),
}

/// A transition guard (D4).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Guard {
    /// Every relevant or undecided hard dependency satisfies dependencies.
    DepsDone,
    /// A `requires_artifact` deliverable has an artifact link.
    HasArtifact,
    /// A placeholder has children or is atomic.
    BrokenDown,
}

/// One specific guard failure (D4): what a bypass records, so a later distinct failure
/// still produces `stale`, and what `stale` lists.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum GuardFailure {
    /// A dependency that does not satisfy dependencies.
    OpenDependency(NodeKey),
    /// No artifact link on a `requires_artifact` deliverable.
    MissingArtifact,
    /// A placeholder with no children that is not atomic.
    NotBrokenDown,
}

impl GuardFailure {
    /// The guard this failure belongs to.
    #[must_use]
    pub fn guard(&self) -> Guard {
        match self {
            GuardFailure::OpenDependency(_) => Guard::DepsDone,
            GuardFailure::MissingArtifact => Guard::HasArtifact,
            GuardFailure::NotBrokenDown => Guard::BrokenDown,
        }
    }
}

/// A guard bypass as stored (D4): the guards bypassed, why, and the specific failures
/// present when it was applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bypass {
    /// The guards bypassed.
    pub guards: BTreeSet<Guard>,
    /// Why.
    pub reason: Reason,
    /// The failures present when the bypass was applied.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub failures: BTreeSet<GuardFailure>,
}

/// A node's overrides (PRD glossary, Override): reasoned, logged departures.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    /// Treat the node as relevant despite conditions (Gating, Force include).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_include: Option<Reason>,
    /// Exempt the node and its subtree from an ancestor's skip (D1a).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep: Option<Reason>,
    /// A guard bypass on the node's transition (D4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass: Option<Bypass>,
}

impl Overrides {
    /// True when the node has no override.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.force_include.is_none() && self.keep.is_none() && self.bypass.is_none()
    }
}

/// The kind of an override.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OverrideKind {
    /// Force include.
    ForceInclude,
    /// Keep under a skipped ancestor.
    Keep,
    /// Guard bypass.
    GuardBypass,
}

/// A journey's state, on its graph document. Maps are keyed and sorted, so a state has one
/// serialization.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyState {
    /// Each node's stored state.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<NodeKey, NodeState>,
    /// Each route-copied node's local edits (B4), apart from its state so that an edit and
    /// a transition on one node touch different records (H5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub local_edits: BTreeMap<NodeKey, BTreeSet<LocalEdit>>,
    /// Answers by decision.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub answers: BTreeMap<NodeKey, AnswerValue>,
    /// Direct role fills, for roles without a filling decision (E3). A role with a filling
    /// decision is filled from that decision's answer, which is derived, not stored here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub role_fills: BTreeMap<RoleKey, EntitySet>,
    /// Pins: explicit journey-level dates on nodes (PRD glossary, Pin). A pin from a
    /// `feeds_milestone` answer is derived from the answer, not stored here (E3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pins: BTreeMap<NodeKey, Date>,
    /// Snoozes by node (B6).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub snoozes: BTreeMap<NodeKey, SnoozeTarget>,
    /// Overrides by node.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<NodeKey, Overrides>,
    /// Route-copied nodes the journey removed, so an upgrade does not re-add them (B4).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub tombstones: BTreeSet<NodeKey>,
    /// Notes and links (G1).
    #[serde(default, skip_serializing_if = "Keyed::is_empty")]
    pub annotations: Keyed<Annotation, ByDocumentSize>,
}

impl JourneyState {
    /// True for a route version or draft: no state at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.local_edits.is_empty()
            && self.answers.is_empty()
            && self.role_fills.is_empty()
            && self.pins.is_empty()
            && self.snoozes.is_empty()
            && self.overrides.is_empty()
            && self.tombstones.is_empty()
            && self.annotations.is_empty()
    }
}
