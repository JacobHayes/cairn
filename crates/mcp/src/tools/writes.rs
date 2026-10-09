//! The direct-write tools: each call is one patch (A17), named by a client-generated patch id
//! so a lost response can be resubmitted safely (H5), against the base revision the agent
//! read. A journey's state tools (answer, transition, snooze, assign, dates, overrides) are
//! one mutation each; `apply_patch` takes any domain patch; the route tools open and publish
//! a draft and import a route file; `manage_entity` writes the deployment.

use std::collections::BTreeMap;

use cairn_schema::{
    AnswerValue, Date, Entity, EntityKey, JourneyId, KeyRefs, KindKey, Lineage, Markdown, Mutation,
    Mutations, NodeKey, OverrideKind, ParticipationSource, Patch, PatchId, PatchTarget,
    RecordedEnd, Revision, RouteFile, RouteId, SignedDays, SnoozeTarget, Title, Transition,
    from_yaml,
};
use cairn_service::{Call, DomainPatch};
use cairn_store::{JourneyQuery, PageSize, Store};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::WriteOutput;
use crate::toolset::{
    Spec, output, parse, schema, schema_citing_mutations, schema_citing_review_items,
};
use crate::{ToolError, ToolSet};

/// The direct-write tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "answer_decision",
        description: "Records a decision's answer (A8): a choice, choices, a boolean, text, a \
            date, an entity, or entities, matching the decision's answer type. Answering \
            changes what is relevant, fills roles, and pins dates; the result lists the \
            consequences: warnings, and what the answer unlocked, took out of scope, or \
            brought into scope. Name the deployment revision when the answer names entities.",
        writes: true,
        destructive: true,
        schema: schema::<AnswerDecision>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "transition_node",
        description: "Moves a node through its lifecycle: start, stop, complete, skip (with a \
            reason), reopen, or reach (a milestone). Guards are checked against the result.",
        writes: true,
        destructive: true,
        schema: schema::<TransitionNode>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "assign",
        description: "Sets who holds a participation kind (owner, or one of the graph's own) on \
            a node: a role, or explicit entities (an empty list is an explicit none). Omit \
            `source` to clear it, so the node inherits again (E2).",
        writes: true,
        destructive: true,
        schema: schema::<Assign>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "snooze",
        description: "Hides a node from the frontier until a date, or until another node is \
            done (B6). A container (a group, or work with children) with open work beneath it \
            sets its whole subtree aside, each descendant naming it as `snoozed_via`; a \
            target inside the subtree, or depending on anything in it, is rejected.",
        writes: true,
        destructive: true,
        schema: schema::<Snooze>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "unsnooze",
        description: "Lifts a node's own snooze. A node held only through a container's \
            snooze (`snoozed_via`) is refused, naming the container to unsnooze instead.",
        writes: true,
        destructive: true,
        schema: schema::<Unsnooze>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "set_date",
        description: "Pins a node to a date, shifts or clears its pin (F2), or records an \
            actual start or finish date (F6).",
        writes: true,
        destructive: true,
        schema: schema::<SetDate>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "override",
        description: "Applies an override to a node with a reason (D4): force include it \
            despite its conditions, keep it under a skipped ancestor, or bypass named guards. \
            Or removes one.",
        writes: true,
        destructive: true,
        schema: schema::<OverrideNode>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "resolve_date_conflict",
        description: "Resolves a contradictory date chain or a shortfall (F5, F6) by applying \
            one of the resolution moves a rejection or a node's dates listed (shift, repin, or \
            unpin a pin; revise a date answer; loosen a rule's offset; shrink an estimate; drop \
            a requirement), then, in the same patch, the mutations that were refused.",
        writes: true,
        destructive: true,
        schema: schema_citing_mutations::<ResolveDateConflict>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "create_journey",
        description: "Creates a journey, empty or from a published route version (B1), as the \
            first patch against its new id (base revision 0).",
        writes: true,
        destructive: false,
        schema: schema::<CreateJourney>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "apply_patch",
        description: "Applies any patch to a journey, a route (its draft included), or the \
            deployment (A17): ordered mutations against a base revision, all or nothing. \
            Structure (nodes, edges, roles, kinds, resources), notes, and bulk changes go \
            here; a proposal is better when someone should review first.",
        writes: true,
        destructive: true,
        schema: schema_citing_review_items::<ApplyPatch>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "open_draft",
        description: "Opens a route's draft for editing (A11), creating the route first when \
            `create` is given (base revision 0). Edit the draft with `apply_patch` or a \
            proposal on the route, then publish it.",
        writes: true,
        destructive: false,
        schema: schema::<OpenDraft>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "publish_draft",
        description: "Publishes a route's draft as its next version (A11); journeys on older \
            versions can then upgrade.",
        writes: true,
        destructive: false,
        schema: schema::<PublishDraft>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "import_route",
        description: "Imports a route file (YAML) as a new route, or as a new draft of an \
            existing one matched by key or path (A13). Every violation in the file is listed.",
        writes: true,
        destructive: false,
        schema: schema::<ImportRoute>,
        output: schema::<WriteOutput>,
    },
    Spec {
        name: "manage_entity",
        description: "Creates an entity (a person or team journeys refer to), edits its name \
            or emails (an email links it to the user who signs in with it, H3), or merges a \
            duplicate into another (E6). A deployment patch.",
        writes: true,
        destructive: true,
        schema: schema::<ManageEntity>,
        output: schema::<WriteOutput>,
    },
];

