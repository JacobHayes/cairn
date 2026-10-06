//! The schema's `Derived` (ARCHITECTURE, Read path: `Derived`; D3): the engine's derived
//! values projected onto the shape that crosses the API, now that every pass it carries exists
//! (DECISIONS.md). Each node gets every D3 value with the inputs that explain it; the frontiers
//! are in global rank order. Explanation lists are cut to the response limit with their totals
//! (`explanation_entry_count_max`): inside the engine they are complete and listed on demand,
//! and a host that needs a full list pages it from the engine.
//!
//! Cost at `node_count_max`: per node, its gravity contributors are listed from its downstream
//! set (at most 2,000) and cut to 50, its blockers off its entry chains (about 1,300 edges at
//! most), and its dates' chains; so about 2,000 x 3,300 steps, and 2,000 x 100 explanation
//! entries kept.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Contribution, EffectiveParticipation, Explained, KindKey, NodeDerived, NodeKey, Real,
    RelevanceExplanation,
};

use super::Derived;
use super::relevance::Producer;
use crate::graph::Graph;

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
        let nodes: BTreeMap<NodeKey, NodeDerived> = graph
            .document()
            .nodes
            .as_map()
            .keys()
            .map(|key| (key.clone(), self.node_derived(graph, key)))
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
        let blocking = &self.blocking;
        let priority = &self.priority;
        NodeDerived {
            relevance: self.relevance_explanation(key),
            effectively_skipped: self.skips.skipped_by(key).is_some(),
            blocked_by: self.blocked_by(key),
            blocked_through: self.blocked_through(graph, key),
            actionable: blocking.actionable(key),
            unassigned: self.participation.is_unassigned(key),
            participations: self.participations(graph, key),
            membership_lost: !self.participation.membership_lost(key).is_empty(),
            stale: self.stale(graph, key),
            overdue: self.dates.overdue(key),
            dates: self.dates.node(graph, key),
            auto_reached: blocking.auto_reached(key),
            snoozed: blocking.snoozed(key).cloned(),
            needs_breakdown: blocking.needs_breakdown(key),
            gravity: priority.gravity(key),
            gravity_from: for_response(priority.gravity_from(key)),
            max_child_gravity: priority.max_child_gravity(key),
            leverage: priority.leverage(key),
            leverage_from: for_response(priority.leverage_from(key)),
            rank: self
                .ranking
                .rank(key)
                .map(|rank| Real::try_from(rank).unwrap_or_else(|error| panic!("{error}"))),
        }
    }

    /// C8: the relevance value and what produced it: the node whose condition or force
    /// include decided it, when not the node itself, and the decisions a condition read.
    fn relevance_explanation(&self, key: &NodeKey) -> RelevanceExplanation {
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
        }
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
                let entities = self.participation.entities(key, kind).clone();
                Some((kind.clone(), EffectiveParticipation { entities, origin }))
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
