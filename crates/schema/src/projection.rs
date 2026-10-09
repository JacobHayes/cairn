//! Projections as they cross a crate or the API (ARCHITECTURE, Engine > Projections): the
//! read views the engine cuts from a derived journey, so the browser, which derives locally,
//! and the server, for agents and the API, produce the same values. Each is computed on read
//! and never stored (D3, D6). Lists an agent pages are cut at `page_item_count_max` and
//! carry a [`Cursor`] to the next page.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::derived::{
    Blocker, Contribution, DateOrigin, DisplayState, EffectiveDate, HeldDependent, PeakGravity,
    Real, Relevance, Score, Stalled,
};
use crate::event::Event;
use crate::id::{EntityKey, KindKey, NodeKey, PatchId, Path, RoleKey, Slug};
use crate::node::NodeKind;
use crate::state::{AnswerValue, SnoozeTarget, State};
use crate::text::{Markdown, Title};

/// A position in a projection's order: where the next page starts (I3, J4). Valid for the
/// same journey revision and derive inputs; a page read at another revision starts over.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct Cursor(u32);

impl Cursor {
    /// The first item.
    pub const START: Cursor = Cursor(0);

    /// The cursor at `position` in a projection's order.
    #[must_use]
    pub const fn at(position: u32) -> Self {
        Self(position)
    }

    /// The position it names.
    #[must_use]
    pub const fn position(self) -> u32 {
        self.0
    }
}

/// C2: how an edge on the canvas arose (C1: implicit gates are drawn dotted).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrigin {
    /// An explicit `requires` edge (A3).
    Explicit,
    /// A condition gate: the dependent's condition reads the decision (Gating).
    Condition,
    /// A stage's opening milestone (F4).
    StageOpening,
}

/// One edge of the graph a canvas edge stands for: `dependent` waits on `requirement`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct UnderlyingEdge {
    /// The node waited on.
    pub requirement: NodeKey,
    /// The node whose `requires`, condition, or opening it is.
    pub dependent: NodeKey,
    /// How it arose.
    pub origin: EdgeOrigin,
    /// It blocks (D1); false for a stage opening with `gates: false`, which holds dates only.
    pub gates: bool,
}

/// C2: an edge drawn at a level, between the nearest visible stand-ins of its ends, with the
/// edges it collapses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LevelEdge {
    /// The stand-in of the requirement side.
    pub from: NodeKey,
    /// The stand-in of the dependent side.
    pub to: NodeKey,
    /// Every underlying edge is implicit (drawn dotted, C1).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub implicit: bool,
    /// Some underlying edge blocks.
    pub gates: bool,
    /// The edges it stands for, sorted; never empty.
    pub underlying: Vec<UnderlyingEdge>,
}

/// D1, C2: a group's display state, derived from its children and dependencies. Deprecated:
/// [`DisplayState`] (D8) is the state of every node, groups included; this stays, equal in
/// meaning on groups, until a later breaking release.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum GroupState {
    /// Not relevant (Gating): drawn grayed (C1).
    NotRelevant,
    /// Explicitly or effectively skipped (D1a).
    Skipped,
    /// It satisfies dependencies: its children and gates are done.
    Done,
    /// Its own or an inherited requirement, opening, or condition is not yet satisfied.
    Waiting,
    /// Work beneath it has started.
    Active,
    /// Open to start, nothing beneath it started.
    NotStarted,
}

/// C2: what a container shows of its children.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollUp {
    /// A deliverable or action whose children and other dependencies are satisfied but which
    /// is not terminal: "children complete, ready to finish".
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub ready_to_finish: bool,
    /// A child is `active`.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub children_active: bool,
    /// Every relevant child that is not done is blocked (`all_blocked`), and there is one.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub all_blocked: bool,
    /// A child decision is actionable: "decision needed".
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub decision_needed: bool,
    /// A child needs breakdown.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub needs_breakdown: bool,
    /// The largest in-scope child gravity (Priority), one level down.
    ///
    /// Deprecated: read `peak_gravity`, which looks at every depth and names the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("deprecated" = true))]
    pub max_child_gravity: Option<Score>,
    /// The largest gravity among its open, in-scope descendants at any depth, naming that
    /// node (Priority).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_gravity: Option<PeakGravity>,
    /// The least slack among in-scope, open children; none when none has a deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_child_slack_days: Option<i32>,
    /// The distinct owners of its children.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub owners: BTreeSet<EntityKey>,
}

