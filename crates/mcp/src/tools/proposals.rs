//! The proposal tools (I6, C14): a proposal is drafted against any domain under an id the
//! agent chooses, so a lost response is answered by fetching or resubmitting it; edited,
//! discarded, or refreshed against its own editing revision; and applied by a user, who is
//! recorded as confirming it (H2). Upgrade, save as route, and re-link are drafted by Cairn
//! as proposals and reach their destination only when one is applied (B7, B8, B9, I7).

use cairn_schema::{
    Actor, Domain, JourneyId, Lineage, Markdown, PatchId, PatchReceipt, Proposal, ProposalDraft,
    ProposalId, Revision, RouteId, Title, VersionNumber,
};
use cairn_service::{Call, ProposalWritten};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{Spec, Writes, output, parse, schema, schema_citing_mutations};
use crate::{ToolError, ToolSet};

/// The proposal tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "create_proposal",
        description: "Draft a proposal for someone to review: ordered mutations, under an id \
            you choose. A destination that does not exist yet is created at \
            revision 0.",
        writes: Writes::Proposal,
        destructive: false,
        schema: schema_citing_mutations::<CreateProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "edit_proposal",
        description: "Replace, discard, or refresh an open proposal, against its editing \
            revision.",
        writes: Writes::Proposal,
        destructive: true,
        schema: schema_citing_mutations::<EditProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "apply_proposal",
        description: "Apply a reviewed proposal; you are recorded as confirming it.",
        writes: Writes::Confirmation,
        destructive: true,
        schema: schema::<ApplyProposal>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "upgrade",
        description: "Draft upgrading a journey to a newer version of its route, as a \
            proposal.",
        writes: Writes::Proposal,
        destructive: false,
        schema: schema::<Upgrade>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "save_as_route",
        description: "Draft saving a journey's structure as a new route, as a proposal.",
        writes: Writes::Proposal,
        destructive: false,
        schema: schema::<SaveAsRoute>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "relink",
        description: "Draft linking a journey to a published route version, as a proposal.",
        writes: Writes::Proposal,
        destructive: false,
        schema: schema::<Relink>,
        output: schema::<ProposalOutput>,
    },
];

impl ToolSet {
    // PRD: I5, I6.
    /// Drafts a proposal as `actor`, as `create_proposal` does: what the assistant's write
    /// wrapper drafts a write it does not apply directly into.
    ///
    /// # Errors
    ///
    /// What `create_proposal` answers.
    pub async fn create_proposal_as(
        &self,
        actor: &Actor,
        arguments: CreateProposal,
    ) -> Result<ProposalOutput, ToolError> {
        self.create_proposal(&self.call_as(actor), arguments).await
    }

    // PRD: I5, I6.
    /// Changes an open proposal as `actor`, as `edit_proposal` does.
    ///
    /// # Errors
    ///
    /// What `edit_proposal` answers.
    pub async fn edit_proposal_as(
        &self,
        actor: &Actor,
        arguments: EditProposal,
    ) -> Result<ProposalOutput, ToolError> {
        self.edit_proposal(&self.call_as(actor), arguments).await
    }

    /// What a call of the proposal tool `name` with `arguments` is about, or `None` for
    /// another tool or arguments that do not match its schema.
    #[must_use]
    pub fn proposal_call(name: &str, arguments: &Value) -> Option<ProposalCall> {
        let arguments = arguments.clone();
        let (proposal, discards) = match name {
            "create_proposal" => (parse::<CreateProposal>(arguments).ok()?.proposal, false),
            "edit_proposal" => {
                let edit = parse::<EditProposal>(arguments).ok()?;
                let discards = matches!(edit.change, ProposalChange::Discard);
                (edit.proposal, discards)
            }
            "upgrade" => (parse::<Upgrade>(arguments).ok()?.proposal, false),
            "save_as_route" => (parse::<SaveAsRoute>(arguments).ok()?.proposal, false),
            "relink" => (parse::<Relink>(arguments).ok()?.proposal, false),
            _ => return None,
        };
        Some(ProposalCall { proposal, discards })
    }

