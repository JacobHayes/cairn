//! The proposal tools (I6, C14): a proposal is drafted against any domain under an id the
//! agent chooses, so a lost response is answered by fetching or resubmitting it; edited,
//! discarded, or refreshed against its own editing revision; and applied by a user, who is
//! recorded as confirming it (H2). Upgrade, save as route, and re-link are drafted by Cairn
//! as proposals and reach their destination only when one is applied (B7, B8, B9, I7).

use cairn_schema::{
    Domain, JourneyId, Lineage, Markdown, PatchId, Proposal, ProposalDraft, ProposalId,
    ProposalStatus, Revision, RouteId, Title, VersionNumber,
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
        description: "Draft a proposal for someone to review, under an id you choose: your own ordered \
            mutations to a destination (one that does not exist yet is created at revision 0), \
            or one Cairn drafts: upgrading a journey to a newer version of its route, saving a \
            journey's structure as a new route, or linking a journey to a published route \
            version. Name exactly one.\n\n\
            To reuse a recurring piece of a process, find a segment first (`list_routes` with \
            `kind` `segment`) and propose an `insert_segment`; the insert-segment workflow has \
            the wiring and an example.",
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
            "create_proposal" => output(self.create_proposal(call, parse(arguments)?).await),
            "edit_proposal" => output(self.edit_proposal(call, parse(arguments)?).await),
            "apply_proposal" => output(self.apply_proposal(call, parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }

    // PRD: I6, B7, B8, B9.
    async fn create_proposal(
        &self,
        call: &Call,
        arguments: CreateProposal,
    ) -> Result<ProposalOutput, ToolError> {
        let (proposal, patch_id, content) = arguments.content()?;
        let written = match content {
            ProposalContent::Draft { destination, draft } => {
                let create =
                    self.service
                        .create_proposal(call, patch_id, &destination, &proposal, draft);
                create.await?
            }
            ProposalContent::Upgrade(UpgradeTo { journey, to }) => {
                let upgrade = self
                    .service
                    .propose_upgrade(call, patch_id, &proposal, &journey, to);
                upgrade.await?
            }
            ProposalContent::SaveAsRoute(SaveAs {
                journey,
                route,
                name,
            }) => {
                let save = self
                    .service
                    .propose_save_as_route(call, patch_id, &proposal, &journey, &route, &name);
                save.await?
            }
            ProposalContent::Relink(RelinkTo { journey, to }) => {
                let relink = self
                    .service
                    .propose_relink(call, patch_id, &proposal, &journey, &to);
                relink.await?
            }
        };
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
}

/// What a proposal write answers: where the proposal stands, not the draft that was sent.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProposalOutput {
    /// `saved` now; `already_saved`, a resubmitted patch id answered from its receipt (fetch
    /// the proposal for where it stands); or `existing`, a create of an id the caller already
    /// holds, answered with it as it stands.
    status: ProposalStatusOutput,
    /// The proposal's id; absent when the answer is only a receipt (`already_saved`).
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal: Option<ProposalId>,
    /// Where it would land.
    destination: Domain,
    /// Its editing revision: review it with `get_proposal` before applying, and name this
    /// revision in its next change.
    revision: Revision,
    /// The destination revision it was drafted against; absent when the answer is only a
    /// receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    destination_revision: Option<Revision>,
    /// Whether it is still open; absent when the answer is only a receipt.
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<ProposalStatus>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ProposalStatusOutput {
    Saved,
    AlreadySaved,
    Existing,
}

impl ProposalOutput {
    /// The proposal's destination.
    #[must_use]
    pub fn destination(&self) -> &Domain {
        &self.destination
    }

    /// Whether the answer says where the proposal stands (a receipt does not).
    #[must_use]
    pub fn state_known(&self) -> bool {
        self.state.is_some()
    }

    /// Whether the answer says the proposal is discarded.
    #[must_use]
    pub fn is_discarded(&self) -> bool {
        self.state == Some(ProposalStatus::Discarded)
    }

    fn held(status: ProposalStatusOutput, proposal: Proposal) -> Self {
        Self {
            status,
            destination_revision: Some(proposal.draft.destination_revision),
            state: Some(proposal.status),
            proposal: Some(proposal.id),
            destination: proposal.destination,
            revision: proposal.revision,
        }
    }
}

impl From<ProposalWritten> for ProposalOutput {
    fn from(written: ProposalWritten) -> Self {
        match written {
            ProposalWritten::Saved { proposal, .. } => {
                Self::held(ProposalStatusOutput::Saved, proposal)
            }
            ProposalWritten::AlreadySaved { receipt } => Self {
                status: ProposalStatusOutput::AlreadySaved,
                proposal: None,
                destination: receipt.domain,
                revision: receipt.revision,
                destination_revision: None,
                state: None,
            },
            ProposalWritten::Existing { proposal } => {
                Self::held(ProposalStatusOutput::Existing, proposal)
            }
        }
    }
}

/// `create_proposal`: exactly one content, a draft of your own (`destination` with `draft`)
/// or one Cairn drafts (`upgrade`, `save_as_route`, or `relink`). The listed schema is a
/// plain object, as every provider requires of a tool's arguments; `content`
/// refuses a call that names none or more than one.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProposal {
    /// A new id for the proposal.
    pub proposal: ProposalId,
    pub patch_id: PatchId,
    /// With `draft`: `{"journey": id}`, `{"route": id}`, or `"deployment"`.
    #[serde(default, deserialize_with = "named")]
    pub destination: Option<Domain>,
    /// With `destination`: title, optional description, the destination revision (0 to
    /// create it), and the mutations in order.
    #[serde(default, deserialize_with = "named")]
    pub draft: Option<ProposalDraft>,
    /// Upgrading a journey to a newer version of its route.
    #[serde(default, deserialize_with = "named")]
    pub upgrade: Option<UpgradeTo>,
    /// Saving a journey's structure as a new route.
    #[serde(default, deserialize_with = "named")]
    pub save_as_route: Option<SaveAs>,
    /// Linking a journey to a published route version.
    #[serde(default, deserialize_with = "named")]
    pub relink: Option<RelinkTo>,
}

/// A content argument that is named: present with a value. A null is refused at its path,
/// as the listed schema refuses it, rather than read as absent.
fn named<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// What a proposal holds: one content.
enum ProposalContent {
    Draft {
        destination: Domain,
        draft: ProposalDraft,
    },
    Upgrade(UpgradeTo),
    SaveAsRoute(SaveAs),
    Relink(RelinkTo),
}

impl CreateProposal {
    /// The proposal, the patch id, and its one content; refused at the argument that makes
    /// it none or more than one.
    fn content(self) -> Result<(ProposalId, PatchId, ProposalContent), ToolError> {
        let own = if self.destination.is_some() {
            Some("destination")
        } else {
            self.draft.as_ref().map(|_| "draft")
        };
        let kinds: Vec<&str> = [
            own,
            self.upgrade.as_ref().map(|_| "upgrade"),
            self.save_as_route.as_ref().map(|_| "save_as_route"),
            self.relink.as_ref().map(|_| "relink"),
        ]
        .into_iter()
        .flatten()
        .collect();
        if kinds.len() != 1 {
            return Err(ToolError::Arguments {
                // The second content named, or the whole call when it names none.
                path: kinds.get(1).copied().unwrap_or_default().to_owned(),
                message: "name exactly one content: destination with draft, upgrade, \
                    save_as_route, or relink"
                    .to_owned(),
            });
        }
        let content = match (
            self.destination,
            self.draft,
            self.upgrade,
            self.save_as_route,
            self.relink,
        ) {
            (Some(destination), Some(draft), ..) => ProposalContent::Draft { destination, draft },
            (Some(_), None, ..) | (None, Some(_), ..) => {
                let missing = if own == Some("destination") {
                    "draft"
                } else {
                    "destination"
                };
                return Err(ToolError::Arguments {
                    path: missing.to_owned(),
                    message: "a draft of your own names both destination and draft".to_owned(),
                });
            }
            (None, None, Some(upgrade), ..) => ProposalContent::Upgrade(upgrade),
            (None, None, None, Some(save), _) => ProposalContent::SaveAsRoute(save),
            (None, None, None, None, Some(relink)) => ProposalContent::Relink(relink),
            (None, None, None, None, None) => unreachable!("one content was named"),
        };
        Ok((self.proposal, self.patch_id, content))
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

// PRD: B7.
/// A journey's upgrade to a newer version of its route.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpgradeTo {
    journey: JourneyId,
    /// The route version to upgrade to.
    to: VersionNumber,
}

// PRD: B8.
/// A journey's structure saved as a new route.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveAs {
    /// The journey whose structure to save.
    journey: JourneyId,
    /// The new route's id.
    route: RouteId,
    /// The new route's name.
    name: Title,
}

// PRD: B9.
/// A journey linked to a published route version.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelinkTo {
    journey: JourneyId,
    /// The route version to link to.
    to: Lineage,
}
