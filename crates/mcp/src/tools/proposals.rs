//! The proposal tools (I6, C14): a proposal is written by adding `proposal` to a patch tool
//! (see `writes`), and here it is changed, discarded, refreshed against its own editing
//! revision, and applied by a user, who is recorded as confirming it (H2). Save as route is
//! drafted by Cairn when it is the content of a proposal write (B8, I7).

use cairn_schema::{Markdown, PatchId, Proposal, ProposalDraft, ProposalId, Revision};
use cairn_service::Call;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{Spec, Writes, output, parse, schema, schema_citing_mutations};
use crate::{ToolError, ToolSet};

/// The proposal tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "edit_proposal",
        description: "Replace the whole content of an open proposal, discard it, or refresh it, \
            against its editing revision. To add to it, write the same patch again with its \
            `proposal` and `proposal_revision`.",
        writes: Writes::Proposal,
        destructive: true,
        schema: schema_citing_mutations::<EditProposal>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "apply_proposal",
        description: "Apply a reviewed proposal; you are recorded as confirming it.",
        writes: Writes::Confirmation,
        destructive: true,
        schema: schema::<ApplyProposal>,
        output: schema::<WriteOutput>,
    },
];

impl ToolSet {
    /// Runs the proposal tool `name`.
    pub(crate) async fn propose(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        match name {
            "edit_proposal" => output(self.edit_proposal(call, parse(arguments)?).await),
            "apply_proposal" => output(self.apply_proposal(call, parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }

    async fn edit_proposal(
        &self,
        call: &Call,
        arguments: EditProposal,
    ) -> Result<WriteOutput, ToolError> {
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