/// C2: one visible node at a level.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LevelNode {
    /// The node.
    pub key: NodeKey,
    /// Its nearest visible ancestor within the level, which it is drawn under; none at the
    /// top level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeKey>,
    /// The hidden nodes that roll up into it (a hidden action is a checklist item on its
    /// deliverable, C4), in tree order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rolled_up: Vec<NodeKey>,
    /// Its hidden, unsatisfied prerequisites that no visible edge stands for: the "hidden
    /// prerequisites" marker, which opens the trace (C7). Empty when it has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_prerequisites: Vec<NodeKey>,
    /// A skipped container whose kept work is not yet done: "skipped, kept work pending"
    /// (D1a).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub kept_work_pending: bool,
    /// D8: the state every surface shows for the node.
    pub display_state: DisplayState,
    /// A group's display state. Deprecated: read `display_state`, which every node carries.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(extend("deprecated" = true))]
    pub group_state: Option<GroupState>,
    /// A container's badges and roll-ups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll_up: Option<RollUp>,
}

/// C2: which nodes a level shows by how their relevance reads to a person. A node is
/// `relevant` when it applies; `conditional` when it may apply (its relevance is undecided,
/// or it is not relevant only because the decision it reads cannot be answered yet); and
/// `not_relevant` when it is settled as not applying (ruled out by an answer, a skip, or a
/// closed branch).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LevelDisplay {
    /// Applies.
    Relevant,
    /// May apply: undecided, or waiting on a decision that cannot be answered yet.
    Conditional,
    /// Settled as not applying.
    NotRelevant,
}

impl LevelDisplay {
    /// Every class.
    pub const ALL: [LevelDisplay; 3] = [
        LevelDisplay::Relevant,
        LevelDisplay::Conditional,
        LevelDisplay::NotRelevant,
    ];
}

impl LevelDisplay {
    /// Every class, as a set: what a level shows when none are asked for.
    #[must_use]
    pub fn every() -> BTreeSet<LevelDisplay> {
        LevelDisplay::ALL.into_iter().collect()
    }

    /// Whether the set is every class.
    #[must_use]
    pub fn is_every(display: &BTreeSet<LevelDisplay>) -> bool {
        display.len() == LevelDisplay::ALL.len()
    }
}

/// C2: what a level request asks for: the kinds shown, the container drilled into, the
/// containers collapsed into their cards, and the relevance classes shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LevelQuery {
    /// The kinds shown.
    pub shown: BTreeSet<NodeKind>,
    /// The container drilled into; the whole journey when none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<NodeKey>,
    /// Containers rolled into their own cards, whatever their kinds' step: everything beneath
    /// each is hidden and rolls up into it. A key outside the level, or at its root (the
    /// drilled-in container), has no effect.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed: BTreeSet<NodeKey>,
    /// The relevance classes shown; every class when none is given. A node of a class left
    /// out is hidden like a node of a kind left out: it rolls up and its edges re-target.
    #[serde(
        default = "LevelDisplay::every",
        skip_serializing_if = "LevelDisplay::is_every"
    )]
    pub display: BTreeSet<LevelDisplay>,
}

impl LevelQuery {
    /// The kinds `shown` within `container`, nothing collapsed, every class shown.
    #[must_use]
    pub fn of_kinds(shown: BTreeSet<NodeKind>, container: Option<NodeKey>) -> Self {
        Self {
            shown,
            container,
            collapsed: BTreeSet::new(),
            display: LevelDisplay::every(),
        }
    }
}

/// C2: one aggregation level of the canvas: the visible nodes for the shown kinds within the
/// drilled-in container, the edges re-targeted to visible stand-ins with duplicates collapsed,
/// and each container's roll-ups. Display only: stored state stays on each node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// The drilled-in container; none for the whole journey.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<NodeKey>,
    /// The kinds shown.
    pub shown: BTreeSet<NodeKind>,
    /// The containers collapsed that took effect: those within the level.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub collapsed: BTreeSet<NodeKey>,
    /// The relevance classes shown; every class when absent.
    #[serde(
        default = "LevelDisplay::every",
        skip_serializing_if = "LevelDisplay::is_every"
    )]
    pub display: BTreeSet<LevelDisplay>,
    /// The visible nodes, in tree order.
    pub nodes: Vec<LevelNode>,
    /// The edges, sorted by their ends.
    pub edges: Vec<LevelEdge>,
}

