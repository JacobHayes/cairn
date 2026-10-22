//! The patch, the only write primitive (A17), and its closed set of mutations. A patch
//! targets one domain, or a proposal within one, names the base revision it was drafted
//! against, and carries mutations that apply in order. Scenario fixtures and the API write
//! them the same way:
//!
//! ```yaml
//! id: p_answer-scope
//! target: {journey: j_eval}
//! base_revision: 4
//! mutations:
//! - op: answer
//!   decision: n_scope
//!   value: {single_choice: narrow}
//! - op: transition
//!   node: n_plan
//!   transition: start
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::{AnnotationBody, Resource};
use crate::collections::{BoundedVec, CollectionError, MutationCountPerPatch};
use crate::domain::{Domain, Identity, JourneyStatus, Lineage, RouteKind};
use crate::field::{NodeField, NodeFieldValue};
use crate::graph::{Edge, ParticipationKind, Role};
use crate::id::{
    AttachmentKey, IdentityKey, InsertionKey, JourneyId, KindKey, NodeKey, PatchId, ProposalId,
    RoleKey, RouteId, Slug,
};
use crate::node::{IdentitySet, Node, ParticipationSource};
use crate::number::{Revision, SignedDays, VersionNumber};
use crate::proposal::ProposalDraft;
use crate::refs::KeyRefs;
use crate::state::{AnswerValue, Guard, LocalEdit, OverrideKind, Provenance, SnoozeTarget};
use crate::text::{Markdown, Reason, Title};
use jiff::civil::Date;

// PRD: A17.
/// What a patch targets: a domain, or a proposal within its destination domain.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchTarget {
    /// A journey.
    Journey(JourneyId),
    /// A route, including its draft.
    Route(RouteId),
    /// The deployment.
    Deployment,
    // PRD: I6.
    /// A proposal, by id, within its destination domain.
    Proposal {
        /// The proposal.
        id: ProposalId,
        /// Its destination domain.
        destination: Domain,
    },
}

impl PatchTarget {
    /// The domain the patch belongs to: the target domain, or a proposal's destination.
    #[must_use]
    pub fn domain(&self) -> Domain {
        match self {
            PatchTarget::Journey(id) => Domain::Journey(id.clone()),
            PatchTarget::Route(id) => Domain::Route(id.clone()),
            PatchTarget::Deployment => Domain::Deployment,
            PatchTarget::Proposal { destination, .. } => destination.clone(),
        }
    }
}

/// A journey's or route's mutations in one patch: at least one, at most
/// `mutation_count_per_patch_max`, in the order they apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    try_from = "BoundedVec<Mutation, MutationCountPerPatch>",
    into = "BoundedVec<Mutation, MutationCountPerPatch>"
)]
#[schemars(with = "BoundedVec<Mutation, MutationCountPerPatch>")]
pub struct Mutations(BoundedVec<Mutation, MutationCountPerPatch>);

impl Mutations {
    /// Checks the count.
    ///
    /// # Errors
    ///
    /// When there are none or too many.
    pub fn new(mutations: Vec<Mutation>) -> Result<Self, CollectionError> {
        Self::try_from(BoundedVec::new(mutations)?)
    }

    /// The mutations, in order.
    #[must_use]
    pub fn as_slice(&self) -> &[Mutation] {
        self.0.as_slice()
    }

    /// The number of mutations: the number of events an accepted patch emits (J2).
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false: a patch has at least one mutation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<BoundedVec<Mutation, MutationCountPerPatch>> for Mutations {
    type Error = CollectionError;

    fn try_from(
        mutations: BoundedVec<Mutation, MutationCountPerPatch>,
    ) -> Result<Self, CollectionError> {
        if mutations.is_empty() {
            return Err(CollectionError::Empty);
        }
        Ok(Self(mutations))
    }
}

impl From<Mutations> for BoundedVec<Mutation, MutationCountPerPatch> {
    fn from(mutations: Mutations) -> Self {
        mutations.0
    }
}

// PRD: A17, H5.
/// A patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    // PRD: H5.
    /// The client-generated id that makes a resubmission safe.
    pub id: PatchId,
    /// The domain or proposal it targets.
    pub target: PatchTarget,
    // PRD: A17.
    /// The target's revision the patch was drafted against; 0 creates the target.
    pub base_revision: Revision,
    // PRD: E6, H5.
    /// The deployment revision a journey patch that writes an identity reference was validated
    /// against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_revision: Option<Revision>,
    /// The mutations, in order.
    pub mutations: Mutations,
}

