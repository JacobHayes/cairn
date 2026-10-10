//! Proposals (I6, C14; ARCHITECTURE, Engine > Model): a patch drafted but not applied,
//! outside every graph, with its own editing revision and the review items a reviewer
//! resolves before apply (B7 to B9, A18).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::Resource;
use crate::collections::{BoundedVec, ByDocumentSize, MutationCountPerPatch};
use crate::derived::Consequences;
use crate::domain::Domain;
use crate::field::NodeFieldValue;
use crate::graph::{Edge, Graph, ParticipationKind, Role};
use crate::id::{
    AgentId, AttachmentKey, EntityKey, InsertionKey, KindKey, NodeKey, ProposalId, RoleKey, Slug,
    UserId,
};
use crate::node::{Choices, EntitySet, Node, ParticipationSource};
use crate::notice::Notice;
use crate::number::Revision;
use crate::patch::{Mutation, ParticipationRef, Removal};
use crate::refs::KeyRefs;
use crate::rejection::Violation;
use crate::state::{AnswerValue, LocalEdit};
use crate::text::{Markdown, Title};
use jiff::Timestamp;

/// A proposal's status.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Under review: editable, appliable, discardable.
    Open,
    /// Applied to its destination.
    Applied,
    /// Discarded.
    Discarded,
}

/// How a reviewer resolves a conflict (B7, B9). Which resolutions a conflict offers depends
/// on what it is about ([`Conflict::offers`]); each becomes ordinary mutations when the
/// proposal is resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConflictResolution {
    /// Keep the journey's value as a local edit, or its definition as an override.
    KeepJourney,
    /// Take the route's value.
    TakeRoute,
    /// Take the route's choices and map each removed choice the answer names to one that
    /// remains (B7: map old to new).
    MapChoices {
        /// Removed choice to remaining choice.
        #[serde(deserialize_with = "crate::serde_util::unique_map")]
        map: BTreeMap<Slug, Slug>,
    },
    /// Take the route's value and clear the state it makes invalid.
    ClearState,
    /// Take the route's value and reopen the node.
    Reopen,
    /// Point every reference to a role the route removed at another role, then remove it.
    /// Not offered while an insertion maps a segment role onto it (B13).
    RemapRole {
        /// The role the references move to.
        role: RoleKey,
    },
    /// Move every participation of a kind the route removed to another kind, then remove it.
    /// Not offered while an insertion maps a segment kind onto it (B13).
    RemapKind {
        /// The kind the participations move to.
        kind: KindKey,
    },
    /// Clear every reference to a role or kind the route removed, then remove it. Where an
    /// insertion maps a segment role onto a removed role, the segment's nodes keep what the
    /// role held: whoever filled it now fills their participations directly, and the
    /// insertion's mapping goes with the role (B13).
    Remove,
}

/// Something in a journey that refers to a role (A6, E3): what remapping or removing the
/// role rewrites (B7).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RoleReference {
    /// A node's participation of a kind names the role.
    Participation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    /// An entity decision fills the role.
    FillsRole {
        /// The decision.
        node: NodeKey,
    },
    /// The role is filled directly with these entities.
    Fill {
        /// The entities.
        entities: EntitySet,
    },
    /// The role is the graph's `default_owner`.
    DefaultOwner,
    /// A participation of a kind on a node an insertion copied in, whose segment role the
    /// insertion mapped onto the role (B13): removing the role hands the node the role's fill
    /// directly.
    SegmentParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    /// A node's message draft names the role (A10).
    Draft {
        /// The node.
        node: NodeKey,
        /// The resource holding the draft, as the journey has it.
        resource: Resource<KeyRefs>,
    },
}

