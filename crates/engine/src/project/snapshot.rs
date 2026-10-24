//! I3, the snapshot: the bounded agent overview of a journey, cut from the same derived journey
//! as the domain document. Scoped to a subtree (the node and everything beneath it). It lists no
//! nodes (the node list does); the acting frontier's first `page_item_count_max` items are the
//! next list's for the subtree (C10), and the rest are keys; the counts add up.
//!
//! Cost at `node_count_max`: one pass over the subtree in tree order, and an entry per frontier
//! item shown (at most 200); the next list, O(nodes log nodes).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{
    DisplayState, FrontierEntry, IdentityKey, JourneyHeader, NextQuery, NodeKey, NodeRef,
    Relevance, Snapshot, SnapshotCounts, SnapshotJourney, SnapshotScope, State,
};

use super::rows::count;
use super::{DerivedJourney, ProjectionError};

impl DerivedJourney<'_> {
    /// I3: the snapshot of the scope: answers in effect, the ranked acting frontier's top N with
    /// the rest as keys, open decisions by rank, placeholders needing breakdown, unassigned
    /// items, shortfalls, the stalled diagnostic, and counts.
    ///
    /// # Errors
    ///
    /// When the subtree is not a node of the journey.
    pub fn snapshot(
        &self,
        scope: &SnapshotScope,
        header: &JourneyHeader,
    ) -> Result<Snapshot, ProjectionError> {
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
            .filter(|key| self.open_decision(key))
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
            journey: self.snapshot_journey(header),
            scope: scope.clone(),
            answers: in_scope
                .iter()
                .filter_map(|key| Some(((*key).clone(), self.answer(key)?)))
                .collect(),
            rationales: in_scope
                .iter()
                .filter_map(|key| Some(((*key).clone(), self.rationale(key)?)))
                .collect(),
            acting_frontier: frontier.iter().take(limit).map(FrontierEntry::of).collect(),
            acting_frontier_rest: rest,
            open_decisions: derived
                .ranking()
                .sorted(&open_decisions)
                .iter()
                .map(|key| NodeRef {
                    key: key.clone(),
                    title: self.node(key).title.clone(),
                })
                .collect(),
            needs_breakdown: pick(&|key| blocking.needs_breakdown(key)),
            unassigned: pick(&|key| derived.is_unassigned(self.graph, key)),
            shortfalls: pick(&|key| derived.dates().shortfall_days(key).is_some()),
            stalled: blocking.stalled().cloned(),
            completion_suggested: self.completion_suggested(),
            counts: self.counts(&scoped, &in_scope, frontier.len(), subtree),
        })
    }

    /// The journey's facts, each owner as the identity it names now, once.
    fn snapshot_journey(&self, header: &JourneyHeader) -> SnapshotJourney {
        let participation = self.derived.participation();
        let owners: BTreeSet<IdentityKey> = header
            .owners
            .iter()
            .map(|owner| participation.canonical_identity(owner).clone())
            .collect();
        SnapshotJourney {
            name: header.name.clone(),
            description: header.description.clone(),
            status: header.status,
            owners: owners.into_iter().collect(),
        }
    }

    /// I3: the scope's counts.
    fn counts(
        &self,
        scoped: &[&NodeKey],
        in_scope: &[&NodeKey],
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
            frontier: count(frontier),
            acting_frontier: count(acting),
        }
    }
}
