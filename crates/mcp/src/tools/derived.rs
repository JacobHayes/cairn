//! The derived reads (I3, C2, C8, C9, C10): the snapshot an agent reads first, a canvas
//! level, one node in full, and the ranked frontier and its filtered lists. Each is derived
//! at the caller's today with the caller's identities as the viewer (H3), and answers the
//! revision it was derived at.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AgentRow, Annotation, AnswerEffects, AnswerValue, Cursor, Date, DisplayState, ExplainedField,
    ExplanationPage, JourneyId, KeyRefs, Level, LevelDateLink, LevelDisplay, LevelNode, LevelQuery,
    ListFlag, ListQuery, LocalEdit, Markdown, Next, NextQuery, Node, NodeDerived, NodeKey,
    NodeKind, NodeState, Overrides, Path, RemovalRead, Snapshot, SnapshotScope, SortBy, Stalled,
    StillWaiting,
};
use cairn_service::{Call, ChildEntry, NodeDetail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{At, Paged, page};
use crate::toolset::{Spec, Writes, output, parse, schema};
use crate::{ToolError, ToolSet};

/// The derived read tools.
pub(crate) const SPECS: &[Spec] = &[
    Spec {
        name: "get_snapshot",
        description: "Read this first on a journey: answers, a page of in-scope nodes in \
            tree order, the ranked acting frontier, open decisions, placeholders \
            to break down, unassigned items, shortfalls, counts. Its `revision` is \
            your next write's `base_revision`. Scope with `subtree` and `depth`.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetSnapshot>,
        output: schema::<At<SnapshotOutput>>,
    },
    Spec {
        name: "list_frontier",
        description: "The ranked acting frontier, paged: what to do next, each node with its \
            rank (`get_node` says why). With `filters`, the in-scope nodes matching every \
            filter, in rank order.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<ListFrontier>,
        output: schema::<At<FrontierOutput>>,
    },
    Spec {
        name: "get_node",
        description: "One node in full: as written, its state, answer, notes, children, and \
            each derived value with why (status, relevance, blocking, dates, \
            priority). `removal` answers the exact `remove_node` removal instead; \
            `explanations` pages a cut list.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetNode>,
        output: schema::<At<NodeOutput>>,
    },
    Spec {
        name: "get_level",
        description: "One level of the graph, at the top or inside `container`: nodes with \
            roll-ups, and the edges between them.",
        writes: Writes::Nothing,
        destructive: false,
        schema: schema::<GetLevel>,
        output: schema::<At<LevelOutput>>,
    },
];

// PRD: I3.
/// `get_snapshot`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetSnapshot {
    journey: JourneyId,
    #[serde(default)]
    subtree: Option<NodeKey>,
    /// Levels listed below `subtree` (or of roots); every level when absent. The frontier
    /// and lists ignore it.
    #[serde(default)]
    depth: Option<u32>,
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

// PRD: C2.
/// `get_level`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetLevel {
    journey: JourneyId,
    #[serde(default)]
    kinds: BTreeSet<NodeKind>,
    /// Drill into this container; the top level when absent.
    #[serde(default)]
    container: Option<NodeKey>,
    /// Containers rolled into their own cards: contents hidden, edges re-targeted.
    #[serde(default)]
    collapsed: BTreeSet<NodeKey>,
    /// Relevance classes shown (`relevant`, `conditional`, `not_relevant`); all when
    /// empty.
    #[serde(default)]
    display: BTreeSet<LevelDisplay>,
    /// Drawn whatever would hide it, with its date relationships to the other cards.
    #[serde(default)]
    selected: Option<NodeKey>,
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
    /// The containers collapsed that took effect.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    collapsed: BTreeSet<NodeKey>,
    /// The relevance classes shown; every class when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    display: Option<BTreeSet<LevelDisplay>>,
    /// A page of the visible nodes, in tree order, with their roll-ups.
    nodes: Paged<LevelNode>,
    /// The level's edges with an end among this page's nodes.
    edges: Vec<Edge>,
    // PRD: C1.
    /// The level's date relationships (not edges: nothing waits on them) with an end among
    /// this page's nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    date_links: Vec<LevelDateLink>,
    // PRD: C1.
    /// Per relevance class left out, how many nodes it alone hides; none when every
    /// class is shown.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    hidden_by_class: BTreeMap<LevelDisplay, u32>,
}

