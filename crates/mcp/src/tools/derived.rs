//! The derived reads (I3, C2, C8, C9, C10): the snapshot an agent reads first, a canvas
//! level, one node in full, and the ranked frontier and its filtered lists. Each is derived
//! at the caller's today with the caller's entities as the viewer (H3), and answers the
//! revision it was derived at.

use std::collections::BTreeSet;

use cairn_schema::{
    Annotation, AnswerValue, Cursor, Date, ExplainedField, ExplanationPage, JourneyId, KeyRefs,
    Level, LevelEdge, LevelNode, ListFlag, ListQuery, LocalEdit, Next, NextQuery, Node,
    NodeDerived, NodeKey, NodeKind, NodeRow, NodeState, Overrides, Path, Snapshot, SnapshotScope,
    SortBy, Stalled,
};
use cairn_service::{Call, ChildEntry, NodeDetail};
use cairn_store::Store;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{At, Paged, page};
use crate::toolset::{Spec, output, parse, schema};
use crate::{ToolError, ToolSet};

/// The derived read tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "get_snapshot",
        description: "Call this first. The bounded state of a journey (I3): the answers in \
            effect, a page of in-scope nodes with state, owners, blocking, and dates, the \
            ranked acting frontier's top items with the rest as keys, open decisions by rank, \
            placeholders needing breakdown, unassigned items, shortfalls, and counts. Scope it \
            to a subtree and depth; page the node list with `cursor`. Its `revision` is the \
            `base_revision` of a write that follows.",
        writes: false,
        destructive: false,
        schema: schema::<GetSnapshot>,
        output: schema::<At<SnapshotOutput>>,
    },
    Spec {
        name: "list_frontier",
        description: "The ranked acting frontier, a page at a time: what to do next, each \
            node with its breadcrumb and why it ranks where it does. With `filters`, the \
            in-scope nodes they all hold for instead (decisions needed, needs breakdown, \
            unassigned, active, blocked, stale, overdue, shortfall), in rank order; `mine` \
            keeps the caller's own.",
        writes: false,
        destructive: false,
        schema: schema::<ListFrontier>,
        output: schema::<At<FrontierOutput>>,
    },
    Spec {
        name: "get_node",
        description: "One node in full: as written (a decision's prompt and choices), its \
            stored state, answer, pin, overrides, notes, a page of its children, and every \
            derived value with its explanation (relevance, blocking, dates, gravity, \
            leverage, rank). Explanation lists hold their largest entries; pass \
            `explanations` to page the rest.",
        writes: false,
        destructive: false,
        schema: schema::<GetNode>,
        output: schema::<At<NodeOutput>>,
    },
    Spec {
        name: "get_level",
        description: "One aggregation level of the journey's graph (C2): the nodes of the \
            shown kinds at the top or inside a container, hidden work rolled up into them, \
            and the edges between them. Nodes are paged; edges are those touching the page.",
        writes: false,
        destructive: false,
        schema: schema::<GetLevel>,
        output: schema::<At<LevelOutput>>,
    },
];

/// `get_snapshot` (I3).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetSnapshot {
    /// The journey.
    journey: JourneyId,
    /// Only this node and everything beneath it; the whole journey when absent.
    #[serde(default)]
    subtree: Option<NodeKey>,
    /// How many levels below the subtree's node (or of roots, at depth 1) the node list
    /// goes; every level when absent. The frontier and the lists ignore it.
    #[serde(default)]
    depth: Option<u32>,
    /// Where the node list's page starts: the `next` of the page before.
    #[serde(default)]
    cursor: Cursor,
}

/// The snapshot, at its revision.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct SnapshotOutput {
    /// The bounded view: answers, a page of in-scope nodes, the acting frontier's top
    /// `page_item_count_max` with the rest as keys, open decisions by rank, placeholders
    /// needing breakdown, unassigned items, shortfalls, and counts.
    snapshot: Snapshot,
}

/// `get_level` (C2).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetLevel {
    /// The journey.
    journey: JourneyId,
    /// The node kinds shown; every kind when empty.
    #[serde(default)]
    kinds: BTreeSet<NodeKind>,
    /// The container drilled into; the top level when absent.
    #[serde(default)]
    container: Option<NodeKey>,
    /// Where the page of visible nodes starts.
    #[serde(default)]
    cursor: Cursor,
}

/// A page of one canvas level.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LevelOutput {
    /// The container drilled into.
    #[serde(skip_serializing_if = "Option::is_none")]
    container: Option<NodeKey>,
    /// The kinds shown.
    shown: BTreeSet<NodeKind>,
    /// A page of the visible nodes, in tree order, with their roll-ups.
    nodes: Paged<LevelNode>,
    /// The level's edges with an end among this page's nodes.
    edges: Vec<LevelEdge>,
}