/// Some nodes an answer's effect names: how many in all, and the first
/// `explanation_entry_count_max` of them in tree order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffectedNodes {
    /// How many nodes there are in all.
    pub total: u32,
    /// The first of them, in tree order, at most `explanation_entry_count_max`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeKey>,
}

/// C12: what one answer to a decision does to the journey's scope, from a three-valued
/// re-evaluation of relevance under that answer: never a full derive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChoiceEffect {
    /// The whole answer evaluated: a boolean, a choice, or (for a multi choice) the current
    /// answer with `choice` toggled.
    pub answer: AnswerValue,
    /// The choice id, for a single or multi choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<Slug>,
    /// This is the recorded answer (for a multi choice: `choice` is in it, so the answer
    /// evaluated removes it).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub current: bool,
    /// Nodes that become relevant.
    pub brings_in: AffectedNodes,
    /// Of those, the decisions that open: relevant and still to be answered.
    pub opens_decisions: AffectedNodes,
    /// Nodes the answer settles as not relevant. Nodes that would still hang on another
    /// unanswered decision are `decided_later`, never here.
    pub drops: AffectedNodes,
    /// Of the drops, the nodes with recorded progress (started, done, decided, reached):
    /// the answer does not undo it.
    pub drops_with_progress: AffectedNodes,
    /// Nodes that would stay undecided because they hang on another decision still to be
    /// answered, which this answer opens or leaves open.
    pub decided_later: AffectedNodes,
}

/// C12, E3: what answering a decision does, per choice, and the static lines every answer
/// of a date or entity decision carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerEffects {
    /// The decision.
    pub decision: NodeKey,
    /// The role its answer fills (E3), for an entity or entity-list decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fills_role: Option<RoleKey>,
    /// The milestone its answer pins (E3), for a date decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pins: Option<NodeKey>,
    /// Whether the decision's answer is in effect: it is relevant, and not skipped. When not,
    /// no answer changes any other node's relevance and every choice's effect is empty.
    pub applies: bool,
    /// One entry per choice of a boolean, single-choice, or multi-choice decision, in the
    /// decision's order (`false` before `true`); none for the other answer types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChoiceEffect>,
}

/// C7: what a node depends on and what depends on it, across levels, over the full structural
/// graph: terminal and not-relevant nodes included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    /// The node traced.
    pub node: NodeKey,
    /// Its transitive dependencies (gating decisions, stage openings, its children), in key
    /// order.
    pub upstream: Vec<NodeKey>,
    /// Its transitive dependents (its ancestors, the nodes whose relevance it determines), in
    /// key order.
    pub downstream: Vec<NodeKey>,
    /// The nodes in `downstream` that contribute to its gravity, in key order.
    pub gravity_contributors: Vec<NodeKey>,
}

/// C9, C10: the single signal a list is sorted by; ties fall back to rank order.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// Rank, highest first, in Priority's tie order.
    #[default]
    Rank,
    /// Slack, least first, no deadline last.
    Slack,
    /// Gravity, greatest first.
    Gravity,
    /// Leverage, greatest first.
    Leverage,
    /// Due date, earliest first, none last.
    Due,
    /// Gravity per estimated day, greatest first, no estimate last (Priority: Effort-adjusted).
    Effort,
}

/// C8, C10: why a node ranks where it does: its rank and the terms it blends.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RankTerms {
    /// From slack over the urgency horizon.
    pub urgency: Real,
    /// From negative slack over the horizon.
    pub late: Real,
    /// Gravity over the largest in the normalization set.
    pub gravity_norm: Real,
    /// Leverage over the largest in the normalization set.
    pub leverage_norm: Real,
    /// The blend.
    pub rank: Real,
}

