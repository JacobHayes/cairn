//! The direct-write tools: each call is one patch (A17), named by a client-generated patch id
//! so a lost response can be resubmitted safely (H5), against the base revision the agent
//! read. `update_journey_state` records what happened in one journey; `apply_patch` takes any
//! domain patch, state and structure alike, so a mixed change stays one patch;
//! `import_route` imports a route file; `manage_identity` writes the deployment's identities,
//! reading the journeys a merge names
//! (decisions/2026-10-28-direct-mcp-writes-are-patches-through-two-tools.md).

use cairn_schema::{
    Domain, Identity, IdentityKey, IdentitySet, JourneyId, Markdown, Mutation, Mutations, Patch,
    PatchId, PatchTarget, ProposalId, Revision, RouteFile, from_yaml,
};
use cairn_service::{Call, DomainPatch, Proposing};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{Spec, Writes, output, parse, schema, schema_with_mutation_sketch};
use crate::{ToolError, ToolSet};

/// The direct-write tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "update_journey_state",
        description: "Record what happened in a journey, as one patch of ordered mutations: answers, \
            transitions, notes, ticks, who does a node, pins and dates, snoozes, overrides, \
            status. It never changes the graph: that is apply_patch.\n\n\
            Add `proposal` to have the patch reviewed instead of applied. \
            Order is never a guard: start, finish or answer a node whatever it waits on or its \
            own date. Reopening what finished work relied on marks that work `stale`.",
        writes: Writes::Patch,
        destructive: true,
        schema: || schema_with_mutation_sketch::<UpdateJourneyState>(&records_journey_state),
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "apply_patch",
        description: "Apply ordered mutations to a journey, a route's draft, or the deployment, all or \
            nothing: structure and state alike, so a mix is one patch. Prefer `set_node_field` \
            for one field. Deleting is permanent, so only when the person asks; one patch to \
            the `deployment` deletes any number.\n\n\
            Add `proposal` (a new id) to draft the patch for review instead of applying it: it is \
            created with `note` as its title, against `base_revision`; with the id of an open \
            proposal and its `proposal_revision`, the mutations are added to it, drafted against \
            the revision it names. A person applies a proposal. Saving a journey as a route is an \
            `open_draft` from `save_as_route` (after `create_route` for a new route), alone, with \
            `proposal`: Cairn drafts the route.",
        writes: Writes::Patch,
        destructive: true,
        schema: || schema_with_mutation_sketch::<ApplyPatch>(&written_directly),
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "import_route",
        description: "Import a YAML route file as a new route, or as a new draft of an \
            existing one.",
        writes: Writes::File,
        destructive: false,
        schema: schema::<ImportRoute>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "manage_identity",
        description: "Create, edit, merge, or delete an identity (a person or team). An identity has a \
            name, emails, or both; someone who only signed in has emails and no name.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<ManageIdentity>,
        output: schema::<WriteOutput>,
    },
];

impl ToolSet {
    /// Runs the direct-write tool `name`.
    pub(crate) async fn write(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        if name == "import_route" {
            return output(self.import_route(call, parse(arguments)?).await);
        }
        let (submitted, proposing) = match name {
            "update_journey_state" => parse::<UpdateJourneyState>(arguments)?.patch()?,
            "apply_patch" => parse::<ApplyPatch>(arguments)?.patch()?,
            "manage_identity" => (self.identity_patch(parse(arguments)?).await?, None),
            _ => {
                return Err(ToolError::UnknownTool {
                    name: name.to_owned(),
                });
            }
        };
        if let Some(proposing) = proposing {
            let proposed = self.service.propose(call, &submitted, &proposing).await;
            output(proposed.map(WriteOutput::from).map_err(ToolError::from))
        } else {
            let written = self.service.patch(call, &submitted).await;
            output(written.map(WriteOutput::from).map_err(ToolError::from))
        }
    }

    // PRD: A13.
    /// `import_route`: one route patch.
    async fn import_route(
        &self,
        call: &Call,
        arguments: ImportRoute,
    ) -> Result<WriteOutput, ToolError> {
        let file: RouteFile = from_yaml(&arguments.file).map_err(|error| ToolError::Arguments {
            path: if error.path.is_empty() {
                "file".to_owned()
            } else {
                format!("file: {}", error.path)
            },
            message: error.message,
        })?;
        let written = self
            .service
            .import_route(call, arguments.patch_id, &file, arguments.note)
            .await?;
        Ok(written.into())
    }