/// `get_node` (C8).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetNode {
    /// The journey.
    journey: JourneyId,
    /// The node.
    node: NodeKey,
    /// Instead of the node, a further page of one of its explanation lists, which the
    /// node's detail cuts to its largest entries.
    #[serde(default)]
    explanations: Option<MoreExplanations>,
    /// Where the page of its children starts: the `next` of their page before.
    #[serde(default)]
    children_cursor: Cursor,
    /// Where the page of its notes and links starts: the `next` of their page before.
    #[serde(default)]
    annotations_cursor: Cursor,
}

/// Which explanation list to continue, and from where.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MoreExplanations {
    /// The derived value whose explanation list to page.
    field: ExplainedField,
    /// Where the page starts.
    cursor: Cursor,
}

/// One node in full, or a page of an explanation list: exactly one is present.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct NodeOutput {
    /// The node's detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Box<Detail>>,
    /// A page of an explanation list.
    #[serde(skip_serializing_if = "Option::is_none")]
    explanations: Option<ExplanationPage>,
}

/// C8: what a node is, its stored state and records, and every derived value with what
/// explains it (gravity and leverage contributions cut to the largest, with totals).
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Detail {
    /// The node as written: description, resources, participations, and its kind's payload
    /// (a decision's prompt and choices).
    node: Node<KeyRefs>,
    /// Where it sits.
    path: Path,
    /// Its parent.
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<NodeKey>,
    /// A page of its children in key order, each with its state; pass `next` as
    /// `children_cursor` for the next page.
    children: Paged<Child>,
    /// Its stored state, provenance, and actual dates.
    record: NodeState,
    /// What the journey edited on it (B4).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    local_edits: BTreeSet<LocalEdit>,
    /// A decision's answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<AnswerValue>,
    /// Its pin (F2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pin: Option<Date>,
    /// Its overrides (D4).
    #[serde(skip_serializing_if = "Option::is_none")]
    overrides: Option<Overrides>,
    /// A page of its notes and links, in key order; pass `next` as `annotations_cursor` for
    /// the next page.
    annotations: Paged<Annotation>,
    /// Every derived value (D3) with its explanation: relevance, blocking, dates, gravity,
    /// leverage, and rank.
    derived: NodeDerived,
}

/// One child of a node.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Child {
    key: NodeKey,
    title: cairn_schema::Title,
    kind: NodeKind,
    state: cairn_schema::State,
}

impl Detail {
    /// The service's detail, its children and notes paged from their cursors.
    fn of(detail: NodeDetail, children_cursor: Cursor, annotations_cursor: Cursor) -> Self {
        let NodeDetail {
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
        } = detail;
        let children = children.into_iter().map(|child| {
            let ChildEntry {
                key,
                title,
                kind,
                state,
            } = child;
            Child {
                key,
                title,
                kind,
                state,
            }
        });
        Self {
            node,
            path,
            parent,
            children: page(children.collect(), children_cursor),
            record,
            local_edits,
            answer,
            pin,
            overrides,
            annotations: page(annotations, annotations_cursor),
            derived,
        }
    }
}

/// `list_frontier` (C9, C10, I2).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListFrontier {
    /// The journey.
    journey: JourneyId,
    /// With none, the acting frontier in rank order: what to do next. With any, the in-scope
    /// nodes every filter holds for, in rank order.
    #[serde(default)]
    filters: BTreeSet<FrontierFilter>,
    /// Only nodes the caller participates in (E4).
    #[serde(default)]
    mine: bool,
    /// Only these node kinds; every kind when empty.
    #[serde(default)]
    kinds: BTreeSet<NodeKind>,
    /// Only this node and the nodes beneath it.
    #[serde(default)]
    within: Option<NodeKey>,
    /// Rank the frontier for the caller ("prioritize for me"); with no filters only.
    #[serde(default)]
    for_viewer: bool,
    /// Where the page starts: the `next` of the page before, at the same revision.
    #[serde(default)]
    cursor: Cursor,
}

/// What `list_frontier` filters by (I2's lists).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrontierFilter {
    /// Actionable decisions.
    DecisionsNeeded,
    /// Placeholders to break down (B10).
    NeedsBreakdown,
    /// No owner.
    Unassigned,
    /// Started and not finished.
    Active,
    /// Waiting on something unfinished.
    Blocked,
    /// Done, but a completing guard would now fail (D4).
    Stale,
    /// Due before today.
    Overdue,
    /// A plan that can no longer be met (F6).
    Shortfall,
}

impl From<FrontierFilter> for ListFlag {
    fn from(filter: FrontierFilter) -> Self {
        match filter {
            FrontierFilter::DecisionsNeeded => ListFlag::DecisionsNeeded,
            FrontierFilter::NeedsBreakdown => ListFlag::NeedsBreakdown,
            FrontierFilter::Unassigned => ListFlag::Unassigned,
            FrontierFilter::Active => ListFlag::Active,
            FrontierFilter::Blocked => ListFlag::Blocked,
            FrontierFilter::Stale => ListFlag::Stale,
            FrontierFilter::Overdue => ListFlag::Overdue,
            FrontierFilter::Shortfall => ListFlag::Shortfall,
        }
    }
}