impl<S: Store + 'static> ToolSet<S> {
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
            "manage_entity" => self.entity_patch(parse(arguments)?).await,
            _ => return Ok(None),
        }?;
        Ok(Some(drafted))
    }

    /// `import_route`: one route patch (A13).
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

    /// `manage_entity`: a merge names every journey referring to either entity at the
    /// revision it was checked against (E6), which the tool reads from the journey index.
    async fn entity_patch(&self, arguments: ManageEntity) -> Result<DomainPatch, ToolError> {
        let mutation = match arguments.change {
            EntityChange::Create { entity } => Mutation::CreateEntity { entity },
            EntityChange::Edit { entity } => Mutation::EditEntity { entity },
            EntityChange::Merge { survivor, merged } => {
                let referencing = [survivor.clone(), merged.clone()].into();
                let journeys = self.revisions_referencing(referencing).await?;
                Mutation::MergeEntities {
                    survivor,
                    merged,
                    journeys,
                }
            }
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

    /// Every journey referring to any of `entities`, directly or through an alias, at its
    /// current revision: one index read per page of journeys.
    async fn revisions_referencing(
        &self,
        entities: std::collections::BTreeSet<EntityKey>,
    ) -> Result<BTreeMap<JourneyId, Revision>, ToolError> {
        let mut found = BTreeMap::new();
        let mut query = JourneyQuery {
            referencing: Some(entities),
            size: PageSize::MAX,
            ..JourneyQuery::default()
        };
        loop {
            let page = self.service.journeys(&query).await?;
            let count = found.len();
            found.extend(
                page.items
                    .into_iter()
                    .map(|summary| (summary.id, summary.revision)),
            );
            match page.next {
                Some(after) => {
                    // The index pages strictly forward, so this ends.
                    assert!(found.len() > count, "a page that continues holds a journey");
                    query.after = Some(after);
                }
                None => return Ok(found),
            }
        }
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
    /// The journey.
    journey: JourneyId,
    /// The decision.
    decision: NodeKey,
    /// The answer.
    value: AnswerValue,
    /// Why it was given, markdown. It belongs to this answer alone: a revision that gives
    /// none leaves the decision without one, so repeat the reason when it still holds.
    #[serde(default)]
    rationale: Option<Markdown>,
    /// The deployment revision the named entities were read at; needed when the answer
    /// names an entity.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The journey.
    journey: JourneyId,
    /// The node.
    node: NodeKey,
    /// The transition.
    transition: Transition,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The journey.
    journey: JourneyId,
    /// The node.
    node: NodeKey,
    /// The participation kind: `owner`, or one the graph defines.
    kind: KindKey,
    /// A role's key, or a list of entity keys; absent clears it.
    #[serde(default)]
    source: Option<ParticipationSource<KeyRefs>>,
    /// The deployment revision the named entities were read at; needed when the source
    /// names entities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The journey.
    journey: JourneyId,
    /// The node: an actionable node, or a container (a group, or a deliverable or action
    /// with children) with open work beneath it, which sets its whole subtree aside while
    /// the snooze holds. A target inside the subtree, or one that depends on anything in it,
    /// is rejected.
    node: NodeKey,
    /// Until a date, or until a node is done.
    until: SnoozeTarget,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The journey.
    journey: JourneyId,
    /// The node. One snoozed only through a container (its `snoozed_via`) is refused,
    /// naming the container: unsnooze the container instead.
    node: NodeKey,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The journey.
    journey: JourneyId,
    /// The node.
    node: NodeKey,
    /// The change.
    change: DateChange,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
    #[serde(default)]
    note: Option<Markdown>,
}

/// A change to a node's dates.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DateChange {
    /// Pin it to a date (F2).
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
    /// Record when it actually started or finished (F6).
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
    /// The journey.
    journey: JourneyId,
    /// The node.
    node: NodeKey,
    /// The override to apply, or the kind to remove.
    change: OverrideChange,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
    #[serde(default)]
    note: Option<Markdown>,
}

/// An override applied or removed.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OverrideChange {
    /// Apply it, with its reason.
    Apply(cairn_schema::Override),
    /// Remove the override of this kind.
    Remove(OverrideKind),
}