/// One node as a row of the list, the next list, or the snapshot (C9, C10, I3): what it is,
/// where it sits, and its derived signals.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeRow {
    /// The node.
    pub key: NodeKey,
    /// Its path.
    pub path: Path,
    /// Its kind.
    pub kind: NodeKind,
    /// Its title.
    pub title: Title,
    /// Its stored state: what transitions act on. Prefer `display_state` for "what is its
    /// status".
    pub state: State,
    /// D8: the state every surface shows for the node.
    pub display_state: DisplayState,
    /// Its ancestors, root first: the breadcrumb (C10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ancestors: Vec<NodeKey>,
    /// Its relevance.
    pub relevance: Relevance,
    /// D2.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub actionable: bool,
    /// Gating, Blocked.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub blocked: bool,
    /// Skipped through an ancestor (D1a).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub effectively_skipped: bool,
    /// Its owners (E2).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub owners: BTreeSet<EntityKey>,
    /// No owner (E1).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub unassigned: bool,
    /// A snooze that holds (B6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snoozed: Option<SnoozeTarget>,
    /// Non-terminal with its due before today (D3).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub overdue: bool,
    /// A terminal node whose completing guards would now fail (D4).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub stale: bool,
    /// B10.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub needs_breakdown: bool,
    /// Days the plan can no longer be met by (F6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortfall_days: Option<u32>,
    /// Latest start minus today; none is no deadline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack_days: Option<i32>,
    /// The latest it can finish.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<Date>,
    /// Gravity.
    pub gravity: Score,
    /// Leverage: the viewer's, in a ranking for them.
    pub leverage: Score,
    /// Its rank and terms, for nodes in the normalization set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<RankTerms>,
}

/// C10: what the next list shows: filters, the sort, and whose ranking.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NextQuery {
    /// The signal to sort by.
    #[serde(default)]
    pub sort: SortBy,
    /// Only nodes the viewer participates in (E4).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub mine: bool,
    /// Only these node kinds; every kind when empty.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub kinds: BTreeSet<NodeKind>,
    /// Only this node and the nodes beneath it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<NodeKey>,
    /// Rank for the viewer: "prioritize for me" (Priority).
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub for_viewer: bool,
}

/// C10: the acting frontier ordered, each item with its breadcrumb and why it ranks where it
/// does; when it is empty, what the journey waits on (C5, D5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Next {
    /// The items, in order.
    pub items: Vec<NodeRow>,
    /// The stalled diagnostic, when the journey is stalled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Stalled>,
}

/// C9: a list filter that needs no argument.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ListFlag {
    /// The viewer participates (E4).
    Mine,
    /// No owner.
    Unassigned,
    /// On the acting frontier.
    NextUp,
    /// An actionable decision.
    DecisionsNeeded,
    /// B10.
    NeedsBreakdown,
    /// Stored `active`.
    Active,
    /// Blocked.
    Blocked,
    /// Overdue.
    Overdue,
    /// Stale.
    Stale,
    /// Snoozed.
    Snoozed,
    /// Snoozed and overdue.
    SnoozedAndOverdue,
    /// With a shortfall.
    Shortfall,
}

/// C9: which nodes the list shows and how it orders them. Every filter given must hold.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    /// Filters with no argument.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub flags: BTreeSet<ListFlag>,
    /// Only nodes beneath this container (by group).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub within: Option<NodeKey>,
    /// Only nodes this entity owns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<EntityKey>,
    /// Only nodes in these stored states; any when empty.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub states: BTreeSet<State>,
    /// Only these node kinds; any when empty.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub kinds: BTreeSet<NodeKind>,
    /// Text found in the title, description, notes, or resources, ignoring ASCII case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<Title>,
    /// The signal to sort by.
    #[serde(default)]
    pub sort: SortBy,
    /// Where the page starts.
    #[serde(default)]
    pub cursor: Cursor,
}

/// C9: a page of the list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListPage {
    /// The rows, at most `page_item_count_max`.
    pub rows: Vec<NodeRow>,
    /// Where the next page starts, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    /// How many rows the query matches in all.
    pub total: u32,
}

/// E4: a node the viewer participates in, with the kinds they hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MineEntry {
    /// The node.
    pub node: NodeKey,
    /// The participation kinds the viewer holds on it.
    pub kinds: BTreeSet<KindKey>,
}

/// I3: which part of the journey a snapshot covers and which page of its nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotScope {
    /// The subtree: this node and everything beneath it; the whole journey when none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtree: Option<NodeKey>,
    /// How many levels below the subtree's node (or of roots, at depth 1) the node list goes;
    /// every level when none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// Where the node list's page starts.
    #[serde(default)]
    pub cursor: Cursor,
}

/// I3: one in-scope node in the snapshot, with its participations, blocking, and dates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotNode {
    /// What it is and its signals.
    pub row: NodeRow,
    /// Its effective participations by kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub participations: BTreeMap<KindKey, BTreeSet<EntityKey>>,
    /// What blocks it itself (Gating, Blocked).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<Blocker>,
    /// The ancestors that block it, nearest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_through: Vec<NodeKey>,
    /// The earliest it can start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_start: Option<Date>,
    /// The latest it can start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_start: Option<Date>,
}