// PRD: A11, A13, B8.
/// Where a route draft comes from.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftSource {
    /// Editing the latest published version (or an empty route).
    Edit,
    /// Importing a route file; the import's content follows as ordinary mutations.
    Import,
    // PRD: B8.
    /// Saving a journey as a route; its structure follows as ordinary mutations.
    SaveAsRoute {
        /// The journey saved.
        journey: JourneyId,
    },
}

// PRD: B13.
/// What a segment role becomes in the graph it is inserted into.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RoleChoice {
    /// A role the graph has.
    Existing(RoleKey),
    /// A new role, minted from the insertion.
    Add,
}

// PRD: B13.
/// What a segment participation kind becomes in the graph it is inserted into.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum KindChoice {
    /// A kind the graph has.
    Existing(KindKey),
    /// A new kind, minted from the insertion.
    Add,
}

/// One end of an edge an insertion adds: a node of the segment, or of the graph.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EdgeEnd {
    /// A node by its key in the segment version.
    Segment(NodeKey),
    /// A node by its key in the graph.
    Host(NodeKey),
}

// PRD: B13.
/// An edge an insertion adds: `node` requires `requires`, one end in the segment and the
/// other in the graph.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct InsertedEdge {
    /// The dependent.
    pub node: EdgeEnd,
    /// What it requires.
    pub requires: EdgeEnd,
}

// PRD: D1.
/// A state-machine transition. Answering a decision is [`Mutation::Answer`]; the dates a
/// start or finish records default to today and are edited with
/// [`Mutation::SetRecordedDate`].
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Transition {
    /// `todo` to `active` (deliverable, action).
    Start,
    /// `active` to `todo`.
    Stop,
    /// To `done` (deliverable, action, milestone).
    Complete,
    // PRD: D1.
    /// To `skipped`, with the reason a skip requires.
    Skip {
        /// Why.
        reason: Reason,
    },
    // PRD: D1.
    /// Back to the initial state (always allowed).
    Reopen,
}

// PRD: F1, F2.
/// Which recorded date a [`Mutation::SetRecordedDate`] edits.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RecordedEnd {
    /// The start date of started work.
    Start,
    /// The finish date: a milestone's actual date, or when work was done or decided.
    Finish,
}

/// An override to apply.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Override {
    /// Treat the node as relevant.
    ForceInclude {
        /// Why.
        reason: Reason,
    },
    /// Exempt the node's subtree from an ancestor's skip.
    Keep {
        /// Why.
        reason: Reason,
    },
    /// Bypass guards on the node's transition in the same patch; the engine records the
    /// specific failures present.
    GuardBypass {
        /// The guards bypassed.
        guards: BTreeSet<Guard>,
        /// Why.
        reason: Reason,
    },
}

impl Override {
    /// The override's kind.
    #[must_use]
    pub fn kind(&self) -> OverrideKind {
        match self {
            Override::ForceInclude { .. } => OverrideKind::ForceInclude,
            Override::Keep { .. } => OverrideKind::Keep,
            Override::GuardBypass { .. } => OverrideKind::GuardBypass,
        }
    }
}

/// One participation, by node and kind.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ParticipationRef {
    /// The node.
    pub node: NodeKey,
    /// The kind.
    pub kind: KindKey,
}

// PRD: A18.
/// Everything a node removal removes, as its author saw it: the subtree, every edge
/// incident to it, and everything attached to those nodes. If anything else is attached
/// when the patch applies, the patch is rejected rather than widening the removal.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Removal {
    /// The node removed.
    pub node: NodeKey,
    /// Its descendants.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub descendants: BTreeSet<NodeKey>,
    /// Every explicit edge into or out of the node or a descendant.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub edges: BTreeSet<Edge>,
    /// Their resources.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub resources: BTreeSet<AttachmentKey>,
    /// Their notes and links, artifacts included.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub annotations: BTreeSet<AttachmentKey>,
    /// Their participations.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub participations: BTreeSet<ParticipationRef>,
}

