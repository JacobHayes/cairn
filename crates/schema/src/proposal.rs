//! Proposals (I6, C14; ARCHITECTURE, Engine > Model): a patch drafted but not applied,
//! outside every graph, with its own editing revision and the review items a reviewer
//! resolves before apply (B7 to B9, A18).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, ByDocumentSize, MutationCountPerPatch};
use crate::domain::Domain;
use crate::field::{NodeField, NodeFieldValue};
use crate::graph::Role;
use crate::id::{AgentId, EntityKey, NodeKey, ProposalId, RoleKey, Slug, UserId};
use crate::number::Revision;
use crate::patch::{Mutation, Removal};
use crate::refs::KeyRefs;
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

/// How a reviewer resolves an upgrade conflict (B7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ConflictResolution {
    /// Keep the journey's value as a local edit.
    KeepJourney,
    /// Take the route's value.
    TakeRoute,
    /// Map a removed choice to one that remains.
    MapChoice {
        /// The removed choice.
        from: Slug,
        /// The choice it maps to.
        to: Slug,
    },
    /// Take the route's value and clear the state it invalidates.
    ClearState,
    /// Take the route's value and reopen the node.
    Reopen,
}

/// What becomes of an explicit entity's participation when a journey is saved as a route
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
    /// Make it the graph's `default_owner`.
    DefaultOwner,
}

/// One thing a reviewer looks at, and resolves where it needs a choice (C14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "item", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewItem {
    /// A field the journey edited and the route changed too (B7).
    Conflict {
        /// The node.
        node: NodeKey,
        /// The field.
        field: NodeField,
        /// The journey's value.
        journey: Option<NodeFieldValue<KeyRefs>>,
        /// The route's value.
        route: Option<NodeFieldValue<KeyRefs>>,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        resolution: Option<ConflictResolution>,
    },
    /// A field the journey edited and the route left alone: kept, listed (B7).
    KeptLocalEdit {
        /// The node.
        node: NodeKey,
        /// The field.
        field: NodeField,
    },
    /// A node the new version removed (B7): kept as orphaned by default.
    Orphan {
        /// The node.
        node: NodeKey,
        /// Keep it (the default) or remove it with its journey-local descendants.
        keep: bool,
    },
    /// An explicit entity on a node being saved as a route (B8).
    Participation {
        /// The entity.
        entity: EntityKey,
        /// The reviewer's choice, once made.
        #[serde(skip_serializing_if = "Option::is_none")]
        mapping: Option<ParticipationMapping>,
    },
    /// A node the reviewer may leave out of a saved route (B8) or a re-link (B9).
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
