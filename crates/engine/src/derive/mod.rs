//! Derive (ARCHITECTURE, Read path: derive; D3, D6): the read path, computing every derived
//! value of a journey from its graph, its state, and the derive inputs, on every read and
//! never stored. Passes run in dependency order, each reading only earlier passes' output:
//!
//! 1. Relevance ([`relevance`]): three-valued, with the condition or override that produced
//!    each value.
//! 2. Effective dependencies ([`dependencies`]): the full and pruned dependency graphs over
//!    start, finish, and entry instants, every implicit edge tagged with its source; then
//!    effective skip and kept work ([`skip`]).
//! 3. Participation ([`participation`]): E2's resolution per node and kind, `unassigned`,
//!    and the membership-loss flag.
//! 4. The date network ([`dates`]): earliest and latest bounds, `due`, `latest_start`,
//!    slack, effective dates, `overdue`, and `shortfall`, each with its chain; the plan
//!    layer's check is validation's ([`check_plan`]).
//! 5. Auto-reach, blocking, and what can be acted on ([`blocking`]): what satisfies
//!    dependencies, `deps_done`, blocked, actionable, the frontier, snoozes that hold, the
//!    acting frontier, `stalled`, and `needs_breakdown`.
//! 6. Gravity and leverage ([`priority`]): each node's downstream set and gravity, its
//!    largest child gravity, and for the rank normalization set what completing it would
//!    unblock and its leverage.
//! 7. Rank ([`rank`]): each ranked node's terms and rank, the frontiers in rank order, the
//!    per-viewer recomputation, and effort-adjusted ordering; then `stale` ([`stale`];
//!    `overdue` is pass 4's): terminal nodes whose completing guards would now fail.
//!
//! [`Derived`] holds the output of every pass. It is the engine's own type, with explanation
//! lists complete and listed on demand; the schema's `Derived`, which crosses the API, is
//! projected from it by [`Derived::to_schema`]
//! (decisions/2026-10-06-derive-returns-the-engines-own-derived-growing-a-field-per.md).

pub mod blocking;
mod condition;
pub mod consequences;
pub mod dates;
pub mod dependencies;
mod display;
pub mod participation;
pub mod priority;
mod project;
pub mod rank;
pub mod relevance;
pub mod skip;
pub(crate) mod stale;

use std::collections::BTreeSet;

use cairn_schema::{
    Blocker, Date, DependencyVia, Deployment, DeriveInputs, EntityKey, GuardFailure, KeyRefs, Node,
    NodeKey, RankConstants, Relevance, State,
};

use crate::graph::{Document, Graph};
pub use blocking::Blocking;
pub use consequences::consequences;
pub use dates::{Dates, PlanCheck, check_plan};
pub use dependencies::{Dependencies, EdgeSet, EffectiveDependency};
pub use participation::Participation;
pub use priority::Priority;
pub use rank::{RankTerms, Ranking};
pub use relevance::{NodeRelevance, Producer, Relevances};
pub use skip::Skips;

/// A journey's derived values (D3): the output of every pass, with the inputs that explain
/// each value. Never stored (D6, J1).
#[derive(Clone, Debug, PartialEq)]
pub struct Derived {
    relevance: Relevances,
    dependencies: Dependencies,
    skips: Skips,
    participation: Participation,
    dates: Dates,
    blocking: Blocking,
    priority: Priority,
    ranking: Ranking,
    rank_constants: RankConstants,
    stale: stale::Staleness,
    today: Date,
}

impl Derived {
    /// Pass 1: relevance.
    #[must_use]
    pub fn relevance(&self) -> &Relevances {
        &self.relevance
    }

    /// Pass 2: the effective dependency graph, full and pruned.
    #[must_use]
    pub fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    /// Pass 2: effective skip and kept work.
    #[must_use]
    pub fn skips(&self) -> &Skips {
        &self.skips
    }

    /// Pass 3: participation.
    #[must_use]
    pub fn participation(&self) -> &Participation {
        &self.participation
    }

    /// Pass 4: the date network's bounds, flags, and chains.
    #[must_use]
    pub fn dates(&self) -> &Dates {
        &self.dates
    }

    /// Pass 5: auto-reach, blocking, the frontiers, snoozes, and `stalled`.
    #[must_use]
    pub fn blocking(&self) -> &Blocking {
        &self.blocking
    }

    /// Pass 6: gravity and leverage.
    #[must_use]
    pub fn priority(&self) -> &Priority {
        &self.priority
    }

