//! Events (J1), the change set an accepted patch produces (ARCHITECTURE, Terms: Change set),
//! and the patch receipt a resubmission is answered from (H5).

use std::fmt::{self, Write as _};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::document::to_json;
use crate::domain::Domain;
use crate::field::NodeField;
use crate::graph::Edge;
use crate::id::{
    AgentId, AttachmentKey, EntityKey, JourneyId, KindKey, NodeKey, PatchId, ProposalId, RoleKey,
    RouteId, UserId,
};
use crate::number::Revision;
use crate::patch::{DraftSource, Mutation, Override, Patch, PatchTarget, Transition};
use crate::record::Write;
use crate::text::Markdown;
use crate::touched::TouchedSet;
use jiff::Timestamp;

/// An event's type (J1's list). Most mutations have one type; a few take theirs from
/// their content ([`Mutation::event_type`]).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum EventType {
    JourneyCreated,
    JourneyEdited,
    JourneyStatusChanged,
    JourneyDeleted,
    RouteCreated,
    RouteEdited,
    RouteRetired,
    RouteUnretired,
    DraftOpened,
    RouteVersionImported,
    SavedAsRoute,
    DraftDiscarded,
    RoutePublished,
    NodeAdded,
    NodeChanged,
    WeightChanged,
    NodeRemoved,
    EdgeChanged,
    RoleChanged,
    ParticipationKindChanged,
    DefaultOwnerChanged,
    ParticipationChanged,
    ResourceChanged,
    NodeTransitioned,
    MilestoneReached,
    AnswerSet,
    RecordedDateChanged,
    RoleFillChanged,
    DatePinned,
    DateShifted,
    DateUnpinned,
    SnoozeSet,
    Unsnoozed,
    OverrideApplied,
    GuardBypassed,
    OverrideRemoved,
    AtomicChanged,
    AnnotationAdded,
    AnnotationEdited,
    AnnotationRemoved,
    JourneyUpgraded,
    Relinked,
    ProvenanceChanged,
    LocalEditChanged,
    EntityCreated,
    EntityEdited,
    EntitiesMerged,
    ProposalCreated,
    ProposalEdited,
    ProposalApplied,
    ProposalDiscarded,
}

/// What an event is about: its subject key (J1).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    /// A node.
    Node(NodeKey),
    /// An edge.
    Edge(Edge),
    /// A role.
    Role(RoleKey),
    /// A participation kind.
    Kind(KindKey),
    /// A resource, note, or link.
    Attachment(AttachmentKey),
    /// An entity.
    Entity(EntityKey),
    /// A journey.
    Journey(JourneyId),
    /// A route.
    Route(RouteId),
    /// A proposal.
    Proposal(ProposalId),
    /// The deployment.
    Deployment,
}

/// Who made a change (H2): a user, or an agent acting for one.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    /// The user.
    pub user: UserId,
    /// The agent acting for the user, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
}

/// An append-only audit record of one mutation (J1). The delta is the after-state of every
/// record the mutation wrote or removed, in order, so replay applies it with nothing
/// re-derived; events never hold derived values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// The patch it came from; a patch's events share it.
    pub patch_id: PatchId,
    /// Its position in the patch, from 0.
    pub ordinal: u32,
    /// The log it belongs to: its patch's domain, except a journey's deletion, which the
    /// deployment log keeps (A19).
    pub log: Domain,
    /// Its type.
    pub event_type: EventType,
    /// Who made it.
    pub actor: Actor,
    /// Who confirmed it, when a proposal was applied (H2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirming_user: Option<UserId>,
    /// What it is about.
    pub subject: Subject,
    /// When it was committed.
    pub at: Timestamp,
    /// An optional note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
    /// What it wrote, as after-state.
    pub delta: Vec<Write>,
}

impl Event {
    /// H5: the records this event wrote or removed.
    #[must_use]
    pub fn touched(&self) -> TouchedSet {
        self.delta.iter().flat_map(Write::keys).collect()
    }
}