/// What a conflict is about, with the journey's side and the route's (B7, B9). A side that
/// is absent is `None` (an absent edge is `false`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "about", rename_all = "snake_case", deny_unknown_fields)]
pub enum Conflict {
    /// A node field the journey edited and the route changed to something else.
    Field {
        /// The node.
        node: NodeKey,
        /// The journey's value.
        journey: NodeFieldValue<KeyRefs>,
        /// The route's value.
        route: NodeFieldValue<KeyRefs>,
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (B4: a removed node's tombstone covers its subtree); only the journey's side is
        /// offered.
        #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
        dangling: bool,
    },
    /// An explicit edge the journey added or removed, and the route the other way.
    Edge {
        /// The edge.
        edge: Edge,
        /// Present in the journey.
        journey: bool,
        /// Present in the route.
        route: bool,
    },
    /// A participation the journey edited and the route changed to something else.
    Participation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
        /// The journey's source.
        #[serde(skip_serializing_if = "Option::is_none")]
        journey: Option<ParticipationSource<KeyRefs>>,
        /// The route's source.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<ParticipationSource<KeyRefs>>,
    },
    /// A resource the journey edited and the route changed to something else.
    Resource {
        /// The node.
        node: NodeKey,
        /// The resource.
        resource: AttachmentKey,
        /// The journey's resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        journey: Option<Resource<KeyRefs>>,
        /// The route's resource.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<Resource<KeyRefs>>,
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (B4: a removed node's tombstone covers its subtree); only the journey's side is
        /// offered.
        #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
        dangling: bool,
    },
    /// A node whose kind or answer type differs while the journey edited its shape or holds
    /// state the change would invalidate: the whole node is in conflict.
    Shape {
        /// The journey's node.
        journey: Box<Node<KeyRefs>>,
        /// The route's node.
        route: Box<Node<KeyRefs>>,
        /// The journey has answered it, so taking an answer-type change reopens it.
        #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
        answered: bool,
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (B4: a removed node's tombstone covers its subtree); only the journey's side is
        /// offered.
        #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
        dangling: bool,
    },
    /// A decided decision whose answer names choices the route's choices no longer have.
    Answer {
        /// The decision.
        decision: NodeKey,
        /// The journey's answer.
        answer: AnswerValue,
        /// The rationale the journey's answer was given with, kept when the answer is
        /// mapped onto the route's choices (B2).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rationale: Option<Markdown>,
        /// The route's choices.
        choices: Choices,
    },
    /// A role both sides changed, one whose new cardinality cannot hold the journey's direct
    /// fill, or one the route removed that the journey still refers to.
    Role {
        /// The role.
        role: RoleKey,
        /// The journey's role.
        #[serde(skip_serializing_if = "Option::is_none")]
        journey: Option<Role<KeyRefs>>,
        /// The route's role, none when it removed it.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<Role<KeyRefs>>,
        /// What in the journey refers to it.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        references: BTreeSet<RoleReference>,
        /// The insertions that map a segment role onto it, when the route removed it (B13):
        /// the role can then be kept or removed, not remapped.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        insertions: BTreeSet<InsertionKey>,
        /// Who fills the role now (E3), when insertions map onto it: removing it hands
        /// these to the segment's nodes directly.
        #[serde(default, skip_serializing_if = "EntitySet::is_empty")]
        members: EntitySet,
    },
    /// A participation kind both sides changed, or one the route removed that the journey
    /// still uses.
    Kind {
        /// The kind.
        kind: KindKey,
        /// The journey's kind.
        #[serde(skip_serializing_if = "Option::is_none")]
        journey: Option<ParticipationKind<KeyRefs>>,
        /// The route's kind, none when it removed it.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<ParticipationKind<KeyRefs>>,
        /// The journey's participations of the kind, by node.
        #[serde(
            default,
            skip_serializing_if = "BTreeMap::is_empty",
            deserialize_with = "crate::serde_util::unique_map"
        )]
        references: BTreeMap<NodeKey, ParticipationSource<KeyRefs>>,
        /// The insertions that map a segment kind onto it, when the route removed it (B13):
        /// the kind can then be kept or removed, not remapped.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        insertions: BTreeSet<InsertionKey>,
    },
    /// The graph's `default_owner`, which both sides changed.
    DefaultOwner {
        /// The journey's.
        #[serde(skip_serializing_if = "Option::is_none")]
        journey: Option<RoleKey>,
        /// The route's.
        #[serde(skip_serializing_if = "Option::is_none")]
        route: Option<RoleKey>,
    },
}

impl Conflict {
    /// B7: whether this conflict offers `resolution`. Every conflict offers keeping the
    /// journey's side; an edit also offers taking the route's, unless the route's names a node
    /// the journey does not hold; a shape change clears the
    /// state; an invalidated answer maps, clears, or reopens; a removed role or kind the
    /// journey still uses is remapped or removed, unless an insertion maps onto it, which
    /// leaves keeping or removing it; a role too narrow for the journey's direct fill clears
    /// it.
    #[must_use]
    pub fn offers(&self, resolution: &ConflictResolution) -> bool {
        use ConflictResolution as R;
        match (self, resolution) {
            (_, R::KeepJourney)
            | (
                Conflict::Field {
                    dangling: false, ..
                }
                | Conflict::Edge { .. }
                | Conflict::Participation { .. }
                | Conflict::Resource {
                    dangling: false, ..
                }
                | Conflict::DefaultOwner { .. }
                | Conflict::Kind { route: Some(_), .. },
                R::TakeRoute,
            )
            | (
                Conflict::Shape {
                    dangling: false, ..
                },
                R::ClearState,
            )
            | (Conflict::Answer { .. }, R::ClearState | R::Reopen)
            | (
                Conflict::Role { route: None, .. } | Conflict::Kind { route: None, .. },
                R::Remove,
            ) => true,
            (
                Conflict::Role {
                    route: None,
                    insertions,
                    ..
                },
                R::RemapRole { .. },
            )
            | (
                Conflict::Kind {
                    route: None,
                    insertions,
                    ..
                },
                R::RemapKind { .. },
            ) => insertions.is_empty(),
            (
                Conflict::Role {
                    route: Some(route),
                    references,
                    ..
                },
                R::TakeRoute | R::ClearState,
            ) => too_narrow(route, references) == (*resolution == R::ClearState),
            (
                Conflict::Answer {
                    answer, choices, ..
                },
                R::MapChoices { map },
            ) => maps_every_removed_choice(answer, choices, map),
            _ => false,
        }
    }
}