    /// Pass 7: the global ranking (Priority: rank is global), its terms, and the frontiers in
    /// rank order.
    #[must_use]
    pub fn ranking(&self) -> &Ranking {
        &self.ranking
    }

    /// Priority, "prioritize for me": the ranking with leverage's owner factor relative to the
    /// viewer's entities (H3) instead of each node's owner. The global ranking is unchanged.
    #[must_use]
    pub fn rank_for(&self, viewer: &BTreeSet<EntityKey>) -> Ranking {
        Ranking::pass(
            &self.priority,
            &self.dates,
            &self.blocking,
            &self.rank_constants,
            |key| self.priority.leverage_for(key, viewer),
        )
    }

    /// Priority, Effort-adjusted: the keys by gravity per estimated day, greatest first, nodes
    /// with no or a zero estimate last, ties in global rank order. `graph` is the one derived.
    #[must_use]
    pub fn by_effort(&self, graph: &Graph, keys: &[NodeKey]) -> Vec<NodeKey> {
        self.ranking.by_effort(graph, &self.priority, keys)
    }

    /// The rank constants it was derived with.
    #[must_use]
    pub fn rank_constants(&self) -> &RankConstants {
        &self.rank_constants
    }

    /// The today it was derived for (D7: both sides of a consequence share it).
    #[must_use]
    pub fn today(&self) -> Date {
        self.today
    }

    /// D4: the node is terminal and its completing guards would now fail.
    #[must_use]
    pub fn is_stale(&self, key: &NodeKey) -> bool {
        self.stale.contains(key)
    }

    /// D4: why the node is stale, empty when it is not. Listed on demand: `graph` is the one
    /// derived.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived.
    #[must_use]
    pub fn stale(&self, graph: &Graph, key: &NodeKey) -> BTreeSet<GuardFailure> {
        let artifacts = stale::artifact_nodes(graph.document());
        let reasons = stale::reasons(
            graph,
            &self.relevance,
            &self.dependencies,
            &self.blocking,
            &artifacts,
            key,
        );
        assert_eq!(reasons.is_empty(), !self.is_stale(key), "the graph derived");
        reasons
    }

    /// D4's `deps_done` failures: each hard dependency of the node, its own and inherited,
    /// that does not satisfy dependencies.
    #[must_use]
    pub fn open_dependencies(&self, key: &NodeKey) -> BTreeSet<NodeKey> {
        stale::open_dependencies(&self.dependencies, &self.blocking, key)
    }

    /// D4's `deps_done` failures as a guarded transition and `stale` read them: those of
    /// [`Derived::open_dependencies`], less an undecided node's condition gates, which its
    /// relevance answers for (finishing undecided work is accepted with a warning).
    #[must_use]
    pub fn guard_open_dependencies(&self, key: &NodeKey) -> BTreeSet<NodeKey> {
        stale::guard_open_dependencies(&self.dependencies, &self.blocking, &self.relevance, key)
    }

    /// Gating, D4: the decisions an undecided node's relevance waits on: those still to be
    /// answered (open, relevant, and not under a skip, so they contribute `undecided`) that
    /// are read by each condition leaving it undecided, its own and its ancestors', up to a
    /// force include. Empty when the node is not undecided. `graph` is the one derived.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived, or an undecided node waits on no decision.
    #[must_use]
    pub fn unanswered(&self, graph: &Graph, key: &NodeKey) -> BTreeSet<NodeKey> {
        let document = graph.document();
        let open = |decision: &&NodeKey| {
            self.relevance.get(decision).map(|found| found.value) == Some(Relevance::Relevant)
                && document
                    .nodes
                    .get(*decision)
                    .is_some_and(|node| stored_state(document, node) == State::Open)
                && self.skips.skipped_by(decision).is_none()
        };
        let mut waiting = BTreeSet::new();
        // Up the tree while undecided: a relevant node (a force include among them) has
        // nothing undecided above it, and the depth limit bounds the walk.
        let mut current = Some(key);
        while let Some(node) = current {
            let Some(found) = self.relevance.get(node) else {
                break;
            };
            if found.value != Relevance::Undecided {
                break;
            }
            // The node's own condition is undecided exactly when it produced the value.
            if let Producer::Condition { on, decisions } = &found.producer
                && on == node
            {
                waiting.extend(decisions.iter().filter(open).cloned());
            }
            current = document
                .nodes
                .get(node)
                .and_then(|found| found.parent.as_ref());
        }
        let undecided =
            self.relevance.get(key).map(|found| found.value) == Some(Relevance::Undecided);
        assert_eq!(
            waiting.is_empty(),
            !undecided,
            "{key} is undecided exactly when it waits on an open decision"
        );
        waiting
    }