/// An edge drawn at a level, between the nearest visible stand-ins of its ends; the edges it
/// stands for are left to the canvas.
#[derive(Debug, Serialize, JsonSchema)]
struct Edge {
    /// The stand-in of the requirement side.
    from: NodeKey,
    /// The stand-in of the dependent side.
    to: NodeKey,
    // PRD: C1.
    /// Every underlying edge is implicit (drawn dotted).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    implicit: bool,
}

// PRD: C8.
/// `get_node`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetNode {
    journey: JourneyId,
    node: NodeKey,
    /// A further page of one explanation list the node cuts, instead of the node.
    #[serde(default)]
    explanations: Option<MoreExplanations>,
    /// Instead of the node, the exact removal for `remove_node` and the outside
    /// references the same patch must change.
    #[serde(default)]
    removal: bool,
    #[serde(default)]
    references_cursor: Cursor,
    #[serde(default)]
    children_cursor: Cursor,
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

/// One node in full, a page of an explanation list, or its removal: exactly one is present.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct NodeOutput {
    /// The node's detail.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<Box<Detail>>,
    /// A page of an explanation list.
    #[serde(skip_serializing_if = "Option::is_none")]
    explanations: Option<ExplanationPage>,
    /// The removal of the node and what else a patch removing it must change.
    #[serde(skip_serializing_if = "Option::is_none")]
    removal: Option<RemovalRead>,
}

// PRD: C8.
/// What a node is, its stored state and records, and every derived value with what
/// explains it (gravity and unlocks contributions cut to the largest, with totals).
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
    // PRD: B4.
    /// What the journey edited on it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    local_edits: BTreeSet<LocalEdit>,
    // PRD: G5.
    /// The checklist items ticked.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    ticks: BTreeSet<cairn_schema::Slug>,
    /// A decision's answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<AnswerValue>,
    // PRD: B2.
    /// The rationale the answer was given with, markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    rationale: Option<Markdown>,
    // PRD: F2.
    /// Its pin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pin: Option<Date>,
    // PRD: D4.
    /// Its overrides.
    #[serde(skip_serializing_if = "Option::is_none")]
    overrides: Option<Overrides>,
    /// A page of its notes and links, in key order; pass `next` as `annotations_cursor` for
    /// the next page.
    annotations: Paged<Annotation>,
    // PRD: D3.
    /// Every derived value with its explanation: relevance, blocking, dates, gravity,
    /// unlocks, and rank.
    derived: NodeDerived,
    // PRD: C8.
    /// The direct dependents completing it would not yet free, each with what else it waits
    /// on (Priority: Unlocks): the largest entries with the total; pass `explanations`
    /// with field `still_waiting` for the rest.
    still_waiting: StillWaiting,
    // PRD: C12.
    /// A decision's effects per choice: what each answer brings in, drops, and leaves
    /// to be decided later, the role it fills, and the milestone it pins. Saying "answering
    /// Partner brings in 6 and opens 2 more decisions" uses these numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    answer_effects: Option<AnswerEffects>,
}

/// One child of a node.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct Child {
    key: NodeKey,
    title: cairn_schema::Title,
    kind: NodeKind,
    /// Its stored state: which transition applies.
    state: cairn_schema::State,
    /// What it is doing now; prefer it for status.
    display_state: DisplayState,
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
            let ChildEntry {
                key,
                title,
                kind,
                state,
                display_state,
            } = child;
            Child {
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
            children: page(children.collect(), children_cursor),
            record,
            local_edits,
            ticks,
            answer,
            rationale,
            pin,
            overrides,
            annotations: page(annotations, annotations_cursor),
            derived,
            still_waiting,
            answer_effects,
        }
    }
}

// PRD: C9, C10, I2.
/// `list_frontier`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListFrontier {
    journey: JourneyId,
    /// None: the acting frontier. Any: the in-scope nodes matching every one, in rank
    /// order.
    #[serde(default)]
    filters: BTreeSet<ListFlag>,
    // PRD: E4.
    #[serde(default)]
    mine: bool,
    #[serde(default)]
    kinds: BTreeSet<NodeKind>,
    #[serde(default)]
    within: Option<NodeKey>,
    /// Rank for the caller; with no filters only.
    #[serde(default)]
    for_viewer: bool,
    #[serde(default)]
    cursor: Cursor,
}

