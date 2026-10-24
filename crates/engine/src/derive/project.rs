//! The schema's `Derived` (ARCHITECTURE, Read path: `Derived`; D3): the engine's derived
//! values projected onto the shape that crosses the API, now that every pass it carries exists
//! (decisions/2026-10-06-derive-returns-the-engines-own-derived-growing-a-field-per.md). Each node
//! gets every D3 value with the inputs that explain it; the frontiers are in global rank
//! order. Explanation lists are cut to the response limit with their totals
//! (`explanation_entry_count_max`): inside the engine they are complete and listed on demand,
//! and a host that needs a full list pages it from the engine.
//!
//! Cost at `node_count_max`: per node, its gravity contributors are listed from its downstream
//! set (at most 2,000) and cut to 50, its blockers off its entry chains (about 1,300 edges at
//! most), and its dates' chains; so about 2,000 x 3,300 steps, and 2,000 x 100 explanation
//! entries kept; plus one pass over the tree for container progress (O(nodes)), and per node
//! its waits (the same entry-chain walk) and failing guards (a lookup).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Blocker, Contribution, DateLink, DependencyVia, EffectiveParticipation, Explained, Guard,
    GuardFailure, KindKey, NodeDates, NodeDerived, NodeKey, Progress, Real, Relevance,
    RelevanceExplanation,
};

use super::checklist::checklist_count;
use super::dependencies::EdgeSet;
use super::links::date_links_by_node;
use super::relevance::Producer;
use super::stale::{Annotated, annotated_nodes, static_failures};
use super::{Derived, held_by, stored_state};
use crate::graph::Graph;
use crate::transition::{self, Move};

impl Derived {
    /// The schema's `Derived` for `graph`, the graph derived (ARCHITECTURE, Read path; D3):
    /// every node's derived values and explanation inputs, explanation lists cut to the
    /// response limit with their totals, the frontiers in global rank order, and the stalled
    /// diagnostic.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived.
    #[must_use]
    pub fn to_schema(&self, graph: &Graph) -> cairn_schema::Derived {
        let mut progress = self.progress(graph);
        let annotated = annotated_nodes(graph.document());
        let mut links = date_links_by_node(&self.authored_links(graph));
        let nodes: BTreeMap<NodeKey, NodeDerived> = graph
            .document()
            .nodes
            .as_map()
            .keys()
            .map(|key| {
                let progress = progress.remove(key);
                (
                    key.clone(),
                    self.node_with(
                        graph,
                        key,
                        progress,
                        &annotated,
                        links.remove(key).unwrap_or_default(),
                    ),
                )
            })
            .collect();
        assert_eq!(
            nodes.len(),
            self.dependencies.node_count(),
            "the graph derived"
        );
        cairn_schema::Derived {
            today: self.today,
            nodes,
            frontier: self.ranking.frontier().to_vec(),
            acting_frontier: self.ranking.acting_frontier().to_vec(),
            stalled: self.blocking.stalled().cloned(),
        }
    }

    /// One node's derived values (D3) with their explanation inputs, explanation lists cut
    /// to the response limit with their totals: what node detail (C8) carries, without
    /// projecting every other node.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived, or does not hold `key`.
    #[must_use]
    pub fn node_derived(&self, graph: &Graph, key: &NodeKey) -> NodeDerived {
        let progress = self.progress(graph).remove(key);
        self.node_with(
            graph,
            key,
            progress,
            &annotated_nodes(graph.document()),
            self.date_links_of(graph, key),
        )
    }

    /// [`Derived::node_derived`] with the shared reads done once by the caller.
    fn node_with(
        &self,
        graph: &Graph,
        key: &NodeKey,
        progress: Option<Progress>,
        annotated: &Annotated<'_>,
        date_links: Vec<DateLink>,
    ) -> NodeDerived {
        let blocking = &self.blocking;
        let priority = &self.priority;
        let waits_on = self.waits_on(graph, key);
        let waits_total = waits_on.len();
        let waits_on = Explained::complete(waits_on)
            .unwrap_or_else(|error| panic!("{error}"))
            .for_response();
        assert_eq!(usize::try_from(waits_on.total).ok(), Some(waits_total));
        NodeDerived {
            relevance: self.relevance_explanation(graph, key),
            display_state: self.display_state(graph, key),
            skipped_by: self.skips.skipped_by(key).cloned(),
            kept_work_pending: self
                .skips
                .kept_work(key)
                .iter()
                .filter(|kept| !blocking.satisfies(kept))
                .cloned()
                .collect(),
            blocked_by: self.blocked_by(key),
            blocked_through: self.blocked_through(graph, key),
            actionable: blocking.actionable(key),
            unassigned: self.is_unassigned(graph, key),
            participations: self.participations(graph, key),
            membership_lost: !self.participation.membership_lost(key).is_empty(),
            stale: self.stale(graph, key),
            overdue: self.dates.overdue(key),
            dates: NodeDates {
                links: date_links,
                ..self.dates.node(graph, key)
            },
            progress,
            checklist_count: graph
                .node(key)
                .and_then(|node| checklist_count(graph.document(), node)),
            failing_guards: self.failing_guards(graph, key, annotated),
            waits_on,
            done_by_date: blocking.done_by_date(key),
            snoozed: blocking.snoozed(key).cloned(),
            snoozed_via: blocking.snoozed_via(key).cloned(),
            needs_breakdown: blocking.needs_breakdown(key),
            gravity: priority.gravity(key),
            gravity_from: for_response(priority.gravity_from(key)),
            subtree_gravity: priority.subtree_gravity(key),
            unlocks: priority.unlocks(key),
            unlocks_from: for_response(priority.unlocks_from(key)),
            rank: self
                .ranking
                .rank(key)
                .map(|rank| Real::try_from(rank).unwrap_or_else(|error| panic!("{error}"))),
        }
    }