    /// Gating, Blocked: what blocks the node itself: its own explicit requirements,
    /// condition gates, and stage opening, and its children, each that does not satisfy
    /// dependencies. What it inherits from an ancestor is listed once, on that ancestor
    /// ([`Derived::blocked_through`]), never copied onto each descendant. A node that is not
    /// blocked (closed, or not relevant) lists what its own requirements, conditions, and
    /// opening still hold back beneath it, so a descendant blocked through it can follow
    /// them; empty when nothing is held back.
    #[must_use]
    pub fn blocked_by(&self, key: &NodeKey) -> Vec<Blocker> {
        let own = |blocker: &Blocker| held_by(&blocker.via).is_none_or(|holder| holder == key);
        if self.blocking.blocked(key) {
            return self
                .unsatisfied(self.dependencies.of(key, EdgeSet::Pruned))
                .filter(own)
                .collect();
        }
        self.unsatisfied(self.dependencies.passed_down(key, EdgeSet::Pruned))
            .filter(|blocker| blocker.via != DependencyVia::Containment)
            .filter(own)
            .collect()
    }

    /// Priority, Leverage: the nodes whose finish completing `key` satisfies: itself, and what
    /// derived group completion and reached milestones cascade to, as leverage simulates it.
    /// `graph` is the one derived.
    #[must_use]
    pub fn finished_by(&self, graph: &Graph, key: &NodeKey) -> BTreeSet<NodeKey> {
        let Some(index) = self.dependencies.node_index(key) else {
            return BTreeSet::new();
        };
        priority::leverage::cascade(
            graph,
            &self.dependencies,
            &self.skips,
            &self.blocking,
            index,
        )
        .into_iter()
        .filter(|instant| instant.point == dependencies::Point::Finish)
        .filter_map(|instant| self.dependencies.key(instant.node).cloned())
        .collect()
    }

    /// Priority, Leverage: what else holds the node once `finished` is: its unsatisfied gate
    /// dependencies, its own and inherited (each with how it arose), without those in
    /// `finished` (see [`Derived::finished_by`]) and without its children, which a container
    /// waits on whatever else holds it. Empty when `finished` is all that holds it back. Sorted.
    #[must_use]
    pub fn held_besides(&self, key: &NodeKey, finished: &BTreeSet<NodeKey>) -> Vec<Blocker> {
        let mut held: Vec<Blocker> = self
            .unsatisfied(self.dependencies.of(key, EdgeSet::Pruned))
            .filter(|blocker| {
                !finished.contains(&blocker.node) && blocker.via != DependencyVia::Containment
            })
            .collect();
        held.sort();
        held.dedup();
        held
    }

    /// Gating, Blocked: the ancestors whose own unsatisfied requirements, openings, or
    /// conditions block the node through its entry chains, nearest first; each lists them in
    /// its own [`Derived::blocked_by`]. Empty when the node is not blocked. `graph` is the one
    /// derived.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived.
    #[must_use]
    pub fn blocked_through(&self, graph: &Graph, key: &NodeKey) -> Vec<NodeKey> {
        if !self.blocking.blocked(key) {
            return Vec::new();
        }
        let holders: BTreeSet<NodeKey> = self
            .unsatisfied(self.dependencies.of(key, EdgeSet::Pruned))
            .collect::<Vec<_>>()
            .iter()
            .filter_map(|blocker| held_by(&blocker.via))
            .filter(|holder| *holder != key)
            .cloned()
            .collect();
        let mut nearest_first = Vec::with_capacity(holders.len());
        let mut ancestor = graph.tree().parent(key);
        while let Some(above) = ancestor {
            if holders.contains(above) {
                nearest_first.push(above.clone());
            }
            ancestor = graph.tree().parent(above);
        }
        assert_eq!(
            nearest_first.len(),
            holders.len(),
            "only ancestors pass gates down"
        );
        nearest_first
    }

    /// The unsatisfied gate dependencies among `listed`, with how each arose.
    fn unsatisfied(&self, listed: Vec<EffectiveDependency>) -> impl Iterator<Item = Blocker> + '_ {
        listed
            .into_iter()
            .filter(|dependency| dependency.class == dependencies::EdgeClass::Gate)
            .filter(|dependency| !self.blocking.satisfies(&dependency.node))
            .map(|dependency| Blocker {
                node: dependency.node,
                via: dependency.via,
            })
    }
}

