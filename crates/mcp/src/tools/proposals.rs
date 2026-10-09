//! The proposal tools (I6, C14): a proposal is drafted against any domain under an id the
//! agent chooses, so a lost response is answered by fetching or resubmitting it; edited,
//! discarded, or refreshed against its own editing revision; and applied by a user, who is
//! recorded as confirming it (H2). Upgrade, save as route, and re-link are drafted by Cairn
//! as proposals and reach their destination only when one is applied (B7, B8, B9, I7).

use cairn_schema::{
    Domain, JourneyId, Lineage, Markdown, PatchId, PatchReceipt, Proposal, ProposalDraft,
    ProposalId, Revision, RouteId, Title, VersionNumber,
};
use cairn_service::{Call, ProposalWritten};
use cairn_store::Store;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{Spec, output, parse, schema, schema_citing_mutations};
use crate::{ToolError, ToolSet};

/// The proposal tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "create_proposal",
        description: "Drafts a proposal against a journey, a route (its draft), or the \
            deployment, under an id you choose (I6): ordered mutations for someone to review \
            and apply, as one patch. A destination that does not exist yet is created at \
            destination revision 0. Use it for structure, breakdowns, and any change a person \
            should see whole before it lands. Resubmitting the same id answers the proposal \
            as it stands. To reuse a segment, propose one insert_segment mutation: \
            {\"op\": \"insert_segment\", \"insertion\": \"i_<new key>\", \"segment\": \
            {\"route\": \"security-review\", \"version\": 1}, \"parent\": \"<node key or \
            omit>\", \"edges\": [{\"node\": {\"segment\": \"<segment node key>\"}, \
            \"requires\": {\"host\": \"<node key>\"}}]}.",
        writes: true,
        destructive: false,
        schema: schema_citing_mutations::<CreateProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "edit_proposal",
        description: "Changes an open proposal against its editing revision: replace its \
            content, discard it, or refresh it against its destination as it stands now (when \
            the destination moved since it was drafted). Each change needs a new review.",
        writes: true,
        destructive: true,
        schema: schema_citing_mutations::<EditProposal>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "apply_proposal",
        description: "Applies a reviewed proposal: both its editing revision and its \
            destination's are checked, its mutations validated and committed with its \
            applied status, and the caller recorded as confirming it (H2, I6). Rejected with \
            every violation, or as stale with what moved (then refresh it).",
        writes: true,
        destructive: true,
        schema: schema::<ApplyProposal>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "upgrade",
        description: "Drafts upgrading a journey to a newer version of its route as a \
            proposal (B7): route changes it did not edit are taken, its local edits kept, and \
            conflicts become review items. Nothing changes until it is applied.",
        writes: true,
        destructive: false,
        schema: schema::<Upgrade>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "save_as_route",
        description: "Drafts saving a journey's structure as a new route's draft, as a \
            proposal on that route (B8), with review items for participations and nodes to \
            leave out.",
        writes: true,
        destructive: false,
        schema: schema::<SaveAsRoute>,
        output: schema::<ProposalOutput>,
    },
    Spec {
        name: "relink",
        description: "Drafts linking a journey to a published route version as a proposal \
            (B9): where it differs from the version is kept as its own edit unless the \
            reviewer takes the route's.",
        writes: true,
        destructive: false,
        schema: schema::<Relink>,
        output: schema::<ProposalOutput>,
    },
];

impl<S: Store + 'static> ToolSet<S> {
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
                let refresh = self.service.refresh_proposal(call, patch_id, id, base);
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

/// What a proposal write answers.
#[derive(Debug, Serialize, JsonSchema)]
struct ProposalOutput {
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

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ProposalStatusOutput {
    Saved,
    AlreadySaved,
    Existing,
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
pub(crate) struct CreateProposal {
    /// The proposal's id, chosen by the caller (`pr_` and a slug).
    proposal: ProposalId,
    /// The domain it would change: `{"journey": id}`, `{"route": id}`, or `"deployment"`.
    destination: Domain,
    /// Its content: a title, an optional description, the destination revision it was
    /// drafted against (0 to create the destination), and its mutations in order.
    draft: ProposalDraft,
    /// A new id for this write.
    patch_id: PatchId,
}

/// `edit_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditProposal {
    /// The proposal.
    proposal: ProposalId,
    /// The change.
    change: ProposalChange,
    /// The proposal's editing revision the change was drafted against.
    base_revision: Revision,
    /// A new id for this write.
    patch_id: PatchId,
}

/// A change to an open proposal.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ProposalChange {
    /// Replace its content.
    Replace {
        /// The new content.
        draft: ProposalDraft,
    },
    /// Discard it.
    Discard,
    /// Draft it again against its destination as it stands (I6), keeping the reviewer's
    /// choices; it must be reviewed again.
    Refresh,
}

/// `apply_proposal`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyProposal {
    /// The proposal.
    proposal: ProposalId,
    /// The editing revision that was reviewed: a later edit makes the apply stale.
    reviewed_revision: Revision,
    /// A new id for this write.
    patch_id: PatchId,
    /// A note recorded on the apply's events.
    #[serde(default)]
    note: Option<Markdown>,
}

/// `upgrade`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Upgrade {
    /// The journey.
    journey: JourneyId,
    /// The version of its route to upgrade to.
    to: VersionNumber,
    /// The proposal's id, chosen by the caller.
    proposal: ProposalId,
    /// A new id for this write.
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
    /// The proposal's id, chosen by the caller.
    proposal: ProposalId,
    /// A new id for this write.
    patch_id: PatchId,
}

/// `relink`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Relink {
    /// The journey.
    journey: JourneyId,
    /// The route version to link it to.
    to: Lineage,
    /// The proposal's id, chosen by the caller.
    proposal: ProposalId,
    /// A new id for this write.
    patch_id: PatchId,
}
