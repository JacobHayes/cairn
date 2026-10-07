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
use crate::domain::{Domain, Entity, JourneyStatus, Lineage};
use crate::field::{NodeField, NodeFieldValue};
use crate::graph::{Edge, ParticipationKind, Role};
use crate::id::{
    AttachmentKey, EntityKey, JourneyId, KindKey, NodeKey, PatchId, ProposalId, RoleKey, RouteId,
};
use crate::node::{EntitySet, Node, ParticipationSource};
use crate::number::{Revision, SignedDays, VersionNumber};
use crate::proposal::ProposalDraft;
use crate::refs::KeyRefs;
use crate::state::{AnswerValue, Guard, LocalEdit, OverrideKind, Provenance, SnoozeTarget};
use crate::text::{Markdown, Reason, Title};
use jiff::civil::Date;

/// What a patch targets (A17): a domain, or a proposal within its destination domain.
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
    /// A proposal, by id, within its destination domain (I6).
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

/// A patch (A17, H5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// The client-generated id that makes a resubmission safe (H5).
    pub id: PatchId,
    /// The domain or proposal it targets.
    pub target: PatchTarget,
    /// The target's revision the patch was drafted against; 0 creates the target (A17).
    pub base_revision: Revision,
    /// The deployment revision a journey patch that writes an entity reference was validated
    /// against (E6, H5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_revision: Option<Revision>,
    /// The mutations, in order.
    pub mutations: Mutations,
}

/// Where a route draft comes from (A11, A13, B8).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DraftSource {
    /// Editing the latest published version (or an empty route).
    Edit,
    /// Importing a route file; the import's content follows as ordinary mutations.
    Import,
    /// Saving a journey as a route (B8); its structure follows as ordinary mutations.
    SaveAsRoute {
        /// The journey saved.
        journey: JourneyId,
    },
}

/// A state-machine transition (D1). Answering a decision is [`Mutation::Answer`]; the dates a
/// start or reach records default to today and are edited with
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
    /// To `done` (deliverable, action).
    Complete,
    /// To `skipped`, with the reason D1 requires.
    Skip {
        /// Why.
        reason: Reason,
    },
    /// Back to the initial state (D1: always allowed).
    Reopen,
    /// `pending` to `reached` (milestone), including confirming an auto-reach (F1).
    Reach,
}

/// Which recorded date a [`Mutation::SetRecordedDate`] edits (F1, F2).
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

/// An override to apply (PRD glossary, Override; Gating; D1a; D4).
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

/// Everything a node removal removes, as its author saw it (A18): the subtree, every edge
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