/// A6: a single-valued role cannot hold a direct fill of several entities.
fn too_narrow(role: &Role<KeyRefs>, references: &BTreeSet<RoleReference>) -> bool {
    !role.multi
        && references.iter().any(
            |reference| matches!(reference, RoleReference::Fill { entities } if entities.len() > 1),
        )
}

/// The choices an answer names that `choices` no longer has.
#[must_use]
pub fn removed_choices<'a>(answer: &'a AnswerValue, choices: &Choices) -> BTreeSet<&'a Slug> {
    let remaining: BTreeSet<&Slug> = choices.as_slice().iter().map(|choice| &choice.id).collect();
    let named: Vec<&Slug> = match answer {
        AnswerValue::SingleChoice(choice) => vec![choice],
        AnswerValue::MultiChoice(set) => set.iter().collect(),
        AnswerValue::Boolean(_)
        | AnswerValue::Text(_)
        | AnswerValue::Date(_)
        | AnswerValue::Entity(_)
        | AnswerValue::EntityList(_) => Vec::new(),
    };
    named
        .into_iter()
        .filter(|choice| !remaining.contains(choice))
        .collect()
}

/// True when `map` sends every choice the answer names and `choices` lacks to one of
/// `choices`, and maps nothing else.
fn maps_every_removed_choice(
    answer: &AnswerValue,
    choices: &Choices,
    map: &BTreeMap<Slug, Slug>,
) -> bool {
    let removed = removed_choices(answer, choices);
    !removed.is_empty()
        && map.len() == removed.len()
        && map.iter().all(|(from, to)| {
            removed.contains(from) && choices.as_slice().iter().any(|choice| choice.id == *to)
        })
}

/// A journey's deviation from its route that an upgrade keeps, listed for the reviewer (B7:
/// locally edited and unchanged in the route).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Kept {
    /// A marked edit on a route-copied node.
    Node {
        /// The node.
        node: NodeKey,
        /// What it edited.
        edit: LocalEdit,
    },
    /// A role the journey changed.
    Role(RoleKey),
    /// A participation kind the journey changed.
    Kind(KindKey),
    /// The graph's `default_owner`, which the journey changed.
    DefaultOwner,
}

/// What becomes of an explicit entity's participations when a journey is saved as a route
/// (B8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ParticipationMapping {
    /// Drop it.
    Drop,
    /// Map it to an existing role.
    Role(RoleKey),
    /// Map it to a new role.
    NewRole(Role<KeyRefs>),
    /// Map it to the graph's `default_owner` role.
    DefaultOwner,
}

/// One thing a reviewer looks at, and resolves where it needs a choice (C14). Resolving a
/// proposal turns each choice into mutations; an item that still needs one blocks apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "item", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewItem {
    /// Something the journey and the route both changed, or a route change that invalidates
    /// the journey's state (B7); a difference a re-link keeps by default (B9).
    Conflict {
        /// What it is about.
        conflict: Conflict,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        resolution: Option<ConflictResolution>,
    },
    /// Something the journey edited and the route left alone: kept, listed (B7).
    KeptLocalEdit {
        /// What is kept.
        kept: Kept,
    },
    /// A node the new version removed (B7): kept as orphaned by default.
    Orphan {
        /// The node.
        node: NodeKey,
        /// Keep it (the default) or remove it with its descendants.
        keep: bool,
        /// What removing it removes, shown in review (A18).
        removal: Removal,
    },
    /// An explicit entity on nodes being saved as a route (B8).
    Participation {
        /// The entity.
        entity: EntityKey,
        /// The participations naming it.
        uses: BTreeSet<ParticipationRef>,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        mapping: Option<ParticipationMapping>,
    },
    /// A node the reviewer may leave out of a saved route, with its subtree (B8).
    Exclusion {
        /// The node.
        node: NodeKey,
        /// Left out.
        excluded: bool,
    },
    /// Everything a removal in the proposal cascades to (A18).
    Cascade {
        /// The removal.
        removal: Removal,
    },
    /// An invariant the proposal's candidate broke when it was drafted (B7): shown, never
    /// applied silently, since apply validates strictly.
    Violation {
        /// The violation.
        violation: Violation,
    },
}

