//! The direct-write tools: each call is one patch (A17), named by a client-generated patch id
//! so a lost response can be resubmitted safely (H5), against the base revision the agent
//! read. A journey's state tools (answer, transition, snooze, assign, dates, overrides) are
//! one mutation each; `apply_patch` takes any domain patch; the route tools open and publish
//! a draft and import a route file; `manage_identity` writes the deployment.

use cairn_schema::{
    AnswerValue, Date, Domain, Identity, IdentityKey, IdentitySet, JourneyId, KeyRefs, KindKey,
    Lineage, Markdown, Mutation, Mutations, NodeKey, OverrideKind, ParticipationSource, Patch,
    PatchId, PatchTarget, RecordedEnd, Revision, RouteFile, RouteId, SignedDays, SnoozeTarget,
    Title, Transition, from_yaml,
};
use cairn_service::{Call, DomainPatch};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{
    Spec, Writes, output, parse, schema, schema_citing_mutations, schema_with_mutation_sketch,
};
use crate::{ToolError, ToolSet};

/// The direct-write tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "answer_decision",
        description: "Answer a decision. The result lists what it brought in, ruled out, \
            unlocked, filled, or pinned.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<AnswerDecision>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "transition_node",
        description: "Start, stop, complete, skip (with a reason), mark a group done (with a \
            reason), or reopen a node, whatever it waits on. Completing without the evidence it \
            needs takes a guard bypass in the same `apply_patch`.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<TransitionNode>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "assign",
        description: "Set who holds a participation kind on a node: a role or identities. \
            Omit `source` to inherit again.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<Assign>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "snooze",
        description: "Set a node, or a container's whole subtree, aside until a date or \
            until another node is done.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<Snooze>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "unsnooze",
        description: "Lift a node's own snooze.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<Unsnooze>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "set_date",
        description: "Pin, shift, or clear a node's date, or record an actual start or \
            finish.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<SetDate>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "override",
        description: "Apply or remove an override, with a reason: force include, or keep \
            under a skipped ancestor. A guard bypass is sent with the completion it covers, in \
            `apply_patch`.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema::<OverrideNode>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "resolve_date_conflict",
        description: "Apply one resolution move a rejection or a node's dates listed, then \
            the refused mutations, as one patch.",
        writes: Writes::Patch,
        destructive: true,
        schema: schema_citing_mutations::<ResolveDateConflict>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "create_journey",
        description: "Create a journey, empty or from a published route version.",
        writes: Writes::Patch,
        destructive: false,
        schema: schema::<CreateJourney>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "apply_patch",
        description: "Apply ordered mutations to a journey, a route draft, or the \
            deployment, all or nothing: structure, notes, bulk edits, deletes. \
            Draft a proposal instead when someone should review first.",
        writes: Writes::Patch,
        destructive: true,
        schema: || schema_with_mutation_sketch::<ApplyPatch>(&written_directly),
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "open_draft",
        description: "Open a route's draft for editing; with `create`, create the route \
            first.",
        writes: Writes::Patch,
        destructive: false,
        schema: schema::<OpenDraft>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "publish_draft",
        description: "Publish a route's draft as its next version.",
        writes: Writes::Patch,
        destructive: false,
        schema: schema::<PublishDraft>,
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
            "answer_decision" => parse::<AnswerDecision>(arguments)?.patch(),
            "transition_node" => parse::<TransitionNode>(arguments)?.patch(),
            "assign" => parse::<Assign>(arguments)?.patch(),
            "snooze" => parse::<Snooze>(arguments)?.patch(),
            "unsnooze" => parse::<Unsnooze>(arguments)?.patch(),
            "set_date" => parse::<SetDate>(arguments)?.patch(),
            "override" => parse::<OverrideNode>(arguments)?.patch(),
            "resolve_date_conflict" => parse::<ResolveDateConflict>(arguments)?.patch(),
            "create_journey" => parse::<CreateJourney>(arguments)?.patch(),
            "apply_patch" => parse::<ApplyPatch>(arguments)?.patch(),
            "open_draft" => parse::<OpenDraft>(arguments)?.patch(),
            "publish_draft" => parse::<PublishDraft>(arguments)?.patch(),
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
        let target = PatchTarget::Deployment;
        domain_patch(
            arguments.patch_id,
            target,
            arguments.base_revision,
            None,
            vec![mutation],
            arguments.note,
        )
    }
}

/// A patch of `mutations`, in order, to `target` at `base_revision`.
fn domain_patch(
    id: PatchId,
    target: PatchTarget,
    base_revision: Revision,
    deployment_revision: Option<Revision>,
    mutations: Vec<Mutation>,
    note: Option<Markdown>,
) -> Result<DomainPatch, ToolError> {
    let mutations = Mutations::new(mutations).map_err(|error| ToolError::Arguments {
        path: String::new(),
        message: error.to_string(),
    })?;
    let patch = Patch {
        id,
        target,
        base_revision,
        deployment_revision,
        mutations,
    };
    DomainPatch::new(patch, note).map_err(ToolError::refused)
}