impl Mutation {
    /// J1: the type of the event this mutation emits.
    #[must_use]
    pub fn event_type(&self) -> EventType {
        match self {
            Mutation::AddNode { .. } => EventType::NodeAdded,
            Mutation::SetNodeField { value, .. } if value.field() == NodeField::Weight => {
                EventType::WeightChanged
            }
            Mutation::SetNodeField { .. } | Mutation::ReplaceNode { .. } => EventType::NodeChanged,
            Mutation::RemoveNode { .. } => EventType::NodeRemoved,
            Mutation::AddEdge { .. } | Mutation::RemoveEdge { .. } => EventType::EdgeChanged,
            Mutation::AddRole { .. } | Mutation::EditRole { .. } | Mutation::RemoveRole { .. } => {
                EventType::RoleChanged
            }
            Mutation::AddParticipationKind { .. }
            | Mutation::EditParticipationKind { .. }
            | Mutation::RemoveParticipationKind { .. } => EventType::ParticipationKindChanged,
            Mutation::SetDefaultOwner { .. } => EventType::DefaultOwnerChanged,
            Mutation::SetParticipation { .. } | Mutation::ClearParticipation { .. } => {
                EventType::ParticipationChanged
            }
            Mutation::AddResource { .. }
            | Mutation::EditResource { .. }
            | Mutation::RemoveResource { .. } => EventType::ResourceChanged,
            // One event records a container's skip; the cascade is derived (D1a).
            Mutation::Transition {
                transition: Transition::Reach,
                ..
            } => EventType::MilestoneReached,
            Mutation::Transition { .. } => EventType::NodeTransitioned,
            Mutation::Answer { .. } => EventType::AnswerSet,
            Mutation::SetRecordedDate { .. } => EventType::RecordedDateChanged,
            Mutation::FillRole { .. } | Mutation::ClearRoleFill { .. } => {
                EventType::RoleFillChanged
            }
            Mutation::SetPin { .. } => EventType::DatePinned,
            Mutation::ShiftPin { .. } => EventType::DateShifted,
            Mutation::ClearPin { .. } => EventType::DateUnpinned,
            Mutation::Snooze { .. } => EventType::SnoozeSet,
            Mutation::Unsnooze { .. } => EventType::Unsnoozed,
            Mutation::ApplyOverride {
                applied: Override::GuardBypass { .. },
                ..
            } => EventType::GuardBypassed,
            Mutation::ApplyOverride { .. } => EventType::OverrideApplied,
            Mutation::RemoveOverride { .. } => EventType::OverrideRemoved,
            Mutation::SetAtomic { .. } => EventType::AtomicChanged,
            Mutation::AddAnnotation { .. } => EventType::AnnotationAdded,
            Mutation::EditAnnotation { .. } => EventType::AnnotationEdited,
            Mutation::RemoveAnnotation { .. } => EventType::AnnotationRemoved,
            other => other.domain_event_type(),
        }
    }

    /// The event types of the mutations that change a domain rather than a graph's content
    /// or state. Every variant is listed in one of the two functions; the coverage test over
    /// a patch holding every mutation keeps it so.
    fn domain_event_type(&self) -> EventType {
        match self {
            Mutation::CreateJourney { .. } => EventType::JourneyCreated,
            Mutation::EditJourney { .. } => EventType::JourneyEdited,
            Mutation::SetJourneyStatus { .. } => EventType::JourneyStatusChanged,
            Mutation::DeleteJourney => EventType::JourneyDeleted,
            Mutation::CreateRoute { .. } => EventType::RouteCreated,
            Mutation::EditRoute { .. } => EventType::RouteEdited,
            Mutation::SetRouteRetired { retired: true } => EventType::RouteRetired,
            Mutation::SetRouteRetired { retired: false } => EventType::RouteUnretired,
            Mutation::OpenDraft {
                source: DraftSource::Edit,
            } => EventType::DraftOpened,
            Mutation::OpenDraft {
                source: DraftSource::Import,
            } => EventType::RouteVersionImported,
            Mutation::OpenDraft {
                source: DraftSource::SaveAsRoute { .. },
            } => EventType::SavedAsRoute,
            Mutation::DiscardDraft => EventType::DraftDiscarded,
            Mutation::PublishDraft => EventType::RoutePublished,
            Mutation::Upgrade { .. } => EventType::JourneyUpgraded,
            Mutation::Relink { .. } => EventType::Relinked,
            Mutation::SetProvenance { .. } => EventType::ProvenanceChanged,
            Mutation::MarkLocalEdit { .. } => EventType::LocalEditChanged,
            Mutation::CreateEntity { .. } => EventType::EntityCreated,
            Mutation::EditEntity { .. } => EventType::EntityEdited,
            Mutation::MergeEntities { .. } => EventType::EntitiesMerged,
            Mutation::CreateProposal { .. } => EventType::ProposalCreated,
            Mutation::EditProposal { .. } => EventType::ProposalEdited,
            Mutation::ApplyProposal { .. } => EventType::ProposalApplied,
            Mutation::DiscardProposal => EventType::ProposalDiscarded,
            graph_mutation => unreachable!("{graph_mutation:?} is typed by Mutation::event_type"),
        }
    }