/// Why a review item blocks resolving its proposal.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// It needs a choice and has none.
    NoChoice,
    /// Its choice is not one the item offers.
    NotOffered,
    /// Its entity shares a participation with entities mapped to other roles (B8).
    MixedMapping,
    /// It maps to the graph's `default_owner`, and the graph has none (B8).
    NoDefaultOwner,
}

/// A review item that blocks resolving its proposal.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedItem {
    /// The item's position in the proposal.
    pub item: u32,
    /// Why.
    pub reason: UnresolvedReason,
}

/// What a proposal would do to its destination now (C14, I6), for review: the items still
/// unresolved, and the candidate the resolved mutations produce, with every violation strict
/// validation finds in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalPreview {
    /// Items that still need a choice; the preview leaves their effect out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<UnresolvedItem>,
    /// Every violation of the candidate; empty when the resolved proposal would apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<Violation>,
    /// The destination's graph after: a journey's, or a route's draft. None when the
    /// candidate is invalid, or for the deployment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<Graph>,
    /// For a journey, its frontier after, in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontier: Vec<NodeKey>,
    /// For a journey, what the change does to derived state (D7).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consequences: Option<Consequences>,
    /// For a route's draft, the advisory notices of the graph after (A20); they never make
    /// the proposal invalid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<Notice>,
}

/// A proposal's content: what creating or editing it carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ProposalDraftWire", into = "ProposalDraftWire")]
#[schemars(with = "ProposalDraftWire")]
pub struct ProposalDraft {
    /// What it proposes, in a line.
    pub title: Title,
    /// More.
    pub description: Option<Markdown>,
    /// The destination's revision it was drafted against; 0 when it creates the destination.
    pub destination_revision: Revision,
    /// The drafted mutations, in order; none of them edits a proposal.
    pub mutations: BoundedVec<Mutation, MutationCountPerPatch>,
    /// The review items.
    pub items: BoundedVec<ReviewItem, ByDocumentSize>,
}

/// A proposal's content as written.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ProposalDraft")]
pub struct ProposalDraftWire {
    title: Title,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Markdown>,
    destination_revision: Revision,
    #[serde(default, skip_serializing_if = "BoundedVec::is_empty")]
    mutations: BoundedVec<Mutation, MutationCountPerPatch>,
    #[serde(default, skip_serializing_if = "BoundedVec::is_empty")]
    items: BoundedVec<ReviewItem, ByDocumentSize>,
}

/// A proposal whose mutations include one that edits a proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NestedProposalError;

impl fmt::Display for NestedProposalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("a proposal's mutations cannot create, edit, apply, or discard a proposal")
    }
}

impl std::error::Error for NestedProposalError {}

impl TryFrom<ProposalDraftWire> for ProposalDraft {
    type Error = NestedProposalError;

    fn try_from(wire: ProposalDraftWire) -> Result<Self, NestedProposalError> {
        if wire
            .mutations
            .as_slice()
            .iter()
            .any(Mutation::edits_a_proposal)
        {
            return Err(NestedProposalError);
        }
        Ok(Self {
            title: wire.title,
            description: wire.description,
            destination_revision: wire.destination_revision,
            mutations: wire.mutations,
            items: wire.items,
        })
    }
}

impl From<ProposalDraft> for ProposalDraftWire {
    fn from(draft: ProposalDraft) -> Self {
        Self {
            title: draft.title,
            description: draft.description,
            destination_revision: draft.destination_revision,
            mutations: draft.mutations,
            items: draft.items,
        }
    }
}

/// A proposal as stored (ARCHITECTURE, Schema outline: `proposals`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    /// The client-generated id (I6).
    pub id: ProposalId,
    /// The domain it applies to, which may not exist yet.
    pub destination: Domain,
    /// Its editing revision, separate from the destination's (H5).
    pub revision: Revision,
    /// Its status.
    pub status: ProposalStatus,
    /// Its content.
    pub draft: ProposalDraft,
    /// The agent that drafted it, if any (I6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposing_agent: Option<AgentId>,
    /// The user it was created by or for.
    pub created_by: UserId,
    /// When it was created.
    pub created_at: Timestamp,
}

impl crate::collections::HasKey for Proposal {
    type Key = ProposalId;

    fn key(&self) -> &ProposalId {
        &self.id
    }
}
