//! Proposals (I6, C14; ARCHITECTURE, Engine > Model): a patch drafted but not applied,
//! outside every graph, with its own editing revision and the review items a reviewer
//! resolves before apply (B7 to B9, A18).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::Resource;
use crate::collections::{BoundedVec, ByDocumentSize, MutationCountPerPatch};
use crate::derived::{Consequences, DomainDocument};
use crate::domain::{Deployment, Domain, JourneyStatus, Route, RouteVersion};
use crate::field::NodeFieldValue;
use crate::graph::{Edge, ParticipationKind, Role};
use crate::id::{
    AgentId, AttachmentKey, IdentityKey, InsertionKey, KindKey, NodeKey, ProposalId, RoleKey, Slug,
    UserId,
};
use crate::node::{Choices, IdentitySet, Node, ParticipationSource};
use crate::notice::Notice;
use crate::number::{Revision, VersionNumber};
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

// PRD: B7, B9.
/// How a reviewer resolves a conflict. Which resolutions a conflict offers depends
/// on what it is about ([`Conflict::offers`]); each becomes ordinary mutations when the
/// proposal is resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConflictResolution {
    /// Keep the journey's value as a local edit, or its definition as an override.
    KeepJourney,
    /// Take the route's value.
    TakeRoute,
    // PRD: B7.
    /// Take the route's choices and map each removed choice the answer names to one that
    /// remains (map old to new).
    MapChoices {
        /// Removed choice to remaining choice.
        #[serde(deserialize_with = "crate::serde_util::unique_map")]
        map: BTreeMap<Slug, Slug>,
    },
    /// Take the route's value and clear the state it makes invalid.
    ClearState,
    /// Take the route's value and reopen the node.
    Reopen,
    // PRD: B13.
    /// Point every reference to a role the route removed at another role, then remove it.
    /// Not offered while an insertion maps a segment role onto it.
    RemapRole {
        /// The role the references move to.
        role: RoleKey,
    },
    // PRD: B13.
    /// Move every participation of a kind the route removed to another kind, then remove it.
    /// Not offered while an insertion maps a segment kind onto it.
    RemapKind {
        /// The kind the participations move to.
        kind: KindKey,
    },
    // PRD: B13.
    /// Clear every reference to a role or kind the route removed, then remove it. Where an
    /// insertion maps a segment role onto a removed role, the segment's nodes keep what the
    /// role held: whoever filled it now fills their participations directly, and the
    /// insertion's mapping goes with the role.
    Remove,
}

// PRD: A6, E3, B7.
/// Something in a journey that refers to a role: what remapping or removing the
/// role rewrites.
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
    /// An identity decision fills the role.
    FillsRole {
        /// The decision.
        node: NodeKey,
    },
    /// The role is filled directly with these identities.
    Fill {
        /// The identities.
        identities: IdentitySet,
    },
    /// The role is the graph's `default_owner`.
    DefaultOwner,
    // PRD: B13.
    /// A participation of a kind on a node an insertion copied in, whose segment role the
    /// insertion mapped onto the role: removing the role hands the node the role's fill
    /// directly.
    SegmentParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    // PRD: A10.
    /// A node's message draft names the role.
    Draft {
        /// The node.
        node: NodeKey,
        /// The resource holding the draft, as the journey has it.
        resource: Resource<KeyRefs>,
    },
}