impl Removal {
    /// The node and its descendants.
    pub fn nodes(&self) -> impl Iterator<Item = &NodeKey> {
        std::iter::once(&self.node).chain(&self.descendants)
    }
}

// PRD: J2.
// PRD: glossary, Mutation.
// PRD: glossary, Structural change.
/// One change within a patch, each emitting one event. The
/// set covers every change a patch can make; [`Mutation::change_class`] sorts each into
/// structural or state.
//
// A mutation without arguments is an empty struct variant, not a unit one: serde lets a unit
// variant of an internally tagged enum ignore unknown fields, so `{op: discard_draft,
// route: r_other}` would parse as a discard of the target's draft. A struct variant rejects
// them; the wire form `{op: discard_draft}` is the same either way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    // Journey lifecycle (A17, B1, B11, A19).
    // PRD: B1.
    /// Create the target journey, empty or from a route version.
    CreateJourney {
        /// The name.
        name: Title,
        /// The description.
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<Markdown>,
        /// The route version to copy, or none for an empty journey.
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<Lineage>,
    },
    /// Rename or redescribe the journey.
    EditJourney {
        /// The name.
        name: Title,
        /// The description.
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<Markdown>,
    },
    // PRD: B11.
    /// Change the journey's status.
    SetJourneyStatus {
        /// The new status.
        status: JourneyStatus,
    },

    // Route lifecycle (A11, A13, A19).
    /// Create the target route, with no versions and no draft.
    CreateRoute {
        /// The name.
        name: Title,
        /// The description.
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<Markdown>,
        // PRD: A21.
        /// Process (the default) or segment; fixed from here on.
        #[serde(default, skip_serializing_if = "RouteKind::is_process")]
        kind: RouteKind,
    },
    /// Rename or redescribe the route.
    EditRoute {
        /// The name.
        name: Title,
        /// The description.
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<Markdown>,
    },
    /// Retire the route (hide it from new-journey creation) or bring it back.
    SetRouteRetired {
        /// Retired or not.
        retired: bool,
    },
    // PRD: A11.
    /// Open the route's draft.
    OpenDraft {
        /// Where it comes from.
        source: DraftSource,
    },
    /// Discard the route's draft.
    DiscardDraft {},
    // PRD: A11.
    /// Publish the draft as the next version and clear it.
    PublishDraft {},

    // Graph structure (A1, B4, A18).
    /// Add a node.
    AddNode {
        /// The node, with its key.
        node: Node<KeyRefs>,
    },
    /// Change one field of a node.
    SetNodeField {
        /// The node.
        node: NodeKey,
        /// The field and its new value.
        value: NodeFieldValue<KeyRefs>,
    },
    // PRD: B7.
    /// Replace a node whole, as a kind or answer-type change does.
    ReplaceNode {
        /// The node, with its key.
        node: Node<KeyRefs>,
    },
    // PRD: A18.
    /// Remove a node and everything its removal names.
    RemoveNode {
        /// What it removes.
        removal: Removal,
    },
    /// Add an explicit edge.
    AddEdge {
        /// The edge.
        edge: Edge,
    },
    /// Remove an explicit edge.
    RemoveEdge {
        /// The edge.
        edge: Edge,
    },
    // PRD: A6.
    /// Add a role.
    AddRole {
        /// The role.
        role: Role<KeyRefs>,
    },
    /// Change a role's id, title, or cardinality.
    EditRole {
        /// The role as it should be.
        role: Role<KeyRefs>,
    },
    // PRD: A18.
    /// Remove a role no participation or `fills_role` references.
    RemoveRole {
        /// The role.
        role: RoleKey,
    },
    // PRD: A7.
    /// Declare a participation kind.
    AddParticipationKind {
        /// The kind.
        kind: ParticipationKind<KeyRefs>,
    },
    /// Change a participation kind's id, title, or cardinality.
    EditParticipationKind {
        /// The kind as it should be.
        kind: ParticipationKind<KeyRefs>,
    },
    /// Remove a participation kind nothing references.
    RemoveParticipationKind {
        /// The kind.
        kind: KindKey,
    },
    // PRD: A6.
    /// Set or clear the graph's `default_owner` role.
    SetDefaultOwner {
        /// The role, or none.
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<RoleKey>,
    },
    // PRD: E2, B5.
    /// Set a node's participation of one kind.
    SetParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
        /// Its source.
        source: ParticipationSource<KeyRefs>,
    },
    // PRD: E2.
    /// Remove a node's participation of one kind, restoring inheritance.
    ClearParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    // PRD: A10.
    /// Add a resource to a node.
    AddResource {
        /// The node.
        node: NodeKey,
        /// The resource, with its key.
        resource: Resource<KeyRefs>,
    },
    /// Replace a resource.
    EditResource {
        /// The node.
        node: NodeKey,
        /// The resource as it should be.
        resource: Resource<KeyRefs>,
    },
    /// Remove a resource.
    RemoveResource {
        /// The node.
        node: NodeKey,
        /// The resource.
        resource: AttachmentKey,
    },
    // PRD: B13.
    /// Insert a published segment version under a parent, wired to the graph's own nodes and
    /// with its roles and kinds mapped, in one event.
    InsertSegment {
        /// The insertion's key, new in the graph.
        insertion: InsertionKey,
        /// The segment and one of its published versions.
        segment: Lineage,
        /// The group, deliverable, or action the root goes under; none for the top level.
        #[serde(skip_serializing_if = "Option::is_none")]
        parent: Option<NodeKey>,
        /// The root's id; the segment root's own, suffixed when a sibling has it, if none.
        #[serde(skip_serializing_if = "Option::is_none")]
        root_id: Option<Slug>,
        /// The root's title; the segment root's own if none.
        #[serde(skip_serializing_if = "Option::is_none")]
        root_title: Option<Title>,
        /// What each segment role becomes; a role not named takes the graph's role with its
        /// id and cardinality, else is added.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        roles: BTreeMap<RoleKey, RoleChoice>,
        /// What each segment participation kind becomes, likewise.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        kinds: BTreeMap<KindKey, KindChoice>,
        /// Segment nodes left out, each with its subtree.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        omit: BTreeSet<NodeKey>,
        /// The wiring: each edge joins one segment node and one node of the graph.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        edges: Vec<InsertedEdge>,
    },

    // Journey state (B2, B5, B6, B10, D1, F1, F5, G1).
    /// A state-machine transition.
    Transition {
        /// The node.
        node: NodeKey,
        /// The transition.
        transition: Transition,
    },
    // PRD: B2.
    /// Answer or revise a decision. The rationale belongs to this answer alone: a
    /// revision that gives none leaves the decision without one.
    Answer {
        /// The decision.
        decision: NodeKey,
        /// The answer.
        value: AnswerValue,
        // PRD: B2.
        /// Why it was given: markdown, free text under the body limit.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rationale: Option<Markdown>,
    },
    // PRD: F1, F2.
    /// Edit a recorded start or finish date.
    SetRecordedDate {
        /// The node.
        node: NodeKey,
        /// Which date.
        end: RecordedEnd,
        /// The date.
        date: Date,
    },
    // PRD: E3.
    /// Fill a role without a filling decision.
    FillRole {
        /// The role.
        role: RoleKey,
        /// The identities.
        identities: IdentitySet,
    },
    /// Empty a directly filled role.
    ClearRoleFill {
        /// The role.
        role: RoleKey,
    },
    // PRD: F5.
    /// Pin a node's date (`repin`).
    SetPin {
        /// The node.
        node: NodeKey,
        /// The date.
        date: Date,
    },
    // PRD: F5.
    /// Move a pin by a number of days (`shift`).
    ShiftPin {
        /// The node.
        node: NodeKey,
        /// Days later (positive) or earlier (negative).
        offset_days: SignedDays,
    },
    // PRD: F5.
    /// Remove a pin (`unpin`).
    ClearPin {
        /// The node.
        node: NodeKey,
    },
    // PRD: B6.
    /// Snooze an actionable node, or a container (a group, or work with children) that has
    /// work left beneath it, which holds every descendant off the acting frontier.
    Snooze {
        /// The node.
        node: NodeKey,
        /// Until a date or a node.
        until: SnoozeTarget,
    },
    // PRD: B6.
    /// Lift a snooze. A node held only through a container's snooze is refused, naming
    /// the container to unsnooze instead.
    Unsnooze {
        /// The node.
        node: NodeKey,
    },
    // PRD: B5, D1a, D4.
    /// Apply an override.
    ApplyOverride {
        /// The node.
        node: NodeKey,
        /// The override.
        #[serde(rename = "override")]
        applied: Override,
    },
    /// Remove an override.
    RemoveOverride {
        /// The node.
        node: NodeKey,
        /// Its kind.
        kind: OverrideKind,
    },
    // PRD: B10.
    /// Mark a placeholder atomic, or not.
    SetAtomic {
        /// The node.
        node: NodeKey,
        /// Atomic or not.
        atomic: bool,
    },
    // PRD: G1.
    /// Add a note or link.
    AddAnnotation {
        /// What its author wrote.
        annotation: AnnotationBody,
    },
    /// Edit a note or link.
    EditAnnotation {
        /// What it should say.
        annotation: AnnotationBody,
    },
    /// Remove a note or link.
    RemoveAnnotation {
        /// The annotation.
        annotation: AttachmentKey,
    },

    // Lineage (B4, B7, B9).
    // PRD: B7.
    /// Move the journey to a newer version of its route, taking every clean outcome of the
    /// three-way merge (route changes it did not edit, new nodes, orphans); conflict
    /// resolutions and orphan removals ride after it in the same patch as ordinary mutations.
    Upgrade {
        /// The version upgraded to.
        to: VersionNumber,
    },
    // PRD: B9.
    /// Link the journey to the version its saved route published, in one event: the
    /// lineage, each node the version holds route-copied and marked exactly where it differs,
    /// every other node local, and the version's roles and kinds the journey lacks.
    Relink {
        /// The new lineage.
        lineage: Lineage,
    },
    // PRD: B7.
    /// Change a node's provenance, as an upgrade orphaning it does.
    SetProvenance {
        /// The node.
        node: NodeKey,
        /// The provenance.
        provenance: Provenance,
    },
    // PRD: B4.
    /// Set or clear a local-edit marker (reset to route clears it).
    MarkLocalEdit {
        /// The node.
        node: NodeKey,
        /// The edited aspect.
        edit: LocalEdit,
        /// Marked or cleared.
        marked: bool,
    },

    // Identities (E6, H3).
    /// Create an identity; may ride in any patch.
    CreateIdentity {
        /// The identity, with its key.
        identity: Identity,
    },
    /// Change an identity's name or emails (deployment).
    EditIdentity {
        /// The identity as it should be.
        identity: Identity,
    },
    // PRD: E6.
    /// Merge identities into one that remains (deployment): every merged identity's emails
    /// move to the survivor and each merged key becomes an alias of it, all checked against one
    /// snapshot.
    MergeIdentities {
        /// The identity that remains.
        survivor: IdentityKey,
        /// The identities merged into it, whose keys become aliases; at least one, at most
        /// `identity_count_per_fill_max`.
        merged: IdentitySet,
        /// Every journey referencing the survivor or any merged identity, at the revision the
        /// merge was checked against; at most `journey_count_per_merge_max`.
        #[serde(deserialize_with = "merge_journeys")]
        #[schemars(extend("maxProperties" = crate::limits::JOURNEY_COUNT_PER_MERGE_MAX))]
        journeys: BTreeMap<JourneyId, Revision>,
    },
    // PRD: E6.
    /// Delete an identity nothing refers to (deployment): the identity, its emails, and
    /// every alias resolving to it. Rejected while any journey, route draft, or route version
    /// refers to it or to one of its aliases; the commit rechecks that none does.
    DeleteIdentity {
        /// The identity.
        identity: IdentityKey,
    },

    // Hard deletion (A19), in the deployment's patch: one patch deletes every journey and
    // route it names, or none.
    // PRD: A19.
    /// Delete a journey and its events for good (deployment): the deployment log keeps who
    /// deleted it and when, and its id is never used again. Rejected when the journey is not
    /// at `revision`; the commit rechecks it.
    DeleteJourney {
        /// The journey.
        journey: JourneyId,
        /// The journey's revision, as read.
        revision: Revision,
    },
    // PRD: A19.
    /// Delete a route or segment with its draft, every version, and its events, for good
    /// (deployment): only while no journey's lineage and no insertion names any of its
    /// versions. Its id is never used again. Rejected when the route is not at `revision`; the
    /// commit rechecks the revision and that nothing has started using it.
    DeleteRoute {
        /// The route.
        route: RouteId,
        /// The route's revision, as read.
        revision: Revision,
    },

    // Proposals (I6).
    /// Create the target proposal.
    CreateProposal {
        /// Its content.
        proposal: ProposalDraft,
    },
    /// Replace the target proposal's content.
    EditProposal {
        /// Its new content.
        proposal: ProposalDraft,
    },
    /// Apply a proposal to its destination: its mutations follow its own revision check.
    ApplyProposal {
        /// The proposal.
        proposal: ProposalId,
        /// The proposal revision the user reviewed.
        reviewed_revision: Revision,
    },
    /// Discard the target proposal.
    DiscardProposal {},
}