    /// J1: the key the event this mutation emits is about, in a patch to `target`.
    #[must_use]
    pub fn subject(&self, target: &PatchTarget) -> Subject {
        match self {
            Mutation::AddNode { node } | Mutation::ReplaceNode { node } => {
                Subject::Node(node.key.clone())
            }
            Mutation::RemoveNode { removal } => Subject::Node(removal.node.clone()),
            Mutation::Answer { decision, .. } => Subject::Node(decision.clone()),
            Mutation::AddEdge { edge } | Mutation::RemoveEdge { edge } => {
                Subject::Edge(edge.clone())
            }
            Mutation::AddRole { role } | Mutation::EditRole { role } => {
                Subject::Role(role.key.clone())
            }
            Mutation::RemoveRole { role }
            | Mutation::FillRole { role, .. }
            | Mutation::ClearRoleFill { role }
            | Mutation::SetDefaultOwner { role: Some(role) } => Subject::Role(role.clone()),
            Mutation::AddParticipationKind { kind } | Mutation::EditParticipationKind { kind } => {
                Subject::Kind(kind.key.clone())
            }
            Mutation::RemoveParticipationKind { kind } => Subject::Kind(kind.clone()),
            Mutation::AddResource { resource, .. } | Mutation::EditResource { resource, .. } => {
                Subject::Attachment(resource.key.clone())
            }
            Mutation::RemoveResource { resource: key, .. }
            | Mutation::RemoveAnnotation { annotation: key } => Subject::Attachment(key.clone()),
            Mutation::AddAnnotation { annotation } | Mutation::EditAnnotation { annotation } => {
                Subject::Attachment(annotation.key.clone())
            }
            Mutation::CreateEntity { entity } | Mutation::EditEntity { entity } => {
                Subject::Entity(entity.key.clone())
            }
            Mutation::MergeEntities { merged, .. } => Subject::Entity(merged.clone()),
            Mutation::ApplyProposal { proposal, .. } => Subject::Proposal(proposal.clone()),
            other => other
                .node_subject()
                .unwrap_or_else(|| domain_subject(target)),
        }
    }

    /// The node a state or field mutation is about; none for a mutation about its domain.
    fn node_subject(&self) -> Option<Subject> {
        match self {
            Mutation::SetNodeField { node, .. }
            | Mutation::SetParticipation { node, .. }
            | Mutation::ClearParticipation { node, .. }
            | Mutation::Transition { node, .. }
            | Mutation::SetRecordedDate { node, .. }
            | Mutation::SetPin { node, .. }
            | Mutation::ShiftPin { node, .. }
            | Mutation::ClearPin { node }
            | Mutation::Snooze { node, .. }
            | Mutation::Unsnooze { node }
            | Mutation::ApplyOverride { node, .. }
            | Mutation::RemoveOverride { node, .. }
            | Mutation::SetAtomic { node, .. }
            | Mutation::SetProvenance { node, .. }
            | Mutation::MarkLocalEdit { node, .. } => Some(Subject::Node(node.clone())),
            _ => None,
        }
    }
}

/// The subject of a mutation about its whole domain: the journey, route, deployment, or
/// proposal the patch targets.
fn domain_subject(target: &PatchTarget) -> Subject {
    match target {
        PatchTarget::Journey(journey) => Subject::Journey(journey.clone()),
        PatchTarget::Route(route) => Subject::Route(route.clone()),
        PatchTarget::Deployment => Subject::Deployment,
        PatchTarget::Proposal { id, .. } => Subject::Proposal(id.clone()),
    }
}

/// A SHA-256 digest of a patch's canonical JSON, in lower-case hex (H5: a resubmitted patch
/// id is matched to its receipt by content).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// The hex digest.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for ContentHash {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let hex = text.len() == 64
            && text
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
        if hex {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!("{text:?} is not a lower-case hex SHA-256 digest"))
        }
    }
}

crate::serde_util::string_serde!(
    ContentHash,
    "ContentHash",
    "A lower-case hex SHA-256 digest.",
    Some("^[0-9a-f]{64}$")
);

impl Patch {
    /// H5: the patch's content hash, over its canonical JSON (the deterministic wire form,
    /// id included). Two patches with the same hash are the same patch.
    ///
    /// # Panics
    ///
    /// Never: a patch always serializes.
    #[must_use]
    pub fn content_hash(&self) -> ContentHash {
        let json = match to_json(self) {
            Ok(json) => json,
            Err(error) => panic!("a patch always serializes: {error}"),
        };
        let digest = Sha256::digest(json.as_bytes());
        let mut hex = String::with_capacity(64);
        for byte in digest {
            // Writing to a String cannot fail.
            let _ = write!(hex, "{byte:02x}");
        }
        ContentHash(hex)
    }
}

/// What a committed patch id is answered from on resubmission (H5; ARCHITECTURE, Schema
/// outline: `patch_receipts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchReceipt {
    /// The patch id.
    pub patch_id: PatchId,
    /// The domain it was committed to.
    pub domain: Domain,
    /// Its content hash: the same id with other content is rejected.
    pub content_hash: ContentHash,
    /// The revision it produced.
    pub revision: Revision,
}

/// The persistence-neutral result of an accepted patch (ARCHITECTURE, Terms: Change set):
/// its events, whose deltas are every record put and removed, the receipt, and the next
/// revision. Only the engine's apply produces one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeSet {
    /// The receipt to store: patch id, domain, content hash, and the revision produced.
    pub receipt: PatchReceipt,
    /// One event per mutation, in order (J2).
    pub events: Vec<Event>,
}

impl ChangeSet {
    /// Every write, in the order a store applies them.
    pub fn writes(&self) -> impl Iterator<Item = &Write> {
        self.events.iter().flat_map(|event| event.delta.iter())
    }

    /// H5: everything the patch wrote or removed.
    #[must_use]
    pub fn touched(&self) -> TouchedSet {
        let mut touched = TouchedSet::default();
        for event in &self.events {
            touched.extend(event.touched());
        }
        touched
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::String(name)) => formatter.write_str(&name),
            _ => write!(formatter, "{self:?}"),
        }
    }
}