    // PRD: E6.
    /// `manage_identity`: a merge names every journey referring to any of its identities at the
    /// revision it was checked against, which the tool reads from the journey index.
    async fn identity_patch(&self, arguments: ManageIdentity) -> Result<DomainPatch, ToolError> {
        let mutation = match arguments.change {
            IdentityChange::Create { identity } => Mutation::CreateIdentity { identity },
            IdentityChange::Edit { identity } => Mutation::EditIdentity { identity },
            IdentityChange::Merge { survivor, merged } => {
                let referencing = std::iter::once(survivor.clone())
                    .chain(merged.iter().cloned())
                    .collect();
                let journeys = self.service.merge_revisions(referencing).await?;
                Mutation::MergeIdentities {
                    survivor,
                    merged,
                    journeys,
                }
            }
            IdentityChange::Delete { identity } => Mutation::DeleteIdentity { identity },
        };
        let patch = Patch {
            id: arguments.patch_id,
            target: PatchTarget::Deployment,
            base_revision: arguments.base_revision,
            deployment_revision: None,
            mutations: Mutations::new(vec![mutation])
                .unwrap_or_else(|error| unreachable!("one mutation is a patch's worth: {error}")),
        };
        DomainPatch::new(patch, arguments.note).map_err(ToolError::refused)
    }
}

// PRD: I6.
/// The ops that change a proposal rather than a domain, which no patch carries:
/// `apply_proposal` has its own tool, which records the caller as confirming the proposal,
/// and a proposal is made and added to by the `proposal` argument.
const PROPOSAL_OPS: [&str; 5] = [
    "create_proposal",
    "edit_proposal",
    "extend_proposal",
    "apply_proposal",
    "discard_proposal",
];

/// Whether a direct write takes the op `op`: what `apply_patch` lists.
fn written_directly(op: &str) -> bool {
    !PROPOSAL_OPS.contains(&op)
}

/// The ops whose mutations record what happened in a journey
/// ([`Mutation::records_journey_state`]): what `update_journey_state` lists and takes.
const JOURNEY_STATE_OPS: [&str; 22] = [
    "edit_journey",
    "set_journey_status",
    "set_participation",
    "clear_participation",
    "transition",
    "answer",
    "set_recorded_date",
    "fill_role",
    "clear_role_fill",
    "set_pin",
    "shift_pin",
    "clear_pin",
    "snooze",
    "unsnooze",
    "apply_override",
    "remove_override",
    "set_atomic",
    "set_ticked",
    "add_annotation",
    "edit_annotation",
    "remove_annotation",
    "create_identity",
];

/// Whether `update_journey_state` takes the op `op`.
fn records_journey_state(op: &str) -> bool {
    JOURNEY_STATE_OPS.contains(&op)
}

/// `update_journey_state`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateJourneyState {
    journey: JourneyId,
    patch_id: PatchId,
    base_revision: Revision,
    /// Needed when the mutations name identities. A person not in Cairn yet is a
    /// `create_identity` earlier in the same patch.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// The title of a proposal this write creates; otherwise a note for its events.
    #[serde(default)]
    note: Option<Markdown>,
    mutations: Mutations,
    /// Draft this patch into the proposal with this id (a new `pr_` id creates it) instead of
    /// applying it.
    #[serde(default)]
    proposal: Option<ProposalId>,
    /// With `proposal`, when it exists: its editing revision, from the answer that created or
    /// last changed it.
    #[serde(default)]
    proposal_revision: Option<Revision>,
}

impl UpdateJourneyState {
    fn patch(self) -> Result<(DomainPatch, Option<Proposing>), ToolError> {
        refuse_proposal_ops("mutations", &self.mutations)?;
        let found = self
            .mutations
            .as_slice()
            .iter()
            .position(|mutation| !mutation.records_journey_state());
        if let Some(index) = found {
            return Err(ToolError::Arguments {
                path: format!("mutations[{index}].op"),
                message: "update_journey_state records what happened in a journey; this op \
                          changes what the graph is, a route, or the deployment: send it \
                          with apply_patch"
                    .to_owned(),
            });
        }
        let patch = Patch {
            id: self.patch_id,
            target: PatchTarget::Journey(self.journey),
            base_revision: self.base_revision,
            deployment_revision: self.deployment_revision,
            mutations: self.mutations,
        };
        let proposing = Proposing::named(self.proposal, self.proposal_revision)?;
        let submitted = DomainPatch::new(patch, self.note).map_err(ToolError::refused)?;
        Ok((submitted, proposing))
    }
}