/// I3: what a snapshot counts in its subtree, whatever its depth and page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotCounts {
    /// In-scope (relevant or undecided) nodes.
    pub in_scope: u32,
    /// Not-relevant nodes.
    pub not_relevant: u32,
    /// In-scope nodes by stored state; they add up to `in_scope`.
    pub by_state: BTreeMap<State, u32>,
    /// D8: the same in-scope nodes by display state; they add up to `in_scope`, and none is
    /// `not_relevant`. A not-relevant node pending on an undecided decision is out of scope
    /// and so not counted here until its decision is answered.
    pub by_display_state: BTreeMap<DisplayState, u32>,
    /// In-scope nodes within the depth: the node list across its pages.
    pub listed: u32,
    /// The frontier.
    pub frontier: u32,
    /// The acting frontier: its top N and the rest.
    pub acting_frontier: u32,
    /// Blocked nodes.
    pub blocked: u32,
}

/// I3: the bounded agent view of a journey: answers, a page of in-scope nodes, the ranked
/// acting frontier's top N with the rest as keys, open decisions by rank, placeholders needing
/// breakdown, unassigned items, shortfalls, and counts, scoped to a subtree and depth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// The today it was derived for.
    pub today: Date,
    /// What it covers.
    pub scope: SnapshotScope,
    /// The answers in effect in the subtree.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub answers: BTreeMap<NodeKey, AnswerValue>,
    /// The rationales those answers were given with, for the answers that have one (B2).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rationales: BTreeMap<NodeKey, Markdown>,
    /// A page of in-scope nodes within the depth, in tree order.
    pub nodes: Vec<SnapshotNode>,
    /// Where the next page of nodes starts, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    /// The acting frontier's first `page_item_count_max` items, in rank order: the next list.
    pub acting_frontier: Vec<NodeRow>,
    /// The rest of the acting frontier, in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acting_frontier_rest: Vec<NodeKey>,
    /// Open, in-scope decisions, in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_decisions: Vec<NodeKey>,
    /// Placeholders needing breakdown (B10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs_breakdown: Vec<NodeKey>,
    /// Open, in-scope nodes no one owns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unassigned: Vec<NodeKey>,
    /// Nodes whose plan can no longer be met.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortfalls: Vec<NodeKey>,
    /// The stalled diagnostic, when the journey is stalled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stalled: Option<Stalled>,
    /// The counts.
    pub counts: SnapshotCounts,
}

/// ARCHITECTURE, Read path: a derived value whose explanation list a server response pages.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ExplainedField {
    /// The nodes that make up its gravity.
    Gravity,
    /// The nodes completing it would unblock.
    Leverage,
    /// The direct dependents completing it would not yet free, with what else each waits on.
    StillWaiting,
}

/// A page of one value's explanation list, largest first, `explanation_entry_count_max` at a
/// time; the first page is what node detail carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplanationPage {
    /// The node.
    pub node: NodeKey,
    /// The value.
    pub field: ExplainedField,
    /// The entries of a gravity or leverage list.
    pub entries: Vec<Contribution>,
    /// The entries of a still-waiting list, which holds dependents rather than
    /// contributions; empty for the other fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held: Vec<HeldDependent>,
    /// Where the next page starts, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    /// How many entries there are in all.
    pub total: u32,
}

/// J4: one patch's events, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PatchEvents {
    /// The patch.
    pub patch_id: PatchId,
    /// Its events on this page.
    pub events: Vec<Event>,
}

/// J4: a page of history, per node or per journey, grouped by patch: at most
/// `page_item_count_max` events, so a large patch spans pages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryPage {
    /// The events, grouped by patch, in log order.
    pub patches: Vec<PatchEvents>,
    /// Where the next page starts, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<Cursor>,
    /// How many events the history holds in all.
    pub total: u32,
}