/// A page of the frontier or of a filtered list, in rank order.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrontierOutput {
    /// The page: each node with its breadcrumb, signals, and rank terms.
    #[serde(flatten)]
    rows: Paged<NodeRow>,
    /// When the frontier is empty and work remains, what the journey waits on (C5, D5).
    #[serde(skip_serializing_if = "Option::is_none")]
    stalled: Option<Stalled>,
}

impl<S: Store + 'static> ToolSet<S> {
    /// Runs the derived read tool `name`.
    pub(crate) async fn derive(
        &self,
        call: &Call,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        match name {
            "get_snapshot" => output(self.get_snapshot(call, parse(arguments)?).await),
            "list_frontier" => output(self.list_frontier(call, parse(arguments)?).await),
            "get_node" => output(self.get_node(call, parse(arguments)?).await),
            "get_level" => output(self.get_level(call, parse(arguments)?).await),
            _ => Err(ToolError::UnknownTool {
                name: name.to_owned(),
            }),
        }
    }

    /// I3: `get_snapshot`.
    async fn get_snapshot(
        &self,
        call: &Call,
        arguments: GetSnapshot,
    ) -> Result<At<SnapshotOutput>, ToolError> {
        let scope = SnapshotScope {
            subtree: arguments.subtree,
            depth: arguments.depth,
            cursor: arguments.cursor,
        };
        let projected = self
            .service
            .snapshot(call, &arguments.journey, &scope)
            .await?;
        Ok(At::of(projected, |snapshot| SnapshotOutput { snapshot }))
    }

    /// C2: `get_level`, its nodes paged and its edges those touching the page.
    async fn get_level(
        &self,
        call: &Call,
        arguments: GetLevel,
    ) -> Result<At<LevelOutput>, ToolError> {
        let shown = if arguments.kinds.is_empty() {
            NodeKind::ALL.into_iter().collect()
        } else {
            arguments.kinds
        };
        let container = arguments.container.as_ref();
        let projected = self
            .service
            .level(call, &arguments.journey, &shown, container)
            .await?;
        Ok(At::of(projected, |level| {
            let Level {
                container,
                shown,
                nodes,
                edges,
            } = level;
            let nodes = page(nodes, arguments.cursor);
            let on_page: BTreeSet<&NodeKey> = nodes.items.iter().map(|node| &node.key).collect();
            let edges = edges
                .into_iter()
                .filter(|edge| on_page.contains(&edge.from) || on_page.contains(&edge.to))
                .collect();
            LevelOutput {
                container,
                shown,
                nodes,
                edges,
            }
        }))
    }

    /// C8: `get_node`, or a page of one of its explanation lists.
    async fn get_node(&self, call: &Call, arguments: GetNode) -> Result<At<NodeOutput>, ToolError> {
        let (journey, node) = (&arguments.journey, &arguments.node);
        let Some(more) = arguments.explanations else {
            let projected = self.service.node_detail(call, journey, node).await?;
            let (children, annotations) = (arguments.children_cursor, arguments.annotations_cursor);
            return Ok(At::of(projected, |detail| NodeOutput {
                detail: Some(Box::new(Detail::of(detail, children, annotations))),
                explanations: None,
            }));
        };
        let projected = self
            .service
            .explanations(call, journey, node, more.field, more.cursor)
            .await?;
        Ok(At::of(projected, |page| NodeOutput {
            detail: None,
            explanations: Some(page),
        }))
    }

    /// C10, C9: `list_frontier`: the acting frontier ranked, or the nodes the filters hold
    /// for, a page at a time.
    async fn list_frontier(
        &self,
        call: &Call,
        arguments: ListFrontier,
    ) -> Result<At<FrontierOutput>, ToolError> {
        let journey = &arguments.journey;
        if arguments.filters.is_empty() {
            let query = NextQuery {
                sort: SortBy::Rank,
                mine: arguments.mine,
                kinds: arguments.kinds,
                within: arguments.within,
                for_viewer: arguments.for_viewer,
            };
            let projected = self.service.next(call, journey, &query).await?;
            let cursor = arguments.cursor;
            return Ok(At::of(projected, |next| {
                let Next { items, stalled } = next;
                FrontierOutput {
                    rows: page(items, cursor),
                    stalled,
                }
            }));
        }
        let mut flags: BTreeSet<ListFlag> = arguments.filters.into_iter().map(Into::into).collect();
        if arguments.mine {
            flags.insert(ListFlag::Mine);
        }
        let query = ListQuery {
            flags,
            within: arguments.within,
            kinds: arguments.kinds,
            sort: SortBy::Rank,
            cursor: arguments.cursor,
            ..ListQuery::default()
        };
        let projected = self.service.list(call, journey, &query).await?;
        Ok(At::of(projected, |list| FrontierOutput {
            rows: Paged {
                items: list.rows,
                next: list.next,
                total: list.total,
            },
            stalled: None,
        }))
    }
}