    /// Runs the proposal tool `name`.
    pub(crate) async fn propose(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        match name {
            "create_proposal" => output(self.create_proposal(call, parse(arguments)?).await),
            "edit_proposal" => output(self.edit_proposal(call, parse(arguments)?).await),
            "apply_proposal" => output(self.apply_proposal(call, parse(arguments)?).await),
            "upgrade" => output(self.upgrade(call, parse(arguments)?).await),
            "save_as_route" => output(self.save_as_route(call, parse(arguments)?).await),
            "relink" => output(self.relink(call, parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }

    async fn create_proposal(
        &self,
        call: &Call,
        arguments: CreateProposal,
    ) -> Result<ProposalOutput, ToolError> {
        let written = self
            .service
            .create_proposal(
                call,
                arguments.patch_id,
                &arguments.destination,
                &arguments.proposal,
                arguments.draft,
            )
            .await?;
        Ok(written.into())
    }

    async fn edit_proposal(
        &self,
        call: &Call,
        arguments: EditProposal,
    ) -> Result<ProposalOutput, ToolError> {
        let id = &arguments.proposal;
        let (patch_id, base) = (arguments.patch_id, arguments.base_revision);
        let written = match arguments.change {
            ProposalChange::Refresh => {
                let destination = self.held(id).await?.destination;
                let refresh = self
                    .service
                    .refresh_proposal(call, patch_id, &destination, id, base);
                refresh.await?
            }
            ProposalChange::Replace { draft } => {
                let destination = self.held(id).await?.destination;
                let edit =
                    self.service
                        .edit_proposal(call, patch_id, &destination, id, base, draft);
                edit.await?
            }
            ProposalChange::Discard => {
                let destination = self.held(id).await?.destination;
                let discard = self
                    .service
                    .discard_proposal(call, patch_id, &destination, id, base);
                discard.await?
            }
        };
        Ok(written.into())
    }

    /// The proposal `id`, which must exist.
    async fn held(&self, id: &ProposalId) -> Result<Proposal, ToolError> {
        let held = self.service.proposal(id).await?;
        held.ok_or_else(|| ToolError::not_found(format_args!("proposal {id}")))
    }

    async fn apply_proposal(
        &self,
        call: &Call,
        arguments: ApplyProposal,
    ) -> Result<WriteOutput, ToolError> {
        let id = &arguments.proposal;
        let held = self.held(id).await?;
        let written = self
            .service
            .apply_proposal(
                call,
                arguments.patch_id,
                &held.destination,
                id,
                arguments.reviewed_revision,
                arguments.note,
            )
            .await?;
        Ok(written.into())
    }

    async fn upgrade(&self, call: &Call, arguments: Upgrade) -> Result<ProposalOutput, ToolError> {
        let written = self
            .service
            .propose_upgrade(
                call,
                arguments.patch_id,
                &arguments.proposal,
                &arguments.journey,
                arguments.to,
            )
            .await?;
        Ok(written.into())
    }

    async fn save_as_route(
        &self,
        call: &Call,
        arguments: SaveAsRoute,
    ) -> Result<ProposalOutput, ToolError> {
        let written = self
            .service
            .propose_save_as_route(
                call,
                arguments.patch_id,
                &arguments.proposal,
                &arguments.journey,
                &arguments.route,
                &arguments.name,
            )
            .await?;
        Ok(written.into())
    }

    async fn relink(&self, call: &Call, arguments: Relink) -> Result<ProposalOutput, ToolError> {
        let written = self
            .service
            .propose_relink(
                call,
                arguments.patch_id,
                &arguments.proposal,
                &arguments.journey,
                &arguments.to,
            )
            .await?;
        Ok(written.into())
    }
}

/// What a call of a proposal tool is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalCall {
    /// The proposal it names.
    pub proposal: ProposalId,
    /// Whether it discards it.
    pub discards: bool,
}

/// What a proposal write answers.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProposalOutput {
    /// `saved` now; `already_saved`, a resubmitted patch id answered from its receipt (fetch
    /// the proposal for where it stands); or `existing`, a create of an id the caller already
    /// holds, answered with it as it stands.
    status: ProposalStatusOutput,
    /// The receipt: its revision is the proposal's editing revision.
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<PatchReceipt>,
    /// The proposal as it stands: review it with `get_proposal` before applying.
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal: Option<Proposal>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ProposalStatusOutput {
    Saved,
    AlreadySaved,
    Existing,
}

impl ProposalOutput {
    /// The proposal's destination: the proposal's, or, when the answer is only a receipt
    /// (a write answered from an earlier one), the receipt's domain, which is the proposal's
    /// destination.
    #[must_use]
    pub fn destination(&self) -> Option<&Domain> {
        let held = self.proposal.as_ref().map(|proposal| &proposal.destination);
        held.or_else(|| self.receipt.as_ref().map(|receipt| &receipt.domain))
    }

    /// The proposal's editing revision, which its next change names: the proposal's own, or
    /// its receipt's.
    #[must_use]
    pub fn editing_revision(&self) -> Option<Revision> {
        let held = self.proposal.as_ref().map(|proposal| proposal.revision);
        held.or_else(|| self.receipt.as_ref().map(|receipt| receipt.revision))
    }
}

impl From<ProposalWritten> for ProposalOutput {
    fn from(written: ProposalWritten) -> Self {
        match written {
            ProposalWritten::Saved { receipt, proposal } => Self {
                status: ProposalStatusOutput::Saved,
                receipt: Some(receipt),
                proposal: Some(proposal),
            },
            ProposalWritten::AlreadySaved { receipt } => Self {
                status: ProposalStatusOutput::AlreadySaved,
                receipt: Some(receipt),
                proposal: None,
            },
            ProposalWritten::Existing { proposal } => Self {
                status: ProposalStatusOutput::Existing,
                receipt: None,
                proposal: Some(proposal),
            },
        }
    }
}

/// `create_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProposal {
    /// A new id for the proposal.
    pub proposal: ProposalId,
    /// `{"journey": id}`, `{"route": id}`, or `"deployment"`.
    pub destination: Domain,
    /// Title, optional description, the destination revision (0 to create it), and the
    /// mutations in order.
    pub draft: ProposalDraft,
    pub patch_id: PatchId,
}

/// `edit_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditProposal {
    pub proposal: ProposalId,
    pub change: ProposalChange,
    /// The proposal's editing revision.
    pub base_revision: Revision,
    pub patch_id: PatchId,
}