/// One mutation to a journey.
fn journey_patch(
    journey: JourneyId,
    write: Write,
    deployment_revision: Option<Revision>,
    mutation: Mutation,
) -> Result<DomainPatch, ToolError> {
    let target = PatchTarget::Journey(journey);
    domain_patch(
        write.patch_id,
        target,
        write.base_revision,
        deployment_revision,
        vec![mutation],
        write.note,
    )
}

/// What every direct write carries besides its mutations.
struct Write {
    patch_id: PatchId,
    base_revision: Revision,
    note: Option<Markdown>,
}

/// The write `arguments` describe: they carry `patch_id`, `base_revision`, and `note`.
macro_rules! write_of {
    ($arguments:expr) => {
        Write {
            patch_id: $arguments.patch_id,
            base_revision: $arguments.base_revision,
            note: $arguments.note,
        }
    };
}

/// `answer_decision`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnswerDecision {
    journey: JourneyId,
    decision: NodeKey,
    value: AnswerValue,
    /// Markdown. It belongs to this answer alone: repeat the reason when a new answer
    /// keeps it.
    #[serde(default)]
    rationale: Option<Markdown>,
    /// Needed when the answer names identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl AnswerDecision {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::Answer {
            decision: self.decision,
            value: self.value,
            rationale: self.rationale,
        };
        journey_patch(
            self.journey,
            write_of!(self),
            self.deployment_revision,
            mutation,
        )
    }
}

/// `transition_node`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionNode {
    journey: JourneyId,
    node: NodeKey,
    transition: Transition,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl TransitionNode {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::Transition {
            node: self.node,
            transition: self.transition,
        };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `assign`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Assign {
    journey: JourneyId,
    node: NodeKey,
    /// `owner`, or a kind the graph defines.
    kind: KindKey,
    /// A role key, or identity keys; absent clears it.
    #[serde(default)]
    source: Option<ParticipationSource<KeyRefs>>,
    /// Needed when the source names identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl Assign {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let (node, kind) = (self.node, self.kind);
        let mutation = match self.source {
            Some(source) => Mutation::SetParticipation { node, kind, source },
            None => Mutation::ClearParticipation { node, kind },
        };
        journey_patch(
            self.journey,
            write_of!(self),
            self.deployment_revision,
            mutation,
        )
    }
}

