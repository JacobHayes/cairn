//! I3, the snapshot: the agent overview of a journey, cut from the same derived journey as the
//! domain document. It lists no nodes: the journey's facts, whether completing it is
//! suggested, and counts, each count of nodes needing attention the total of the node list
//! under the matching filter (C9).
//!
//! Cost at `node_count_max`: one pass over the nodes, testing each against each filter,
//! O(nodes).

use std::collections::BTreeSet;

use cairn_schema::{
    IdentityKey, JourneyHeader, ListFlag, NodeKey, Relevance, Snapshot, SnapshotCounts,
    SnapshotJourney,
};

use super::DerivedJourney;
use super::rows::count;

impl DerivedJourney<'_> {
    /// I3: the snapshot: the journey's facts, whether completing it is suggested, and counts.
    #[must_use]
    pub fn snapshot(&self, header: &JourneyHeader) -> Snapshot {
        Snapshot {
            journey: self.snapshot_journey(header),
            completion_suggested: self.completion_suggested(),
            counts: self.counts(),
        }
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

    /// I3: the counts, over the in-scope nodes.
    fn counts(&self) -> SnapshotCounts {
        let derived = self.derived;
        let in_scope = self.in_scope();
        let (by_state, by_display_state) = self.state_counts(&in_scope);
        let viewer = BTreeSet::new();
        let next_up: BTreeSet<&NodeKey> = derived.ranking().acting_frontier().iter().collect();
        let stalled = self.stalled_nodes();
        let flagged = |flag: ListFlag| {
            count(
                in_scope
                    .iter()
                    .filter(|key| self.flag(flag, key, &viewer, &next_up, &stalled))
                    .count(),
            )
        };
        SnapshotCounts {
            in_scope: count(in_scope.len()),
            not_relevant: count(
                self.graph
                    .document()
                    .nodes
                    .as_map()
                    .keys()
                    .filter(|key| derived.relevance().value(key) == Relevance::NotRelevant)
                    .count(),
            ),
            by_state,
            by_display_state,
            frontier: count(derived.ranking().frontier().len()),
            acting_frontier: flagged(ListFlag::NextUp),
            decisions_needed: flagged(ListFlag::DecisionsNeeded),
            needs_breakdown: flagged(ListFlag::NeedsBreakdown),
            unassigned: flagged(ListFlag::Unassigned),
            shortfalls: flagged(ListFlag::Shortfall),
            stalled: flagged(ListFlag::Stalled),
        }
    }
}