/// A change to an open proposal.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposalChange {
    /// Replace its content.
    Replace {
        /// The new content.
        draft: ProposalDraft,
    },
    /// Discard it.
    Discard,
    // PRD: I6.
    /// Draft it again against its destination as it stands, keeping the reviewer's
    /// choices; it must be reviewed again.
    Refresh,
}

/// `apply_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyProposal {
    proposal: ProposalId,
    /// The editing revision you reviewed.
    reviewed_revision: Revision,
    patch_id: PatchId,
    #[serde(default)]
    note: Option<Markdown>,
}

/// `upgrade`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Upgrade {
    journey: JourneyId,
    /// The route version to upgrade to.
    to: VersionNumber,
    /// A new id for the proposal.
    proposal: ProposalId,
    patch_id: PatchId,
}

/// `save_as_route`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveAsRoute {
    /// The journey whose structure to save.
    journey: JourneyId,
    /// The new route's id.
    route: RouteId,
    /// The new route's name.
    name: Title,
    /// A new id for the proposal.
    proposal: ProposalId,
    patch_id: PatchId,
}

/// `relink`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Relink {
    journey: JourneyId,
    /// The route version to link to.
    to: Lineage,
    /// A new id for the proposal.
    proposal: ProposalId,
    patch_id: PatchId,
}