/// `snooze`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Snooze {
    journey: JourneyId,
    node: NodeKey,
    until: SnoozeTarget,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl Snooze {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::Snooze {
            node: self.node,
            until: self.until,
        };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `unsnooze`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unsnooze {
    journey: JourneyId,
    node: NodeKey,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl Unsnooze {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::Unsnooze { node: self.node };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `set_date`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetDate {
    journey: JourneyId,
    node: NodeKey,
    change: DateChange,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

/// A change to a node's dates.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DateChange {
    // PRD: F2.
    /// Pin it to a date.
    Pin {
        /// The date.
        date: Date,
    },
    /// Move its pin by days, later when positive.
    ShiftPin {
        /// The days.
        offset_days: SignedDays,
    },
    /// Clear its pin.
    Unpin,
    // PRD: F6.
    /// Record when it actually started or finished.
    Actual {
        /// Its start or its finish.
        end: RecordedEnd,
        /// The date.
        date: Date,
    },
}

impl SetDate {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let node = self.node;
        let mutation = match self.change {
            DateChange::Pin { date } => Mutation::SetPin { node, date },
            DateChange::ShiftPin { offset_days } => Mutation::ShiftPin { node, offset_days },
            DateChange::Unpin => Mutation::ClearPin { node },
            DateChange::Actual { end, date } => Mutation::SetRecordedDate { node, end, date },
        };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `override`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OverrideNode {
    journey: JourneyId,
    node: NodeKey,
    /// The override to apply, or the kind to remove.
    change: OverrideChange,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

/// An override applied or removed.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OverrideChange {
    /// Apply it, with its reason.
    Apply(OverrideApplied),
    /// Remove the override of this kind.
    Remove(OverrideKind),
}

/// An override this tool applies. A guard bypass is sent with the completion it covers, in
/// `apply_patch`: alone it covers nothing.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OverrideApplied {
    /// Treat the node as relevant despite its conditions.
    ForceInclude {
        /// Why.
        reason: cairn_schema::Reason,
    },
    /// Exempt the node's subtree from an ancestor's skip.
    Keep {
        /// Why.
        reason: cairn_schema::Reason,
    },
}

impl From<OverrideApplied> for cairn_schema::Override {
    fn from(applied: OverrideApplied) -> Self {
        match applied {
            OverrideApplied::ForceInclude { reason } => Self::ForceInclude { reason },
            OverrideApplied::Keep { reason } => Self::Keep { reason },
        }
    }
}

impl OverrideNode {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let node = self.node;
        let mutation = match self.change {
            OverrideChange::Apply(applied) => Mutation::ApplyOverride {
                node,
                applied: applied.into(),
            },
            OverrideChange::Remove(kind) => Mutation::RemoveOverride { node, kind },
        };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `resolve_date_conflict`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveDateConflict {
    journey: JourneyId,
    /// One move, as the chain listed it.
    resolution: Mutation,
    /// The refused mutations, applied after the move.
    #[serde(default)]
    then: Vec<Mutation>,
    /// Needed when the move or the mutations name identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl ResolveDateConflict {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        // F5: the moves a chain lists
        // (decisions/2026-10-06-chains-gain-answer-and-today-fixers-resolution-moves.md).
        let move_kind = matches!(
            self.resolution,
            Mutation::ShiftPin { .. }
                | Mutation::SetPin { .. }
                | Mutation::ClearPin { .. }
                | Mutation::Answer { .. }
                | Mutation::SetNodeField { .. }
                | Mutation::RemoveEdge { .. }
        );
        if !move_kind {
            return Err(ToolError::Arguments {
                path: "resolution.op".to_owned(),
                message: "a resolution is one of the moves a chain lists: shift_pin, set_pin, \
                          clear_pin, answer, set_node_field, or remove_edge"
                    .to_owned(),
            });
        }
        let mut mutations = vec![self.resolution];
        mutations.extend(self.then);
        let target = PatchTarget::Journey(self.journey);
        let write = write_of!(self);
        domain_patch(
            write.patch_id,
            target,
            write.base_revision,
            self.deployment_revision,
            mutations,
            write.note,
        )
    }
}

/// `create_journey`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateJourney {
    /// The new journey's id.
    journey: JourneyId,
    patch_id: PatchId,
    name: Title,
    #[serde(default)]
    description: Option<Markdown>,
    /// Identity keys; nobody by default, the creator included.
    #[serde(default)]
    owners: IdentitySet,
    /// Needed when owners are named.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// A published route version; empty when absent.
    #[serde(default)]
    from: Option<Lineage>,
    #[serde(default)]
    note: Option<Markdown>,
}

impl CreateJourney {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::CreateJourney {
            name: self.name,
            description: self.description,
            owners: self.owners,
            from: self.from,
        };
        let target = PatchTarget::Journey(self.journey);
        domain_patch(
            self.patch_id,
            target,
            Revision::NONE,
            self.deployment_revision,
            vec![mutation],
            self.note,
        )
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

/// `apply_patch`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyPatch {
    patch: DirectPatch,
    #[serde(default)]
    note: Option<Markdown>,
}

// PRD: A17.
/// The patch: its id, its target, the base revision, and its mutations in order.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DirectPatch {
    id: PatchId,
    /// A domain; a proposal is written with the proposal tools.
    target: Domain,
    /// 0 creates the target.
    base_revision: Revision,
    /// Needed when a journey write names identities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    mutations: Mutations,
}

impl ApplyPatch {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let DirectPatch {
            id,
            target,
            base_revision,
            deployment_revision,
            mutations,
        } = self.patch;
        refuse_proposal_ops("patch.mutations", &mutations)?;
        let patch = Patch {
            id,
            target: target.into(),
            base_revision,
            deployment_revision,
            mutations,
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

/// `open_draft`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpenDraft {
    route: RouteId,
    /// Create the route first, with this name and description (base revision 0).
    #[serde(default)]
    create: Option<NewRoute>,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

/// A route to create.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewRoute {
    /// Its name.
    name: Title,
    /// What it is for.
    #[serde(default)]
    description: Option<Markdown>,
    /// `process` (the default) or `segment`: a reusable piece that is inserted, never started.
    #[serde(default)]
    kind: cairn_schema::RouteKind,
}

impl OpenDraft {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mut mutations: Vec<Mutation> = self
            .create
            .into_iter()
            .map(|route| Mutation::CreateRoute {
                name: route.name,
                description: route.description,
                kind: route.kind,
            })
            .collect();
        mutations.push(Mutation::OpenDraft {
            source: cairn_schema::DraftSource::Edit,
        });
        let write = write_of!(self);
        let target = PatchTarget::Route(self.route);
        domain_patch(
            write.patch_id,
            target,
            write.base_revision,
            None,
            mutations,
            write.note,
        )
    }
}

/// `publish_draft`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PublishDraft {
    route: RouteId,
    patch_id: PatchId,
    base_revision: Revision,
    #[serde(default)]
    note: Option<Markdown>,
}

impl PublishDraft {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let target = PatchTarget::Route(self.route);
        let write = write_of!(self);
        domain_patch(
            write.patch_id,
            target,
            write.base_revision,
            None,
            vec![Mutation::PublishDraft {}],
            write.note,
        )
    }
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

    /// What `apply_patch` lists is what it takes: an op is left out exactly when its
    /// mutations change a proposal.
    #[test]
    fn apply_patch_lists_the_ops_it_takes() {
        let patch: Patch = cairn_schema::from_yaml(EVERY_MUTATION).expect("every mutation");
        for mutation in patch.mutations.as_slice() {
            let op = serde_json::to_value(mutation).expect("a mutation")["op"].clone();
            let op = op.as_str().expect("an op");
            assert_eq!(written_directly(op), !mutation.edits_a_proposal(), "{op}");
        }
    }
}