    /// C8: the relevance value and what produced it: the node whose condition or force
    /// include decided it, when not the node itself, and the decisions a condition read; D8:
    /// the undecided decisions a `not_relevant` value is pending on.
    fn relevance_explanation(&self, graph: &Graph, key: &NodeKey) -> RelevanceExplanation {
        let Some(found) = self.relevance.get(key) else {
            panic!("the graph derived holds {key}");
        };
        let elsewhere = |on: &NodeKey| (on != key).then(|| on.clone());
        let (condition_on, decisions, forced) = match &found.producer {
            Producer::Unconditioned => (None, BTreeSet::new(), false),
            Producer::Condition { on, decisions } => (elsewhere(on), decisions.clone(), false),
            Producer::Forced { on } => (elsewhere(on), BTreeSet::new(), true),
        };
        RelevanceExplanation {
            value: found.value,
            condition_on,
            decisions,
            forced,
            pending_on: found.pending_on.clone(),
            unanswered: if found.value == Relevance::Undecided {
                self.unanswered(graph, key)
            } else {
                BTreeSet::new()
            },
        }
    }

    /// D4: the bypassable guards completing the node would fail if attempted now with no
    /// bypass in the patch: the evidence a deliverable or action still lacks. Empty for any
    /// other node, and for one out of scope or closed. A bypass recorded by an earlier patch
    /// removes nothing: apply accepts only a bypass in the same patch. `annotated` is
    /// [`annotated_nodes`] of `graph`.
    fn failing_guards(
        &self,
        graph: &Graph,
        key: &NodeKey,
        annotated: &Annotated<'_>,
    ) -> BTreeSet<Guard> {
        let document = graph.document();
        let Some(node) = graph.node(key) else {
            panic!("the graph holds {key}");
        };
        let completes =
            transition::next(node.kind(), stored_state(document, node), Move::Complete).is_some();
        if !completes || !self.relevance.in_scope(key) || self.blocking.closed(key) {
            return BTreeSet::new();
        }
        static_failures(document, graph.tree(), node, annotated)
            .iter()
            .map(GuardFailure::guard)
            .collect()
    }

    /// Gating, D8: the unsatisfied gate dependencies holding the node now, its own and those
    /// it inherits (its children aside), one per dependency node, with how the nearest holder
    /// makes it: its own first, then the nearest ancestor's, then by key. Empty when the node
    /// is not blocked.
    fn waits_on(&self, graph: &Graph, key: &NodeKey) -> Vec<Blocker> {
        if !self.blocking.blocked(key) {
            return Vec::new();
        }
        let mut distance: BTreeMap<&NodeKey, u32> = BTreeMap::new();
        let mut ancestor = graph.tree().parent(key);
        let mut steps = 1u32;
        while let Some(above) = ancestor {
            distance.insert(above, steps);
            steps += 1;
            ancestor = graph.tree().parent(above);
        }
        // Own dependencies are nearest (0); an ancestor's, by how far up it is.
        let nearness = |blocker: &Blocker| {
            held_by(&blocker.via).map_or(0, |holder| {
                if holder == key {
                    0
                } else {
                    distance.get(holder).copied().unwrap_or(u32::MAX)
                }
            })
        };
        let mut held: Vec<(u32, Blocker)> = self
            .unsatisfied(self.dependencies.of(key, EdgeSet::Pruned))
            .filter(|blocker| blocker.via != DependencyVia::Containment)
            .map(|blocker| (nearness(&blocker), blocker))
            .collect();
        held.sort_by(|left, right| {
            (left.0, &left.1.node, &left.1.via).cmp(&(right.0, &right.1.node, &right.1.via))
        });
        // The nearest holder keeps a dependency that several of them hold.
        let mut seen = BTreeSet::new();
        held.into_iter()
            .filter_map(|(_, blocker)| seen.insert(blocker.node.clone()).then_some(blocker))
            .collect()
    }

    /// E2: the node's effective participation of each kind that resolves to something.
    fn participations(
        &self,
        graph: &Graph,
        key: &NodeKey,
    ) -> BTreeMap<KindKey, EffectiveParticipation> {
        let mut kinds: BTreeSet<&KindKey> = graph
            .document()
            .participation_kinds
            .as_map()
            .keys()
            .collect();
        let owner = KindKey::owner();
        kinds.insert(&owner);
        kinds
            .into_iter()
            .filter_map(|kind| {
                let origin = self.participation.origin(key, kind)?.clone();
                let identities = self.participation.identities(key, kind).clone();
                Some((kind.clone(), EffectiveParticipation { identities, origin }))
            })
            .collect()
    }
}

/// A complete, ordered explanation list cut to the response limit, with its total.
fn for_response(entries: Vec<Contribution>) -> Explained<Contribution> {
    let total = entries.len();
    let complete = Explained::complete(entries).unwrap_or_else(|error| panic!("{error}"));
    let cut = complete.for_response();
    assert_eq!(
        usize::try_from(cut.total).ok(),
        Some(total),
        "the total survives the cut"
    );
    cut
}