/// A page of the frontier or of a filtered list, in rank order.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct FrontierOutput {
    /// The page: each node with its breadcrumb, signals, and rank.
    #[serde(flatten)]
    rows: Paged<AgentRow>,
    // PRD: C5, D5.
    /// When the frontier is empty and work remains, what the journey waits on.
    #[serde(skip_serializing_if = "Option::is_none")]
    stalled: Option<Stalled>,
}

impl ToolSet {
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

    // PRD: I3.
    /// `get_snapshot`.
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

    // PRD: C2.
    /// `get_level`, its nodes paged and its edges those touching the page.
    async fn get_level(
        &self,
        call: &Call,
        arguments: GetLevel,
    ) -> Result<At<LevelOutput>, ToolError> {
        let mut query = LevelQuery::requested(
            arguments.kinds,
            arguments.container,
            arguments.collapsed,
            arguments.display,
        );
        query.selected = arguments.selected;
        let projected = self.service.level(call, &arguments.journey, &query).await?;
        Ok(At::of(projected, |level| {
            let Level {
                container,
                shown,
                collapsed,
                display,
                nodes,
                edges,
                date_links,
                // What the layout reads; an agent has no layout.
                hidden_by_class,
                // The tool's query has no flag or owner filters, so there is nothing to count.
                hidden_by_filter: _,
            } = level;
            let display = (display.len() < LevelDisplay::ALL.len()).then_some(display);
            let nodes = page(nodes, arguments.cursor);
            let on_page: BTreeSet<&NodeKey> = nodes.items.iter().map(|node| &node.key).collect();
            let edges = edges
                .into_iter()
                .filter(|edge| on_page.contains(&edge.from) || on_page.contains(&edge.to))
                .map(|edge| Edge {
                    from: edge.from,
                    to: edge.to,
                    implicit: edge.implicit,
                })
                .collect();
            let date_links = date_links
                .into_iter()
                .filter(|link| on_page.contains(&link.from) || on_page.contains(&link.to))
                .collect();
            LevelOutput {
                container,
                shown,
                collapsed,
                display,
                nodes,
                edges,
                date_links,
                hidden_by_class,
            }
        }))
    }

    // PRD: C8.
    /// `get_node`, or a page of one of its explanation lists.
    async fn get_node(&self, call: &Call, arguments: GetNode) -> Result<At<NodeOutput>, ToolError> {
        let (journey, node) = (&arguments.journey, &arguments.node);
        if arguments.removal && arguments.explanations.is_some() {
            return Err(ToolError::Arguments {
                path: "removal".into(),
                message: "ask for `removal` or `explanations`, not both".into(),
            });
        }
        if arguments.removal {
            let cursor = arguments.references_cursor;
            let projected = self
                .service
                .node_removal(call, journey, node, cursor)
                .await?;
            return Ok(At::of(projected, |read| NodeOutput {
                detail: None,
                explanations: None,
                removal: Some(read),
            }));
        }
        let Some(more) = arguments.explanations else {
            let projected = self.service.node_detail(call, journey, node).await?;
            let (children, annotations) = (arguments.children_cursor, arguments.annotations_cursor);
            return Ok(At::of(projected, |detail| NodeOutput {
                detail: Some(Box::new(Detail::of(detail, children, annotations))),
                explanations: None,
                removal: None,
            }));
        };
        let projected = self
            .service
            .explanations(call, journey, node, more.field, more.cursor)
            .await?;
        Ok(At::of(projected, |page| NodeOutput {
            detail: None,
            explanations: Some(page),
            removal: None,
        }))
    }

    // PRD: C10, C9.
    /// `list_frontier`: the acting frontier ranked, or the nodes the filters hold
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
                let Paged { items, next, total } = page(items, cursor);
                FrontierOutput {
                    rows: Paged {
                        items: items.iter().map(AgentRow::on_frontier).collect(),
                        next,
                        total,
                    },
                    stalled,
                }
            }));
        }
        let mut flags = arguments.filters;
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
                items: list.rows.iter().map(AgentRow::on_frontier).collect(),
                next: list.next,
                total: list.total,
            },
            stalled: None,
        }))
    }
}