/// A merge's journeys as written: each once, at most `journey_count_per_merge_max`.
fn merge_journeys<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<JourneyId, Revision>, D::Error> {
    crate::serde_util::unique_entries(
        deserializer,
        "a map from journey to its revision, each journey once",
        Some(crate::limits::Limit::JourneyCountPerMerge),
    )
}

// PRD: glossary, Structural change.
/// Structural or state.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClass {
    /// A change to what the graph is.
    Structural,
    /// A change to what has happened in it.
    State,
}

impl ChangeClass {
    /// What a change of this class is, in words: what the assistant tells its model about
    /// which of its writes apply directly.
    #[must_use]
    pub const fn meaning(self) -> &'static str {
        match self {
            ChangeClass::Structural => {
                "a change to what the graph is (its nodes, edges, roles, conditions, date \
                 rules, resources), or any change to a route or its draft"
            }
            ChangeClass::State => {
                "a change to what has happened in a journey (transitions, answers, \
                 participations, pins, dates, snoozes, notes, overrides)"
            }
        }
    }
}

impl Mutation {
    /// The node this mutation adds, changes, or removes (the decision, for an answer); none
    /// for a mutation about an edge, a role, a kind, an annotation, or its domain.
    #[must_use]
    pub fn node(&self) -> Option<&NodeKey> {
        match self {
            Mutation::AddNode { node } | Mutation::ReplaceNode { node } => Some(&node.key),
            Mutation::RemoveNode { removal } => Some(&removal.node),
            Mutation::SetNodeField { node, .. }
            | Mutation::SetParticipation { node, .. }
            | Mutation::ClearParticipation { node, .. }
            | Mutation::AddResource { node, .. }
            | Mutation::EditResource { node, .. }
            | Mutation::RemoveResource { node, .. }
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
            | Mutation::MarkLocalEdit { node, .. }
            | Mutation::Answer { decision: node, .. } => Some(node),
            Mutation::CreateJourney { .. }
            | Mutation::EditJourney { .. }
            | Mutation::SetJourneyStatus { .. }
            | Mutation::CreateRoute { .. }
            | Mutation::EditRoute { .. }
            | Mutation::SetRouteRetired { .. }
            | Mutation::OpenDraft { .. }
            | Mutation::DiscardDraft {}
            | Mutation::PublishDraft {}
            | Mutation::AddEdge { .. }
            | Mutation::RemoveEdge { .. }
            | Mutation::AddRole { .. }
            | Mutation::EditRole { .. }
            | Mutation::RemoveRole { .. }
            | Mutation::AddParticipationKind { .. }
            | Mutation::EditParticipationKind { .. }
            | Mutation::RemoveParticipationKind { .. }
            | Mutation::SetDefaultOwner { .. }
            | Mutation::InsertSegment { .. }
            | Mutation::FillRole { .. }
            | Mutation::ClearRoleFill { .. }
            | Mutation::AddAnnotation { .. }
            | Mutation::EditAnnotation { .. }
            | Mutation::RemoveAnnotation { .. }
            | Mutation::Upgrade { .. }
            | Mutation::Relink { .. }
            | Mutation::CreateIdentity { .. }
            | Mutation::EditIdentity { .. }
            | Mutation::MergeIdentities { .. }
            | Mutation::DeleteIdentity { .. }
            | Mutation::DeleteJourney { .. }
            | Mutation::DeleteRoute { .. }
            | Mutation::CreateProposal { .. }
            | Mutation::EditProposal { .. }
            | Mutation::ApplyProposal { .. }
            | Mutation::DiscardProposal {} => None,
        }
    }