/// One change within a patch (PRD glossary, Mutation), each emitting one event (J2). The
/// set covers every change the PRD defines; [`Mutation::change_class`] sorts each into
/// structural or state (PRD glossary, Structural change).
//
// A mutation without arguments is an empty struct variant, not a unit one: serde lets a unit
// variant of an internally tagged enum ignore unknown fields, so `{op: delete_journey,
// journey: j_other}` would parse as a delete of the target. A struct variant rejects them;
// the wire form `{op: delete_journey}` is the same either way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    // Journey lifecycle (A17, B1, B11, A19).
    /// Create the target journey, empty or from a route version (B1).
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
    /// Change the journey's status (B11).
    SetJourneyStatus {
        /// The new status.
        status: JourneyStatus,
    },
    /// Hard-delete the journey and its events (A19).
    DeleteJourney {},

    // Route lifecycle (A11, A13, A19).
    /// Create the target route, with no versions and no draft.
    CreateRoute {
        /// The name.
        name: Title,
        /// The description.
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<Markdown>,
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
    /// Open the route's draft (A11).
    OpenDraft {
        /// Where it comes from.
        source: DraftSource,
    },
    /// Discard the route's draft.
    DiscardDraft {},
    /// Publish the draft as the next version and clear it (A11).
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
    /// Replace a node whole, as a kind or answer-type change does (B7).
    ReplaceNode {
        /// The node, with its key.
        node: Node<KeyRefs>,
    },
    /// Remove a node and everything its removal names (A18).
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
    /// Add a role (A6).
    AddRole {
        /// The role.
        role: Role<KeyRefs>,
    },
    /// Change a role's id, title, or cardinality.
    EditRole {
        /// The role as it should be.
        role: Role<KeyRefs>,
    },
    /// Remove a role no participation or `fills_role` references (A18).
    RemoveRole {
        /// The role.
        role: RoleKey,
    },
    /// Declare a participation kind (A7).
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
    /// Set or clear the graph's `default_owner` role (A6).
    SetDefaultOwner {
        /// The role, or none.
        #[serde(skip_serializing_if = "Option::is_none")]
        role: Option<RoleKey>,
    },
    /// Set a node's participation of one kind (E2, B5).
    SetParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
        /// Its source.
        source: ParticipationSource<KeyRefs>,
    },
    /// Remove a node's participation of one kind, restoring inheritance (E2).
    ClearParticipation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    /// Add a resource to a node (A10).
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

    // Journey state (B2, B5, B6, B10, D1, F1, F5, G1).
    /// A state-machine transition.
    Transition {
        /// The node.
        node: NodeKey,
        /// The transition.
        transition: Transition,
    },
    /// Answer or revise a decision (B2).
    Answer {
        /// The decision.
        decision: NodeKey,
        /// The answer.
        value: AnswerValue,
    },
    /// Edit a recorded start or finish date (F1, F2).
    SetRecordedDate {
        /// The node.
        node: NodeKey,
        /// Which date.
        end: RecordedEnd,
        /// The date.
        date: Date,
    },
    /// Fill a role without a filling decision (E3).
    FillRole {
        /// The role.
        role: RoleKey,
        /// The entities.
        entities: EntitySet,
    },
    /// Empty a directly filled role.
    ClearRoleFill {
        /// The role.
        role: RoleKey,
    },
    /// Pin a node's date (PRD glossary, Pin; F5 `repin`).
    SetPin {
        /// The node.
        node: NodeKey,
        /// The date.
        date: Date,
    },
    /// Move a pin by a number of days (F5 `shift`).
    ShiftPin {
        /// The node.
        node: NodeKey,
        /// Days later (positive) or earlier (negative).
        offset_days: SignedDays,
    },
    /// Remove a pin (F5 `unpin`).
    ClearPin {
        /// The node.
        node: NodeKey,
    },
    /// Snooze an actionable node (B6).
    Snooze {
        /// The node.
        node: NodeKey,
        /// Until a date or a node.
        until: SnoozeTarget,
    },
    /// Lift a snooze (B6).
    Unsnooze {
        /// The node.
        node: NodeKey,
    },
    /// Apply an override (B5, D1a, D4).
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
    /// Mark a placeholder atomic, or not (B10).
    SetAtomic {
        /// The node.
        node: NodeKey,
        /// Atomic or not.
        atomic: bool,
    },
    /// Add a note or link (G1).
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
    /// Move the journey to a newer version of its route, taking every clean outcome of the
    /// three-way merge (route changes it did not edit, new nodes, orphans); conflict
    /// resolutions and orphan removals ride after it in the same patch as ordinary mutations
    /// (B7).
    Upgrade {
        /// The version upgraded to.
        to: VersionNumber,
    },
    /// Link the journey to the version its saved route published (B9), in one event: the
    /// lineage, each node the version holds route-copied and marked exactly where it differs,
    /// every other node local, and the version's roles and kinds the journey lacks.
    Relink {
        /// The new lineage.
        lineage: Lineage,
    },
    /// Change a node's provenance, as an upgrade orphaning it does (B7).
    SetProvenance {
        /// The node.
        node: NodeKey,
        /// The provenance.
        provenance: Provenance,
    },
    /// Set or clear a local-edit marker (B4: reset to route clears it).
    MarkLocalEdit {
        /// The node.
        node: NodeKey,
        /// The edited aspect.
        edit: LocalEdit,
        /// Marked or cleared.
        marked: bool,
    },

    // Entities (E6, H3).
    /// Create an entity; may ride in any patch.
    CreateEntity {
        /// The entity, with its key.
        entity: Entity,
    },
    /// Change an entity's name or emails (deployment).
    EditEntity {
        /// The entity as it should be.
        entity: Entity,
    },
    /// Merge one entity into another (deployment, E6).
    MergeEntities {
        /// The entity that remains.
        survivor: EntityKey,
        /// The entity merged into it, whose key becomes an alias.
        merged: EntityKey,
        /// Every journey referencing either, at the revision the merge was checked against.
        #[serde(deserialize_with = "crate::serde_util::unique_map")]
        journeys: BTreeMap<JourneyId, Revision>,
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

/// Structural or state (PRD glossary, Structural change).
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

impl Mutation {
    /// PRD glossary, Structural change: what this mutation is in a journey or the
    /// deployment. Every change to a route is structural ([`Patch::change_class`]).
    #[must_use]
    pub fn change_class(&self) -> ChangeClass {
        match self {
            Mutation::CreateJourney { .. }
            | Mutation::DeleteJourney {}
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
            | Mutation::Upgrade { .. }
            | Mutation::Relink { .. }
            | Mutation::SetProvenance { .. }
            | Mutation::MarkLocalEdit { .. }
            | Mutation::MergeEntities { .. }
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
            | Mutation::CreateEntity { .. }
            | Mutation::EditEntity { .. }
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