/// `apply_patch`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyPatch {
    /// `{"journey": id}`, `{"route": id}`, or `"deployment"`.
    target: Domain,
    patch_id: PatchId,
    /// 0 creates the target.
    base_revision: Revision,
    /// Needed when a journey write names identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// The title of a proposal this write creates; otherwise a note for its events.
    #[serde(default)]
    note: Option<Markdown>,
    mutations: Mutations,
    /// Draft this patch into the proposal with this id (a new `pr_` id creates it) instead of
    /// applying it.
    #[serde(default)]
    proposal: Option<ProposalId>,
    /// With `proposal`, when it exists: its editing revision, from the answer that created or
    /// last changed it.
    #[serde(default)]
    proposal_revision: Option<Revision>,
}

impl ApplyPatch {
    fn patch(self) -> Result<(DomainPatch, Option<Proposing>), ToolError> {
        refuse_proposal_ops("mutations", &self.mutations)?;
        let patch = Patch {
            id: self.patch_id,
            target: self.target.into(),
            base_revision: self.base_revision,
            deployment_revision: self.deployment_revision,
            mutations: self.mutations,
        };
        let proposing = Proposing::named(self.proposal, self.proposal_revision)?;
        let submitted = DomainPatch::new(patch, self.note)
            .unwrap_or_else(|_| unreachable!("a target is a domain, never a proposal"));
        Ok((submitted, proposing))
    }
}

/// Refuses a mutation that changes a proposal, at its path under `at`.
fn refuse_proposal_ops(at: &str, mutations: &Mutations) -> Result<(), ToolError> {
    let found = mutations
        .as_slice()
        .iter()
        .enumerate()
        .find(|(_, mutation)| mutation.edits_a_proposal());
    let Some((index, mutation)) = found else {
        return Ok(());
    };
    let message = if matches!(mutation, Mutation::ApplyProposal { .. }) {
        "a proposal is applied with apply_proposal, which records you as confirming it"
    } else {
        "a proposal is made and added to by naming it in `proposal`, and changed with edit_proposal"
    };
    Err(ToolError::Arguments {
        path: format!("{at}[{index}].op"),
        message: message.to_owned(),
    })
}

/// `import_route`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportRoute {
    patch_id: PatchId,
    /// The route file in YAML (or JSON); `extends` imports a new draft of an existing
    /// route.
    file: String,
    #[serde(default)]
    note: Option<Markdown>,
}

/// `manage_identity`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManageIdentity {
    change: IdentityChange,
    patch_id: PatchId,
    /// The deployment revision.
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

// PRD: E6.
/// A change to the deployment's identities.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum IdentityChange {
    /// Create an identity with its key: `who_` and 16 random lowercase letters and digits, never
    /// derived from a name.
    Create {
        /// The identity.
        identity: Identity,
    },
    /// Replace an identity's name and emails.
    Edit {
        /// The identity as it should be.
        identity: Identity,
    },
    /// Merge the `merged` identities into `survivor` in one step; each merged key becomes an
    /// alias of it and its emails move to it. Use it when an identity something refers to is a
    /// duplicate.
    Merge {
        /// The identity that remains.
        survivor: IdentityKey,
        /// The duplicates: at least one.
        merged: IdentitySet,
    },
    /// Delete an identity nothing refers to (no journey, route draft or route version, directly
    /// or through an alias), with its emails and aliases. One something refers to is refused,
    /// naming where; merge it into the identity that remains instead.
    Delete {
        /// The identity.
        identity: IdentityKey,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mutation, one of each op at least.
    const EVERY_MUTATION: &str = include_str!("../../../schema/tests/data/every_mutation.yaml");

    /// What each patch tool lists is what it takes: `apply_patch` leaves an op out exactly
    /// when its mutations change a proposal, and `update_journey_state` lists exactly the ops
    /// whose mutations record what happened in a journey.
    #[test]
    fn each_patch_tool_lists_the_ops_it_takes() {
        let patch: Patch = cairn_schema::from_yaml(EVERY_MUTATION).expect("every mutation");
        for mutation in patch.mutations.as_slice() {
            let op = serde_json::to_value(mutation).expect("a mutation")["op"].clone();
            let op = op.as_str().expect("an op");
            assert_eq!(written_directly(op), !mutation.edits_a_proposal(), "{op}");
            assert_eq!(
                records_journey_state(op),
                mutation.records_journey_state(),
                "{op}"
            );
        }
    }
}