    /// PRD glossary, Structural change: what this mutation is in a journey or the
    /// deployment. Every change to a route is structural ([`Patch::change_class`]).
    #[must_use]
    pub fn change_class(&self) -> ChangeClass {
        match self {
            Mutation::CreateJourney { .. }
            | Mutation::DeleteJourney { .. }
            | Mutation::DeleteRoute { .. }
            | Mutation::CreateRoute { .. }
            | Mutation::EditRoute { .. }
            | Mutation::SetRouteRetired { .. }
            | Mutation::OpenDraft { .. }
            | Mutation::DiscardDraft {}
            | Mutation::PublishDraft {}
            | Mutation::AddNode { .. }
            | Mutation::ReplaceNode { .. }
            | Mutation::RemoveNode { .. }
            | Mutation::AddEdge { .. }
            | Mutation::RemoveEdge { .. }
            | Mutation::AddRole { .. }
            | Mutation::EditRole { .. }
            | Mutation::RemoveRole { .. }
            | Mutation::AddParticipationKind { .. }
            | Mutation::EditParticipationKind { .. }
            | Mutation::RemoveParticipationKind { .. }
            | Mutation::SetDefaultOwner { .. }
            | Mutation::AddResource { .. }
            | Mutation::EditResource { .. }
            | Mutation::RemoveResource { .. }
            | Mutation::InsertSegment { .. }
            | Mutation::Upgrade { .. }
            | Mutation::Relink { .. }
            | Mutation::SetProvenance { .. }
            | Mutation::MarkLocalEdit { .. }
            | Mutation::MergeIdentities { .. }
            | Mutation::DeleteIdentity { .. }
            | Mutation::ApplyProposal { .. } => ChangeClass::Structural,
            Mutation::SetNodeField { value, .. } => match value.field() {
                // A journey weight override is state (B5); every other field is structure.
                NodeField::Weight => ChangeClass::State,
                _ => ChangeClass::Structural,
            },
            Mutation::EditJourney { .. }
            | Mutation::SetJourneyStatus { .. }
            | Mutation::SetParticipation { .. }
            | Mutation::ClearParticipation { .. }
            | Mutation::Transition { .. }
            | Mutation::Answer { .. }
            | Mutation::SetRecordedDate { .. }
            | Mutation::FillRole { .. }
            | Mutation::ClearRoleFill { .. }
            | Mutation::SetPin { .. }
            | Mutation::ShiftPin { .. }
            | Mutation::ClearPin { .. }
            | Mutation::Snooze { .. }
            | Mutation::Unsnooze { .. }
            | Mutation::ApplyOverride { .. }
            | Mutation::RemoveOverride { .. }
            | Mutation::SetAtomic { .. }
            | Mutation::AddAnnotation { .. }
            | Mutation::EditAnnotation { .. }
            | Mutation::RemoveAnnotation { .. }
            | Mutation::CreateIdentity { .. }
            | Mutation::EditIdentity { .. }
            | Mutation::CreateProposal { .. }
            | Mutation::EditProposal { .. }
            | Mutation::DiscardProposal {} => ChangeClass::State,
        }
    }

    /// True for the mutations that edit a proposal rather than a domain; a proposal's own
    /// content never holds them.
    #[must_use]
    pub fn edits_a_proposal(&self) -> bool {
        matches!(
            self,
            Mutation::CreateProposal { .. }
                | Mutation::EditProposal { .. }
                | Mutation::ApplyProposal { .. }
                | Mutation::DiscardProposal {}
        )
    }
}

impl Patch {
    /// PRD glossary, Structural change: a patch is structural when any of its mutations is,
    /// and every patch to a route is structural (a route has no state).
    #[must_use]
    pub fn change_class(&self) -> ChangeClass {
        if matches!(self.target.domain(), Domain::Route(_)) {
            return ChangeClass::Structural;
        }
        let structural = self
            .mutations
            .as_slice()
            .iter()
            .any(|mutation| mutation.change_class() == ChangeClass::Structural);
        if structural {
            ChangeClass::Structural
        } else {
            ChangeClass::State
        }
    }
}

impl fmt::Display for ChangeClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            ChangeClass::Structural => "structural",
            ChangeClass::State => "state",
        })
    }
}
