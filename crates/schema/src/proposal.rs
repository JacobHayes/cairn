//! Proposals (I6, C14; ARCHITECTURE, Engine > Model): a patch drafted but not applied,
//! outside every graph, with its own editing revision and the review items a reviewer
//! resolves before apply (B8, A18).

use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, ByDocumentSize, MutationCountPerPatch};
use crate::derived::{Consequences, DomainDocument};
use crate::domain::{Deployment, Domain, JourneyStatus, Route, RouteVersion};
use crate::graph::Role;
use crate::id::{AgentId, IdentityKey, NodeKey, ProposalId, RoleKey, UserId};
use crate::notice::Notice;
use crate::number::Revision;
use crate::patch::{Mutation, ParticipationRef, Removal};
use crate::refs::KeyRefs;
use crate::rejection::Violation;

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
    /// Inserts a published route.
    InsertsRoute,
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
    /// Routes it deletes.
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
    /// to their node. A route insertion is a fact of its own: the nodes it adds
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
        Mutation::InsertRoute { .. } => OutlineFact::InsertsRoute,
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
        | Mutation::InsertRoute { .. } => {
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
        | Mutation::SetTicked { .. } => return false,
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