/// C12: one decision in the decision view: its answer and which nodes the answer affects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionEntry {
    /// The decision.
    pub node: NodeKey,
    /// Its stored state.
    pub state: State,
    /// D8: the state every surface shows for it.
    pub display_state: DisplayState,
    /// Its relevance.
    pub relevance: Relevance,
    /// Its answer, while in effect (decided and in scope, E3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerValue>,
    /// The rationale its answer was given with, while the answer is in effect (B2, C12).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<Markdown>,
    /// Its owners.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub owners: BTreeSet<EntityKey>,
    /// The nodes whose relevance a condition reading it decides, their descendants included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affects: Vec<NodeKey>,
    /// The milestone its answer pins (E3, `feeds_milestone`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pins: Option<NodeKey>,
    /// The role its answer fills (E3, `fills_role`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fills: Option<RoleKey>,
    /// Its unsatisfied prerequisites that no drawn edge stands for (C2's marker).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_prerequisites: Vec<NodeKey>,
}

/// C12: the graph filtered to decisions and their gating edges, with answers and what each
/// answer affected. A projection of the level with only decisions shown, not a separate
/// structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionView {
    /// The decisions, in tree order.
    pub decisions: Vec<DecisionEntry>,
    /// The edges between decisions.
    pub edges: Vec<LevelEdge>,
}

/// C13: one date on the timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineEntry {
    /// The node.
    pub node: NodeKey,
    /// Its kind.
    pub kind: NodeKind,
    /// The date.
    pub date: Date,
    /// Where the date comes from: a milestone's actual, a pin, or a derived due date.
    pub origin: DateOrigin,
    /// Non-terminal with its due before today.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub overdue: bool,
    /// Days the plan can no longer be met by (F6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortfall_days: Option<u32>,
    /// The journey's `final` milestone: the end anchor.
    #[serde(
        default,
        rename = "final",
        skip_serializing_if = "crate::serde_util::is_false"
    )]
    pub is_final: bool,
}

/// C13: milestones, pinned dates, and the derived due dates of open work on a time axis, with
/// overdue and shortfall marked and the `final` milestone as the end anchor. In scope only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    /// The dates, earliest first, then by key.
    pub entries: Vec<TimelineEntry>,
    /// The `final` milestone, when it is in scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<NodeKey>,
    /// In-scope milestones with no date yet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undated: Vec<NodeKey>,
}

/// C18: an open decision and who owns it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenDecision {
    /// The decision.
    pub node: NodeKey,
    /// Its owners; empty when unassigned.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub owners: BTreeSet<EntityKey>,
}

/// C18: a milestone not yet reached and its effective date.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpcomingMilestone {
    /// The milestone.
    pub node: NodeKey,
    /// Its effective date (F1).
    pub date: EffectiveDate,
}

/// C18: the journey status summary for observers and reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusSummary {
    /// In-scope nodes by stored state.
    pub by_state: BTreeMap<State, u32>,
    /// D8: the same in-scope nodes by display state; they add up to the same total, and none
    /// is `not_relevant`. Prefer it to `by_state` for "how is the journey doing".
    pub by_display_state: BTreeMap<DisplayState, u32>,
    /// In-scope nodes with work left on them.
    pub remaining: u32,
    /// Overdue nodes, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overdue: Vec<NodeKey>,
    /// Nodes with a shortfall, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shortfalls: Vec<NodeKey>,
    /// Stale nodes, by key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<NodeKey>,
    /// In-scope milestones not yet reached that have a date, earliest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upcoming_milestones: Vec<UpcomingMilestone>,
    /// Open, in-scope decisions with their owners, in rank order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_decisions: Vec<OpenDecision>,
}

/// A10, G3: one piece of a rendered message draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RenderedSegment {
    /// Text, literal or filled in from the journey.
    Text(String),
    /// A placeholder with nothing to fill it (an unfilled role, an unanswered decision, a
    /// journey field with no value), as written (`roles.eval_owner.name`): shown as a marker.
    Missing(String),
}

/// A10, G3: a message draft rendered with journey context. Rendering never fails: what cannot
/// be filled is a visible marker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderedDraft {
    /// The pieces, in order.
    pub segments: Vec<RenderedSegment>,
}

impl RenderedDraft {
    /// Every placeholder was filled.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.segments
            .iter()
            .all(|segment| matches!(segment, RenderedSegment::Text(_)))
    }

    /// The text to copy, each missing placeholder written as a marker: `[missing:
    /// roles.eval_owner.name]`.
    #[must_use]
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| match segment {
                RenderedSegment::Text(text) => text.clone(),
                RenderedSegment::Missing(placeholder) => format!("[missing: {placeholder}]"),
            })
            .collect()
    }
}