// PRD: B7, B9.
/// What a conflict is about, with the journey's side and the route's. A side that
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
        // PRD: B4.
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (a removed node's tombstone covers its subtree); only the journey's side is
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
        // PRD: B4.
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (a removed node's tombstone covers its subtree); only the journey's side is
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
        // PRD: B4.
        /// The route's value names a node the journey does not hold, so it cannot be taken
        /// (a removed node's tombstone covers its subtree); only the journey's side is
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
        // PRD: B2.
        /// The rationale the journey's answer was given with, kept when the answer is
        /// mapped onto the route's choices.
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
        // PRD: B13.
        /// The insertions that map a segment role onto it, when the route removed it:
        /// the role can then be kept or removed, not remapped.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        insertions: BTreeSet<InsertionKey>,
        // PRD: E3.
        /// Who fills the role now, when insertions map onto it: removing it hands
        /// these to the segment's nodes directly.
        #[serde(default, skip_serializing_if = "IdentitySet::is_empty")]
        members: IdentitySet,
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
        // PRD: B13.
        /// The insertions that map a segment kind onto it, when the route removed it:
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

/// A6: a single-valued role cannot hold a direct fill of several identities.
fn too_narrow(role: &Role<KeyRefs>, references: &BTreeSet<RoleReference>) -> bool {
    !role.multi
        && references.iter().any(
            |reference| matches!(reference, RoleReference::Fill { identities } if identities.len() > 1),
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
        | AnswerValue::Identity(_)
        | AnswerValue::IdentityList(_) => Vec::new(),
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

// PRD: B7.
/// A journey's deviation from its route that an upgrade keeps, listed for the reviewer (
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

// PRD: B8.
/// What becomes of an explicit identity's participations when a journey is saved as a route.
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

// PRD: C14.
/// One thing a reviewer looks at, and resolves where it needs a choice. Resolving a
/// proposal turns each choice into mutations; an item that still needs one blocks apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "item", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewItem {
    // PRD: B7, B9.
    /// Something the journey and the route both changed, or a route change that invalidates
    /// the journey's state; a difference a re-link keeps by default.
    Conflict {
        /// What it is about.
        conflict: Conflict,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        resolution: Option<ConflictResolution>,
    },
    // PRD: B7.
    /// Something the journey edited and the route left alone: kept, listed.
    KeptLocalEdit {
        /// What is kept.
        kept: Kept,
    },
    // PRD: B7.
    /// A node the new version removed: kept as orphaned by default.
    Orphan {
        /// The node.
        node: NodeKey,
        /// Keep it (the default) or remove it with its descendants.
        keep: bool,
        // PRD: A18.
        /// What removing it removes, shown in review.
        removal: Removal,
    },
    // PRD: B8.
    /// An explicit identity on nodes being saved as a route.
    Participation {
        /// The identity.
        identity: IdentityKey,
        /// The participations naming it.
        uses: BTreeSet<ParticipationRef>,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        mapping: Option<ParticipationMapping>,
    },
    // PRD: B8.
    /// A node the reviewer may leave out of a saved route, with its subtree.
    Exclusion {
        /// The node.
        node: NodeKey,
        /// Left out.
        excluded: bool,
    },
    // PRD: A18.
    /// Everything a removal in the proposal cascades to.
    Cascade {
        /// The removal.
        removal: Removal,
    },
    // PRD: B7.
    /// An invariant the proposal's candidate broke when it was drafted: shown, never
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
    // PRD: B8.
    /// Its identity shares a participation with identities mapped to other roles.
    MixedMapping,
    // PRD: B8.
    /// It maps to the graph's `default_owner`, and the graph has none.
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

// PRD: C14, I6.
/// What a proposal would do to its destination now, for review: the items still
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
    /// For a journey, its frontier after, in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frontier: Vec<NodeKey>,
    // PRD: D7.
    /// For a journey, what the change does to derived state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consequences: Option<Consequences>,
    // PRD: A20.
    /// For a route's draft, the advisory notices of the graph after; they never make
    /// the proposal invalid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<Notice>,
    /// What the proposal as previewed (the reviewer's edits included) does, in facts.
    pub outline: ProposalOutline,
}

// PRD: C14.
/// One thing a proposal does to its destination's own header or lifecycle, as a fact
/// the listing and the review word the same way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "fact", rename_all = "snake_case")]
pub enum OutlineFact {
    /// Creates the journey.
    CreatesJourney,
    /// Renames or redescribes the journey.
    EditsJourney,
    /// Sets the journey's status.
    SetsJourneyStatus {
        /// The status.
        status: JourneyStatus,
    },
    /// Creates the route.
    CreatesRoute,
    /// Renames or redescribes the route.
    EditsRoute,
    /// Retires the route.
    RetiresRoute,
    /// Brings the route back.
    RestoresRoute,
    /// Opens the route's draft.
    OpensDraft,
    /// Discards the route's draft.
    DiscardsDraft,
    /// Publishes the route's draft.
    PublishesDraft,
    /// Moves the journey to a newer version of its route.
    Upgrades {
        /// The version.
        to: VersionNumber,
    },
    /// Links the journey to the version its saved route published.
    Relinks,
    /// Inserts a published segment.
    InsertsSegment,
}

// PRD: C14.
/// What a proposal does, as counts of the things it names: the facts about its
/// destination's header first, then the nodes it adds, changes and removes (each node once,
/// by the mutations that name it), the requirements, the identities, and everything else in one
/// count. Derived from the mutations alone on every read, never stored.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalOutline {
    /// The header and lifecycle facts, in the order the mutations make them, each once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<OutlineFact>,
    /// Nodes it adds.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub nodes_added: u32,
    /// Nodes it changes: definition or state; a node it adds and then changes counts as added.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub nodes_changed: u32,
    /// Nodes it removes, a removal's descendants included.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub nodes_removed: u32,
    /// Requirements it adds.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub requirements_added: u32,
    /// Requirements it removes.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub requirements_removed: u32,
    /// Identities it creates.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub identities_created: u32,
    /// Identities it edits.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub identities_edited: u32,
    /// Identities it merges into another.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub identities_merged: u32,
    /// Identities it deletes.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub identities_deleted: u32,
    /// Journeys it deletes.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub journeys_deleted: u32,
    /// Routes and segments it deletes.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub routes_deleted: u32,
    /// Roles, participation kinds, role fills, the default owner and annotations it changes.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_zero")]
    pub other: u32,
}

impl ProposalOutline {
    /// C14: the facts `mutations` make, in order, and the nodes, requirements, identities and
    /// other things they name. A node is counted once, by the first thing that happened to it:
    /// a node added and then changed is added, one added and then removed is not counted at
    /// all; state mutations (pins, answers, transitions, snoozes, overrides) count as changes
    /// to their node. An upgrade or a segment insertion is a fact of its own: the nodes it adds
    /// or changes are the preview's, not the mutations'.
    #[must_use]
    pub fn of(mutations: &[Mutation]) -> Self {
        let mut outline = Self::default();
        let mut added: BTreeSet<&NodeKey> = BTreeSet::new();
        let mut changed: BTreeSet<&NodeKey> = BTreeSet::new();
        let mut removed: BTreeSet<&NodeKey> = BTreeSet::new();
        for mutation in mutations {
            if tally_other(&mut outline, mutation) {
                continue;
            }
            match mutation {
                Mutation::AddNode { node } => {
                    removed.remove(&node.key);
                    added.insert(&node.key);
                }
                Mutation::RemoveNode { removal } => {
                    for key in removal.nodes() {
                        // Added by this proposal and removed again: nothing is left to name.
                        if !added.remove(key) {
                            changed.remove(key);
                            removed.insert(key);
                        }
                    }
                }
                // Anything else about a node changes it; what has no node was tallied by
                // `tally_other`.
                other => {
                    if let Some(node) = other.node()
                        && !added.contains(node)
                        && !removed.contains(node)
                    {
                        changed.insert(node);
                    }
                }
            }
        }
        outline.nodes_added = count(added.len());
        outline.nodes_changed = count(changed.len());
        outline.nodes_removed = count(removed.len());
        outline
    }
}

/// The lifecycle fact `mutation` makes, if it makes one.
fn lifecycle_fact(mutation: &Mutation) -> Option<OutlineFact> {
    Some(match mutation {
        Mutation::CreateJourney { .. } => OutlineFact::CreatesJourney,
        Mutation::EditJourney { .. } => OutlineFact::EditsJourney,
        Mutation::SetJourneyStatus { status } => OutlineFact::SetsJourneyStatus { status: *status },
        Mutation::CreateRoute { .. } => OutlineFact::CreatesRoute,
        Mutation::EditRoute { .. } => OutlineFact::EditsRoute,
        Mutation::SetRouteRetired { retired: true } => OutlineFact::RetiresRoute,
        Mutation::SetRouteRetired { retired: false } => OutlineFact::RestoresRoute,
        Mutation::OpenDraft { .. } => OutlineFact::OpensDraft,
        Mutation::DiscardDraft {} => OutlineFact::DiscardsDraft,
        Mutation::PublishDraft {} => OutlineFact::PublishesDraft,
        Mutation::Upgrade { to } => OutlineFact::Upgrades { to: *to },
        Mutation::Relink { .. } => OutlineFact::Relinks,
        Mutation::InsertSegment { .. } => OutlineFact::InsertsSegment,
        _ => return None,
    })
}

/// Tallies the mutations that name no node of their own (a lifecycle fact, a requirement, an
/// identity, a role or the like), and says whether `mutation` was one of them. The node mutations
/// are left to the caller, which tallies them against each other.
fn tally_other(outline: &mut ProposalOutline, mutation: &Mutation) -> bool {
    if let Some(next) = lifecycle_fact(mutation) {
        fact(outline, next);
        return true;
    }
    match mutation {
        Mutation::AddEdge { .. } => outline.requirements_added += 1,
        Mutation::RemoveEdge { .. } => outline.requirements_removed += 1,
        Mutation::CreateIdentity { .. } => outline.identities_created += 1,
        Mutation::EditIdentity { .. } => outline.identities_edited += 1,
        Mutation::MergeIdentities { merged, .. } => {
            outline.identities_merged += u32::try_from(merged.len()).unwrap_or(u32::MAX);
        }
        Mutation::DeleteIdentity { .. } => outline.identities_deleted += 1,
        Mutation::DeleteJourney { .. } => outline.journeys_deleted += 1,
        Mutation::DeleteRoute { .. } => outline.routes_deleted += 1,
        Mutation::AddRole { .. }
        | Mutation::EditRole { .. }
        | Mutation::RemoveRole { .. }
        | Mutation::AddParticipationKind { .. }
        | Mutation::EditParticipationKind { .. }
        | Mutation::RemoveParticipationKind { .. }
        | Mutation::SetDefaultOwner { .. }
        | Mutation::FillRole { .. }
        | Mutation::ClearRoleFill { .. }
        | Mutation::AddAnnotation { .. }
        | Mutation::EditAnnotation { .. }
        | Mutation::RemoveAnnotation { .. } => outline.other += 1,
        // A proposal's own content never edits a proposal.
        Mutation::CreateProposal { .. }
        | Mutation::EditProposal { .. }
        | Mutation::ApplyProposal { .. }
        | Mutation::DiscardProposal {} => {}
        // `lifecycle_fact` took these above.
        Mutation::CreateJourney { .. }
        | Mutation::EditJourney { .. }
        | Mutation::SetJourneyStatus { .. }
        | Mutation::CreateRoute { .. }
        | Mutation::EditRoute { .. }
        | Mutation::SetRouteRetired { .. }
        | Mutation::OpenDraft { .. }
        | Mutation::DiscardDraft {}
        | Mutation::PublishDraft {}
        | Mutation::Upgrade { .. }
        | Mutation::Relink { .. }
        | Mutation::InsertSegment { .. } => {
            unreachable!("a lifecycle mutation makes a lifecycle fact")
        }
        // The rest act on a node, which the caller tallies against the others.
        Mutation::AddNode { .. }
        | Mutation::SetNodeField { .. }
        | Mutation::ReplaceNode { .. }
        | Mutation::RemoveNode { .. }
        | Mutation::SetParticipation { .. }
        | Mutation::ClearParticipation { .. }
        | Mutation::AddResource { .. }
        | Mutation::EditResource { .. }
        | Mutation::RemoveResource { .. }
        | Mutation::Transition { .. }
        | Mutation::Answer { .. }
        | Mutation::SetRecordedDate { .. }
        | Mutation::SetPin { .. }
        | Mutation::ShiftPin { .. }
        | Mutation::ClearPin { .. }
        | Mutation::Snooze { .. }
        | Mutation::Unsnooze { .. }
        | Mutation::ApplyOverride { .. }
        | Mutation::RemoveOverride { .. }
        | Mutation::SetAtomic { .. }
        | Mutation::SetProvenance { .. }
        | Mutation::MarkLocalEdit { .. } => return false,
    }
    true
}

fn count(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// Records `next` once, in the order the mutations make their facts.
fn fact(outline: &mut ProposalOutline, next: OutlineFact) {
    if !outline.facts.contains(&next) {
        outline.facts.push(next);
    }
}

// PRD: C14.
/// What a destination is once a proposal applies, as the engine works it out from the
/// candidate records: what the views draw in place of the destination.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "destination", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum DestinationAfter {
    /// A journey as it would be, with the derive inputs its views are derived with.
    Journey {
        /// The document after.
        document: DomainDocument,
    },
    /// A route as it would be: its header, its versions, its draft; and the latest version
    /// when the candidate holds its graph (one the proposal publishes).
    Route {
        /// The route after.
        route: Route,
        /// The latest version after, when the candidate holds it.
        #[serde(skip_serializing_if = "Option::is_none")]
        latest: Option<RouteVersion>,
    },
    /// The deployment as it would be.
    Deployment {
        /// The deployment after.
        deployment: Deployment,
    },
    /// The destination does not exist after the candidate: a proposal to create it that
    /// drafts nothing creating it yet, with nothing to draw.
    Absent,
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

impl ProposalDraft {
    /// A proposal's content, checked: none of its mutations edits a proposal.
    ///
    /// # Errors
    ///
    /// [`NestedProposalError`] when one does.
    pub fn new(
        title: Title,
        description: Option<Markdown>,
        destination_revision: Revision,
        mutations: BoundedVec<Mutation, MutationCountPerPatch>,
        items: BoundedVec<ReviewItem, ByDocumentSize>,
    ) -> Result<Self, NestedProposalError> {
        if mutations.as_slice().iter().any(Mutation::edits_a_proposal) {
            return Err(NestedProposalError);
        }
        Ok(Self {
            title,
            description,
            destination_revision,
            mutations,
            items,
        })
    }
}

impl TryFrom<ProposalDraftWire> for ProposalDraft {
    type Error = NestedProposalError;

    fn try_from(wire: ProposalDraftWire) -> Result<Self, NestedProposalError> {
        Self::new(
            wire.title,
            wire.description,
            wire.destination_revision,
            wire.mutations,
            wire.items,
        )
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

// ARCHITECTURE: Schema outline: `proposals`.
/// A proposal as stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    // PRD: I6.
    /// The client-generated id.
    pub id: ProposalId,
    /// The domain it applies to, which may not exist yet.
    pub destination: Domain,
    // PRD: H5.
    /// Its editing revision, separate from the destination's.
    pub revision: Revision,
    /// Its status.
    pub status: ProposalStatus,
    /// Its content.
    pub draft: ProposalDraft,
    // PRD: I6.
    /// The agent that drafted it, if any.
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
