//! The derived reads' wire shapes (ARCHITECTURE, HTTP API: Resources; Engine > Projections):
//! each projection is the engine's schema type unchanged, answered with the revisions and the
//! today it was derived from, so a client compares a revision tick with what it holds (H6).
//! Node detail (C8) and history (J4) are the service's own shapes given serde here.

use std::collections::BTreeSet;

use cairn_schema::{
    Annotation, AnswerEffects, AnswerValue, Date, DisplayState, KeyRefs, LocalEdit, MineEntry,
    Node, NodeDerived, NodeKey, NodeKind, NodeState, Overrides, PatchEvents, Path, Revision, State,
    StillWaiting, Title,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A projection with what it was derived from (D3: computed per read, never stored).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Projected{T}")]
pub struct Projected<T> {
    /// The journey's revision it was derived from: a tick newer than this means refetch (H6).
    pub revision: Revision,
    /// The deployment revision it was derived over (E6).
    pub deployment_revision: Revision,
    /// The today it was derived for, in the deployment's zone (A9).
    pub today: Date,
    /// The projection.
    pub value: T,
}

impl<T> Projected<T> {
    /// The service's projection, its value given its wire shape.
    pub fn from_service<U: Into<T>>(projected: cairn_service::Projected<U>) -> Self {
        let cairn_service::Projected {
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

/// E4: the nodes the caller participates in.
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

/// C8: one node in full. Its explanation lists (`derived.gravity_from`, `leverage_from`)
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
    /// What the journey edited on it, for a route-copied node (B4).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub local_edits: BTreeSet<LocalEdit>,
    /// A decision's answer, as recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerValue>,
    /// Its pin (F2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Date>,
    /// Its overrides (D4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Overrides>,
    /// The notes and links on it (G1), in key order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
    /// Every derived value (D3) with what explains it.
    pub derived: NodeDerived,
    /// The direct dependents completing it would not yet free, each with what else it waits
    /// on (C8, Priority: Leverage): the largest entries with the total; page the rest with
    /// the explanations endpoint, field `still_waiting`.
    pub still_waiting: StillWaiting,
    /// A decision's effects per choice (C12): what each answer brings in, drops, and leaves
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
    /// D8: the state every surface shows for it.
    pub display_state: DisplayState,
}

impl From<cairn_service::NodeDetail> for NodeDetail {
    fn from(detail: cairn_service::NodeDetail) -> Self {
        let cairn_service::NodeDetail {
            node,
            path,
            parent,
            children,
            record,
            local_edits,
            answer,
            pin,
            overrides,
            annotations,
            derived,
            still_waiting,
            answer_effects,
        } = detail;
        let children = children.into_iter().map(|child| {
            let cairn_service::ChildEntry {
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
            answer,
            pin,
            overrides,
            annotations,
            derived,
            still_waiting,
            answer_effects,
        }
    }
}

/// J4: a page of a journey's history, or of one node's, grouped by patch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// At most a page of events, grouped by patch, in log order; a large patch spans pages.
    pub patches: Vec<PatchEvents>,
    /// Pass as `after` for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

impl From<cairn_service::History> for History {
    fn from(history: cairn_service::History) -> Self {
        Self {
            patches: history.patches,
            next: history.next,
        }
    }
}