impl OverrideNode {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let node = self.node;
        let mutation = match self.change {
            OverrideChange::Apply(applied) => Mutation::ApplyOverride { node, applied },
            OverrideChange::Remove(kind) => Mutation::RemoveOverride { node, kind },
        };
        journey_patch(self.journey, write_of!(self), None, mutation)
    }
}

/// `resolve_date_conflict`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveDateConflict {
    /// The journey.
    journey: JourneyId,
    /// One resolution move, as the chain listed it.
    resolution: Mutation,
    /// The mutations the conflict refused, applied after the resolution in the same patch.
    #[serde(default)]
    then: Vec<Mutation>,
    /// The deployment revision, when the resolution or the mutations name entities.
    #[serde(default)]
    deployment_revision: Option<Revision>,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// The new journey's id (`j_` and a slug).
    journey: JourneyId,
    /// A new id for this write.
    patch_id: PatchId,
    /// Its name.
    name: Title,
    /// What it is for.
    #[serde(default)]
    description: Option<Markdown>,
    /// The published route version to start from; empty when absent.
    #[serde(default)]
    from: Option<Lineage>,
    /// A note recorded on the write's events.
    #[serde(default)]
    note: Option<Markdown>,
}

impl CreateJourney {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mutation = Mutation::CreateJourney {
            name: self.name,
            description: self.description,
            from: self.from,
        };
        let target = PatchTarget::Journey(self.journey);
        domain_patch(
            self.patch_id,
            target,
            Revision::NONE,
            None,
            vec![mutation],
            self.note,
        )
    }
}

/// `apply_patch`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ApplyPatch {
    /// The patch: its id, its target (a journey, a route, or the deployment), the base
    /// revision, and its mutations in order.
    patch: Patch,
    /// A note recorded on the patch's events.
    #[serde(default)]
    note: Option<Markdown>,
}

impl ApplyPatch {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        DomainPatch::new(self.patch, self.note).map_err(|refusal| {
            ToolError::refused(format_args!("{refusal}: use the proposal tools"))
        })
    }
}

/// `open_draft`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OpenDraft {
    /// The route (a slug).
    route: RouteId,
    /// Create the route first, with this name and description (base revision 0).
    #[serde(default)]
    create: Option<NewRoute>,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
}

impl OpenDraft {
    fn patch(self) -> Result<DomainPatch, ToolError> {
        let mut mutations: Vec<Mutation> = self
            .create
            .into_iter()
            .map(|route| Mutation::CreateRoute {
                name: route.name,
                description: route.description,
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
    /// The route.
    route: RouteId,
    /// A new id for this write, unique to it (`p_` and a slug). Resubmitting the same id with
    /// the same content is answered from its receipt, so a lost response is safe to retry.
    patch_id: PatchId,
    /// The revision the write was drafted against: the `revision` of the read it follows,
    /// or the receipt's of the write before it. A write against an older revision is
    /// rejected as stale, naming what moved.
    base_revision: Revision,
    /// A note recorded on the write's events: why.
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
    /// A new id for this write; the import's new keys are minted from it.
    patch_id: PatchId,
    /// The route file, in YAML (or JSON): `route`, `name`, and its roles, kinds, and nodes,
    /// naming the published version it `extends` to import a new draft of an existing route.
    file: String,
    /// A note recorded on the import's events.
    #[serde(default)]
    note: Option<Markdown>,
}

/// `manage_entity`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManageEntity {
    /// The change.
    change: EntityChange,
    /// A new id for this write.
    patch_id: PatchId,
    /// The deployment revision it was drafted against.
    base_revision: Revision,
    /// A note recorded on the write's events.
    #[serde(default)]
    note: Option<Markdown>,
}

/// A change to the deployment's entities (E6).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EntityChange {
    /// Create an entity with its key (`e_` and a slug).
    Create {
        /// The entity.
        entity: Entity,
    },
    /// Replace an entity's name and emails.
    Edit {
        /// The entity as it should be.
        entity: Entity,
    },
    /// Merge `merged` into `survivor`; `merged`'s key becomes an alias of it.
    Merge {
        /// The entity that remains.
        survivor: EntityKey,
        /// The duplicate.
        merged: EntityKey,
    },
}
