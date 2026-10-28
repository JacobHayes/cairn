//! The direct-write tools: each call is one patch (A17), named by a client-generated patch id
//! so a lost response can be resubmitted safely (H5), against the base revision the agent
//! read. `update_journey_state` records what happened in one journey; `apply_patch` takes any
//! domain patch, state and structure alike, so a mixed change stays one patch;
//! `import_route` imports a route file; `manage_identity` writes the deployment's identities,
//! reading the journeys a merge names
//! (decisions/2026-10-28-direct-mcp-writes-are-patches-through-two-tools.md).

use cairn_schema::{
    Domain, Identity, IdentityKey, IdentitySet, JourneyId, Markdown, Mutation, Mutations, Patch,
    PatchId, PatchTarget, Revision, RouteFile, from_yaml,
};
use cairn_service::{Call, DomainPatch};

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
        description: "Record what happened in one journey, as one patch of ordered mutations: \
            answers, transitions, notes, ticks, who does a node, pins and recorded dates, \
            snoozes, overrides, the journey's name, owners and status, and a new identity an \
            answer names. It never changes what the graph is: that is apply_patch or a \
            proposal.",
        writes: Writes::Patch,
        destructive: true,
        schema: || schema_with_mutation_sketch::<UpdateJourneyState>(&records_journey_state),
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "apply_patch",
        description: "Apply ordered mutations to a journey, a route's draft, or the \
            deployment, all or nothing: any change, structure and state alike. Base revision \
            0 creates the target. Draft a proposal instead when someone should review first.",
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
        description: "Create, edit, merge, or delete an identity (a person or team).",
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
        let Some(submitted) = self.drafted_patch_of(name, arguments).await? else {
            return Err(ToolError::UnknownTool {
                name: name.to_owned(),
            });
        };
        let written = self.service.patch(call, &submitted).await;
        output(written.map(WriteOutput::from).map_err(ToolError::from))
    }

    /// The domain patch the direct-write tool `name` submits for `arguments`, worked out
    /// without writing; `None` for `import_route`, whose patch the service builds from the
    /// file, and for a name that is not a direct-write tool.
    pub(crate) async fn drafted_patch_of(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<Option<DomainPatch>, ToolError> {
        let drafted = match name {
            "update_journey_state" => parse::<UpdateJourneyState>(arguments)?.patch(),
            "apply_patch" => parse::<ApplyPatch>(arguments)?.patch(),
            "manage_identity" => self.identity_patch(parse(arguments)?).await,
            _ => return Ok(None),
        }?;
        Ok(Some(drafted))
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
/// The ops that change a proposal rather than a domain, which no direct write takes:
/// `apply_proposal` has its own tool, which records the caller as confirming the proposal,
/// and the others are the proposal tools' to write.
const PROPOSAL_OPS: [&str; 4] = [
    "create_proposal",
    "edit_proposal",
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
    /// Needed when the mutations name identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    #[serde(default)]
    note: Option<Markdown>,
    mutations: Mutations,
}

impl UpdateJourneyState {
    fn patch(self) -> Result<DomainPatch, ToolError> {
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
                          with apply_patch, or draft a proposal"
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
        DomainPatch::new(patch, self.note).map_err(ToolError::refused)
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
    #[serde(default)]
    note: Option<Markdown>,
    mutations: Mutations,
}

impl ApplyPatch {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        refuse_proposal_ops("mutations", &self.mutations)?;
        let patch = Patch {
            id: self.patch_id,
            target: self.target.into(),
            base_revision: self.base_revision,
            deployment_revision: self.deployment_revision,
            mutations: self.mutations,
        };
        DomainPatch::new(patch, self.note)
            .map_err(|_| unreachable!("a direct target is a domain, never a proposal"))
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
        "a proposal is drafted and changed with create_proposal and edit_proposal"
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
