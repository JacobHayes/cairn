//! I3, the snapshot: the bounded agent view of a journey, cut from the same derived journey as
//! the domain document. Scoped to a subtree (the node and everything beneath it) and a depth
//! (levels below the subtree's node, roots at depth 1 when there is none), which bounds the
//! node list only; every other part covers the whole subtree. The node list is paged
//! `page_item_count_max` at a time; the acting frontier's first `page_item_count_max` items are
//! the next list's for the subtree (C10), and the rest are keys; the counts add up.
//!
//! Cost at `node_count_max`: one pass over the subtree in tree order, each node's depth by its
//! ancestors (at most 16); a row per listed node on the page and per frontier item shown (at
//! most 2 x 200), each node's blockers off its entry chains (about 1,300 edges each); the next
//! list, O(nodes log nodes).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{
    DisplayState, KindKey, NextQuery, NodeKey, NodeKind, Relevance, Snapshot, SnapshotCounts,
    SnapshotNode, SnapshotScope, State,
};

use super::rows::{count, page};
use super::{DerivedJourney, ProjectionError};

impl DerivedJourney<'_> {
    /// I3: the snapshot of the scope: answers in effect, a page of in-scope nodes with their
    /// state, participations, dates, blocking, and priority, the ranked acting frontier's top N
    /// with the rest as keys, open decisions by rank, placeholders needing breakdown,
    /// unassigned items, shortfalls, the stalled diagnostic, and counts.
    ///
    /// # Errors
    ///
    /// When the subtree is not a node of the journey.
    pub fn snapshot(&self, scope: &SnapshotScope) -> Result<Snapshot, ProjectionError> {
        let subtree = scope.subtree.as_ref();
        let mut scoped: Vec<&NodeKey> = Vec::new();
        if let Some(top) = subtree {
            self.known(top)?;
            scoped.push(top);
        }
        scoped.extend(self.tree_order(subtree));
        let derived = self.derived;
        let relevance = derived.relevance();
        let blocking = derived.blocking();
        let in_scope: Vec<&NodeKey> = scoped
            .iter()
            .copied()
            .filter(|key| relevance.in_scope(key))
            .collect();
        let listed = self.within_depth(&in_scope, subtree, scope.depth);
        let (shown, next) = page(&listed, scope.cursor, PAGE_ITEM_COUNT_MAX);
        let query = NextQuery {
            within: subtree.cloned(),
            ..NextQuery::default()
        };
        let frontier = self.next(&query, &BTreeSet::new())?.items;
        let limit = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap_or(usize::MAX);
        let rest: Vec<NodeKey> = frontier
            .iter()
            .skip(limit)
            .map(|row| row.key.clone())
            .collect();
        let open_decisions: Vec<NodeKey> = in_scope
            .iter()
            .filter(|key| self.node(key).kind() == NodeKind::Decision)
            .filter(|key| self.state(key) == State::Open && !blocking.closed(key))
            .map(|key| (*key).clone())
            .collect();
        let pick = |test: &dyn Fn(&NodeKey) -> bool| -> Vec<NodeKey> {
            in_scope
                .iter()
                .filter(|key| test(key))
                .map(|key| (*key).clone())
                .collect()
        };
        Ok(Snapshot {
            today: derived.today(),
            scope: scope.clone(),
            answers: in_scope
                .iter()
                .filter_map(|key| Some(((*key).clone(), self.answer(key)?)))
                .collect(),
            rationales: in_scope
                .iter()
                .filter_map(|key| Some(((*key).clone(), self.rationale(key)?)))
                .collect(),
            nodes: shown.iter().map(|key| self.snapshot_node(key)).collect(),
            next,
            acting_frontier: frontier.iter().take(limit).cloned().collect(),
            acting_frontier_rest: rest,
            open_decisions: derived.ranking().sorted(&open_decisions),
            needs_breakdown: pick(&|key| blocking.needs_breakdown(key)),
            unassigned: pick(&|key| derived.is_unassigned(self.graph, key)),
            shortfalls: pick(&|key| derived.dates().shortfall_days(key).is_some()),
            stalled: blocking.stalled().cloned(),
            counts: self.counts(&scoped, &in_scope, listed.len(), frontier.len(), subtree),
        })
    }

    /// The in-scope nodes no deeper than `depth` levels below the subtree's node (roots at
    /// depth 1 when there is none).
    fn within_depth(
        &self,
        in_scope: &[&NodeKey],
        subtree: Option<&NodeKey>,
        depth: Option<u32>,
    ) -> Vec<NodeKey> {
        let top = subtree.map_or(0, |top| self.ancestors(top).len() + 1);
        in_scope
            .iter()
            .filter(|key| {
                let below = self.ancestors(key).len() + 1 - top;
                depth.is_none_or(|most| below <= most as usize)
            })
            .map(|key| (*key).clone())
            .collect()
    }

    /// One node in the snapshot.
    fn snapshot_node(&self, key: &NodeKey) -> SnapshotNode {
        let derived = self.derived;
        let mut kinds: Vec<KindKey> = self
            .graph
            .document()
            .participation_kinds
            .as_map()
            .keys()
            .cloned()
            .collect();
        kinds.push(KindKey::owner());
        SnapshotNode {
            row: self.row(key, derived.ranking()),
            participations: kinds
                .into_iter()
                .filter_map(|kind| {
                    let entities = derived.participation().entities(key, &kind);
                    (!entities.is_empty()).then(|| (kind, entities.clone()))
                })
                .collect(),
            blocked_by: derived.blocked_by(key),
            blocked_through: derived.blocked_through(self.graph, key),
            earliest_start: derived.dates().earliest_start(key),
            latest_start: derived.dates().latest_start(key),
        }
    }

    /// I3: the scope's counts.
    fn counts(
        &self,
        scoped: &[&NodeKey],
        in_scope: &[&NodeKey],
        listed: usize,
        acting: usize,
        subtree: Option<&NodeKey>,
    ) -> SnapshotCounts {
        let derived = self.derived;
        let mut by_state: BTreeMap<State, u32> = BTreeMap::new();
        let mut by_display_state: BTreeMap<DisplayState, u32> = BTreeMap::new();
        for key in in_scope {
            *by_state.entry(self.state(key)).or_default() += 1;
            let shown = derived.display_state(self.graph, key);
            *by_display_state.entry(shown).or_default() += 1;
        }
        let frontier = derived
            .ranking()
            .frontier()
            .iter()
            .filter(|key| {
                subtree.is_none_or(|top| *key == top || self.graph.tree().is_ancestor(top, key))
            })
            .count();
        SnapshotCounts {
            in_scope: count(in_scope.len()),
            not_relevant: count(
                scoped
                    .iter()
                    .filter(|key| derived.relevance().value(key) == Relevance::NotRelevant)
                    .count(),
            ),
            by_state,
            by_display_state,
            listed: count(listed),
            frontier: count(frontier),
            acting_frontier: count(acting),
            blocked: count(
                in_scope
                    .iter()
                    .filter(|key| derived.blocking().blocked(key))
                    .count(),
            ),
        }
    }
}
