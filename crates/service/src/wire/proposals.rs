//! Proposals' request and response bodies (I6, C14; ARCHITECTURE, HTTP API: Resources):
//! edited and discarded against their own editing revision, previewed, applied, and
//! refreshed (a proposal is made by a patch that names it); and route files imported (A13).
//! Every write names a client patch id, so a lost response is resubmitted rather than guessed
//! at (H5).

use std::collections::BTreeMap;

use cairn_schema::{
    AgentId, Consequences, DestinationAfter, Domain, JourneyId, JourneyStatus, Markdown, PatchId,
    PatchReceipt, Proposal, ProposalDraft, ProposalId, ProposalOutline, ProposalPreview, Revision,
    RevisionConflict, RouteFile, Timestamp, Title, TouchedSet, UserId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// PRD: H5.
/// `PATCH /api/proposals/{id}`: the proposal's new content, against the editing revision the
/// editor saw. Its destination's revision does not move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalEdit {
    // PRD: H5.
    /// The write's patch id.
    pub patch_id: PatchId,
    /// The proposal's editing revision the editor saw.
    pub base_revision: Revision,
    /// Its content.
    pub draft: ProposalDraft,
}

/// `POST /api/proposals/{id}/discard` and `POST /api/proposals/{id}/refresh`: against the editing
/// revision the caller saw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalStep {
    // PRD: H5.
    /// The write's patch id.
    pub patch_id: PatchId,
    /// The proposal's editing revision the caller saw.
    pub base_revision: Revision,
}

// PRD: H2.
/// `POST /api/proposals/{id}/apply`: applies the proposal as the caller, its confirming user,
/// at the editing revision they reviewed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalApply {
    // PRD: H5.
    /// The apply's patch id.
    pub patch_id: PatchId,
    /// The proposal's editing revision the reviewer previewed.
    pub reviewed_revision: Revision,
    // PRD: J1.
    /// A note for each of the apply's events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
}

// PRD: I6.
/// What a write to a proposal answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalAnswer {
    /// Committed now: the receipt (its revision is the proposal's editing revision) and the
    /// proposal as it now stands.
    Saved {
        /// The receipt.
        receipt: PatchReceipt,
        /// The proposal.
        proposal: Proposal,
    },
    // PRD: H5.
    /// The patch id was committed before with the same content: its receipt; fetch the
    /// proposal by id for where it stands now.
    AlreadySaved {
        /// The original receipt.
        receipt: PatchReceipt,
    },
    // PRD: I6.
    /// A create naming a proposal id its caller already holds for this destination: that
    /// proposal as it stands, not a second one.
    Existing {
        /// The proposal.
        proposal: Proposal,
    },
}

impl From<crate::ProposalWritten> for ProposalAnswer {
    fn from(written: crate::ProposalWritten) -> Self {
        match written {
            crate::ProposalWritten::Saved { receipt, proposal } => {
                ProposalAnswer::Saved { receipt, proposal }
            }
            crate::ProposalWritten::AlreadySaved { receipt } => {
                ProposalAnswer::AlreadySaved { receipt }
            }
            crate::ProposalWritten::Existing { proposal } => ProposalAnswer::Existing { proposal },
        }
    }
}

impl ProposalAnswer {
    /// The outcome, as the patch count and logs name it.
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            ProposalAnswer::Saved { .. } => "saved",
            ProposalAnswer::AlreadySaved { .. } => "already_saved",
            ProposalAnswer::Existing { .. } => "existing",
        }
    }
}

// PRD: C14, D7, I6.
/// `POST /api/proposals/{id}/preview`: what applying the proposal now would do, and,
/// when its destination moved since it was drafted, what moved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalReview {
    /// The proposal, at the editing revision a reviewer applies.
    pub proposal: Proposal,
    /// Its items still unresolved, the violations of its candidate, and for a journey the
    /// frontier after.
    pub preview: ProposalPreview,
    /// What the destination would be once it applies: a route, the deployment, or a journey
    /// the proposal creates; none when the candidate breaks a rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<DestinationAfter>,
    // PRD: D7.
    /// By journey: what applying it would newly cause; a journey with nothing new is left
    /// out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub consequences: BTreeMap<JourneyId, Consequences>,
    // PRD: I6.
    /// Present when the destination moved since drafting: applying it is stale until it
    /// is refreshed and reviewed again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<StaleBase>,
}

