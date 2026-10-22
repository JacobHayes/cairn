//! The derived reads' wire shapes (ARCHITECTURE, HTTP API: Resources; Engine > Projections):
//! each projection is the engine's schema type unchanged, answered with the revisions and the
//! today it was derived from, so a client compares a revision tick with what it holds (H6).
//! Node detail (C8) and history (J4) are the service's own shapes given serde here.

use std::collections::BTreeSet;

use cairn_schema::{
    Annotation, AnswerEffects, AnswerValue, Date, DisplayState, KeyRefs, LocalEdit, Markdown,
    MineEntry, Node, NodeDerived, NodeKey, NodeKind, NodeState, Overrides, PatchEvents, Path,
    Revision, State, StillWaiting, Title,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// PRD: D3.
/// A projection with what it was derived from (computed per read, never stored).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Projected{T}")]
pub struct Projected<T> {
    // PRD: H6.
    /// The journey's revision it was derived from: a tick newer than this means refetch.
    pub revision: Revision,
    // PRD: E6.
    /// The deployment revision it was derived over.
    pub deployment_revision: Revision,
    // PRD: A9.
    /// The today it was derived for, in the deployment's zone.
    pub today: Date,
    /// The projection.
    pub value: T,
}

impl<T> Projected<T> {
    /// The service's projection, its value given its wire shape.
    pub fn from_service<U: Into<T>>(projected: crate::Projected<U>) -> Self {
        let crate::Projected {
            revision,
            deployment_revision,
            today,
            value,
        } = projected;
        Self {
            revision,
            deployment_revision,
            today,
            value: value.into(),
        }
    }
}

// PRD: E4.
/// The nodes the caller participates in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mine {
    /// The nodes, in key order, each with the participation kinds the caller holds on it.
    pub entries: Vec<MineEntry>,
}

impl From<Vec<MineEntry>> for Mine {
    fn from(entries: Vec<MineEntry>) -> Self {
        Self { entries }
    }
}

// PRD: C8.
/// One node in full. Its explanation lists (`derived.gravity_from`, `unlocks_from`)
/// carry their largest entries up to `explanation_entry_count_max` with their totals; the rest
/// page through the node's explanations endpoint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeDetail {
    /// The node: description, resources, participations as written, and its kind's payload.
    pub node: Node<KeyRefs>,
    /// Where it sits.
    pub path: Path,
    /// Its parent, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeKey>,
    /// Its children in key order, each with its state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ChildEntry>,
    /// Its stored state, provenance, and actual dates.
    pub record: NodeState,
    // PRD: B4.
    /// What the journey edited on it, for a route-copied node.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub local_edits: BTreeSet<LocalEdit>,
    // PRD: G5.
    /// The checklist items ticked.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub ticks: BTreeSet<cairn_schema::Slug>,
    /// A decision's answer, as recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerValue>,
    // PRD: B2.
    /// The rationale the recorded answer was given with, markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<Markdown>,
    // PRD: F2.
    /// Its pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Date>,
    // PRD: D4.
    /// Its overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Overrides>,
    // PRD: G1.
    /// The notes and links on it, in key order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    // PRD: D3.
    /// Every derived value with what explains it.
    pub derived: NodeDerived,
    // PRD: C8.
    /// The direct dependents completing it would not yet free, each with what else it waits
    /// on (Priority: Unlocks): the largest entries with the total; page the rest with
    /// the explanations endpoint, field `still_waiting`.
    pub still_waiting: StillWaiting,
    // PRD: C12.
    /// A decision's effects per choice: what each answer brings in, drops, and leaves
    /// to be decided later, with the role it fills and the milestone it pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_effects: Option<AnswerEffects>,
}

/// One child in a node's detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChildEntry {
    /// The child.
    pub key: NodeKey,
    /// Its title.
    pub title: Title,
    /// Its kind.
    pub kind: NodeKind,
    /// Its stored state.
    pub state: State,
    // PRD: D8.
    /// The state every surface shows for it.
    pub display_state: DisplayState,
}

impl From<crate::NodeDetail> for NodeDetail {
    fn from(detail: crate::NodeDetail) -> Self {
        let crate::NodeDetail {
            node,
            path,
            parent,
            children,
            record,
            local_edits,
            ticks,
            answer,
            rationale,
            pin,
            overrides,
            annotations,
            derived,
            still_waiting,
            answer_effects,
        } = detail;
        let children = children.into_iter().map(|child| {
            let crate::ChildEntry {
                key,
                title,
                kind,
                state,
                display_state,
            } = child;
            ChildEntry {
                key,
                title,
                kind,
                state,
                display_state,
            }
        });
        Self {
            node,
            path,
            parent,
            children: children.collect(),
            record,
            local_edits,
            ticks,
            answer,
            rationale,
            pin,
            overrides,
            annotations,
            derived,
            still_waiting,
            answer_effects,
        }
    }
}

// PRD: J4.
/// A page of a journey's history, or of one node's, grouped by patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// At most a page of events, grouped by patch, in log order; a large patch spans pages.
    pub patches: Vec<PatchEvents>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

impl From<crate::History> for History {
    fn from(history: crate::History) -> Self {
        Self {
            patches: history.patches,
            next: history.next,
        }
    }
}