/// The node whose structure makes a dependency: the dependent itself for its own edges and
/// its children, none for those; the ancestor whose requirement, condition, or opening it is.
pub(crate) fn held_by(via: &DependencyVia) -> Option<&NodeKey> {
    match via {
        DependencyVia::Explicit | DependencyVia::Containment => None,
        DependencyVia::Inherited { ancestor } => Some(ancestor),
        DependencyVia::Condition { condition_on } => Some(condition_on),
        DependencyVia::StageOpening { group } => Some(group),
    }
}

/// Derives a journey (ARCHITECTURE, Read path: derive): a pure function of the graph, its
/// state, the journey's `created_on` (the day `created_at` rules measure from, A8), and the
/// inputs (D6). A route version or draft derives as a journey just created from it would,
/// every node in its kind's initial state, with no `created_on`.
///
/// # Panics
///
/// When the graph holds state for some nodes and not others, which validation rejects.
#[must_use]
pub fn derive(journey: &Graph, created_on: Option<Date>, inputs: &DeriveInputs) -> Derived {
    let document = journey.document();
    let state = &document.state.nodes;
    assert!(
        state.is_empty() || state.len() == document.nodes.len(),
        "every node of a journey has a stored state"
    );
    derive_at(
        journey,
        created_on,
        inputs.today,
        &inputs.deployment,
        &inputs.rank,
    )
}

/// Derives a journey at `today` over `deployment` with the rank constants: every pass that
/// reads no viewer, which the validation pipeline's derived guards also run (ARCHITECTURE,
/// Write path: one derive of the final candidate).
#[must_use]
pub(crate) fn derive_at(
    journey: &Graph,
    created_on: Option<Date>,
    today: Date,
    deployment: &Deployment,
    rank: &RankConstants,
) -> Derived {
    let early = early(journey, deployment);
    let participation = participation::pass(journey, &early.relevance, deployment);
    let mut dates = Dates::pass(journey, &early, today, created_on);
    let blocking = Blocking::pass(journey, &early, &dates, today);
    dates.settle_reached(journey, |key| blocking.auto_reached(key));
    let priority = Priority::pass(journey, &early, &participation, &blocking, rank);
    let ranking = Ranking::pass(&priority, &dates, &blocking, rank, |key| {
        priority.leverage(key)
    });
    let stale = stale::Staleness::pass(journey, &early.relevance, &early.dependencies, &blocking);
    let Early {
        relevance,
        dependencies,
        skips,
    } = early;
    Derived {
        relevance,
        dependencies,
        skips,
        participation,
        dates,
        blocking,
        priority,
        ranking,
        rank_constants: *rank,
        stale,
        today,
    }
}

/// Passes 1 and 2, which the plan check reads as well as derive.
pub(crate) struct Early {
    /// Pass 1.
    pub relevance: Relevances,
    /// Pass 2: the effective dependency graph, pruned.
    pub dependencies: Dependencies,
    /// Pass 2: effective skip and kept work.
    pub skips: Skips,
}

/// Runs passes 1 and 2: relevance, the effective dependency graph, and effective skip.
#[must_use]
pub(crate) fn early(journey: &Graph, deployment: &Deployment) -> Early {
    let document = journey.document();
    let inherited = skip::inherited(journey);
    let relevance = relevance::pass(journey, deployment, &inherited);
    let mut dependencies = Dependencies::build(document, journey.tree());
    dependencies.prune(document, &relevance);
    assert_eq!(dependencies.node_count(), document.nodes.len());
    let skips = skip::pass(journey, &relevance, &inherited);
    assert!(skips.effective().all(|(key, _)| relevance.in_scope(key)));
    Early {
        relevance,
        dependencies,
        skips,
    }
}

/// A node's stored state, or its kind's initial state where nothing is stored (a route).
#[must_use]
pub(crate) fn stored_state(document: &Document, node: &Node<KeyRefs>) -> State {
    document
        .state
        .nodes
        .get(&node.key)
        .map_or(State::initial(node.kind()), |stored| stored.state)
}

/// Gating: the node is force-included.
#[must_use]
pub(crate) fn forced(document: &Document, key: &NodeKey) -> bool {
    document
        .state
        .overrides
        .get(key)
        .is_some_and(|overrides| overrides.force_include.is_some())
}

/// D1a: the node is kept under a skipped ancestor.
#[must_use]
pub(crate) fn kept(document: &Document, key: &NodeKey) -> bool {
    document
        .state
        .overrides
        .get(key)
        .is_some_and(|overrides| overrides.keep.is_some())
}