// PRD: I6.
/// A proposal's destination moved past the revision it was drafted against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaleBase {
    /// The destination's revision the proposal names, and its current one.
    pub conflict: RevisionConflict,
    // PRD: H5.
    /// What the intervening events touched.
    pub intervening: TouchedSet,
}

impl From<crate::ProposalReview> for ProposalReview {
    fn from(review: crate::ProposalReview) -> Self {
        let crate::ProposalReview {
            proposal,
            preview,
            after,
            consequences,
            stale,
        } = review;
        Self {
            proposal,
            preview,
            after,
            consequences,
            stale: stale.map(|stale| StaleBase {
                conflict: stale.conflict,
                intervening: stale.intervening,
            }),
        }
    }
}

// PRD: A13.
/// `POST /api/routes/{id}/import`: a route file, imported as a new route or a new draft of
/// the path's route, matched against the version it extends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RouteImport {
    // PRD: H5.
    /// The import's patch id: the same file under the same id is the same patch.
    pub patch_id: PatchId,
    // ARCHITECTURE: File format.
    /// The file, as JSON; its route must be the path's.
    pub file: RouteFile,
    // PRD: J1.
    /// A note for each of its events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
}

// PRD: C14.
/// A proposal's destination as it is now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DestinationNow {
    /// Whether it exists.
    pub exists: bool,
    /// Its name, when it exists and has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Title>,
    /// Its revision; 0 when it does not exist.
    pub revision: Revision,
    /// A journey's status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<JourneyStatus>,
}

// PRD: C14.
/// An open proposal in a listing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalSummary {
    /// The proposal.
    pub id: ProposalId,
    /// Its destination.
    pub destination: Domain,
    /// What it proposes, in a line.
    pub title: Title,
    /// Its editing revision.
    pub revision: Revision,
    /// The destination's revision it was drafted against.
    pub drafted_against: Revision,
    /// The user it was created by or for.
    pub created_by: UserId,
    /// The agent that drafted it, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposing_agent: Option<AgentId>,
    /// When it was created.
    pub created_at: Timestamp,
    /// What it does, in facts, worked out from its mutations.
    pub outline: ProposalOutline,
    /// Its destination as it is now.
    pub now: DestinationNow,
    // PRD: I6.
    /// Its destination moved since it was drafted (written, created, or removed since): it
    /// needs a refresh before it can be applied.
    pub stale: bool,
}

/// Where a proposal listing resumes: pass its two fields as `after_at` and `after`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalCursor {
    /// When the last proposal of the page before was created.
    pub created_at: Timestamp,
    /// Its id.
    pub id: ProposalId,
}

// PRD: C14, C16.
/// A page of open proposals, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposalList {
    /// How many open proposals the listing holds in all.
    pub count: u32,
    /// A destination's listing: the destination as it is now, so a header does not depend on
    /// having rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<DestinationNow>,
    /// The proposals.
    pub items: Vec<ProposalSummary>,
    /// Pass as `after_at` and `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<ProposalCursor>,
}

impl From<cairn_store::DestinationNow> for DestinationNow {
    fn from(now: cairn_store::DestinationNow) -> Self {
        let cairn_store::DestinationNow {
            exists,
            name,
            revision,
            status,
        } = now;
        Self {
            exists,
            name,
            revision,
            status,
        }
    }
}

impl From<cairn_store::ProposalSummary> for ProposalSummary {
    fn from(summary: cairn_store::ProposalSummary) -> Self {
        let cairn_store::ProposalSummary {
            id,
            destination,
            title,
            revision,
            drafted_against,
            created_by,
            proposing_agent,
            created_at,
            outline,
            now,
            stale,
        } = summary;
        Self {
            id,
            destination,
            title,
            revision,
            drafted_against,
            created_by,
            proposing_agent,
            created_at,
            outline,
            now: now.into(),
            stale,
        }
    }
}

impl From<cairn_store::ProposalPage> for ProposalList {
    fn from(page: cairn_store::ProposalPage) -> Self {
        let cairn_store::ProposalPage {
            count,
            destination,
            items,
            next,
        } = page;
        Self {
            count,
            destination: destination.map(Into::into),
            items: items.into_iter().map(Into::into).collect(),
            next: next.map(|cursor| ProposalCursor {
                created_at: cursor.created_at,
                id: cursor.id,
            }),
        }
    }
}
