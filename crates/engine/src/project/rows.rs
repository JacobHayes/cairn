//! One node as a row (C9, C10, I3), the orders a list sorts by, and who participates where
//! (E4). Shared by the next list, the list, `mine`, and the snapshot.
//!
//! Cost at `node_count_max`: a row reads each pass's per-node values and walks the node's
//! ancestors (at most 16); sorting is O(nodes log nodes) comparisons.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use cairn_schema::{Cursor, EntityKey, KindKey, NodeKey, NodeRow, Real, SortBy};

use super::DerivedJourney;
use crate::derive::Ranking;

impl DerivedJourney<'_> {
    /// The node's row (C9, C10), its leverage and rank from `ranking`: the global one, or a
    /// viewer's.
    ///
    /// # Panics
    ///
    /// When the node is not in the graph or no root reaches it, which a valid graph rules out.
    pub(crate) fn row(&self, key: &NodeKey, ranking: &Ranking) -> NodeRow {
        let node = self.node(key);
        let derived = self.derived;
        let blocking = derived.blocking();
        let dates = derived.dates();
        let priority = derived.priority();
        NodeRow {
            key: key.clone(),
            path: self
                .graph
                .tree()
                .path(key)
                .cloned()
                .unwrap_or_else(|| panic!("a root reaches {key}")),
            kind: node.kind(),
            title: node.title.clone(),
            state: self.state(key),
            display_state: derived.display_state(self.graph, key),
            ancestors: self.ancestors(key),
            relevance: derived.relevance().value(key),
            actionable: blocking.actionable(key),
            blocked: blocking.blocked(key),
            effectively_skipped: derived.skips().skipped_by(key).is_some(),
            owners: self.owners(key).clone(),
            unassigned: derived.is_unassigned(self.graph, key),
            snoozed: blocking.snoozed(key).cloned(),
            overdue: dates.overdue(key),
            stale: derived.is_stale(key),
            needs_breakdown: blocking.needs_breakdown(key),
            shortfall_days: dates.shortfall_days(key),
            slack_days: dates.slack_days(key),
            due: dates.due(key),
            gravity: priority.gravity(key),
            leverage: ranking
                .leverage(key)
                .unwrap_or_else(|| priority.leverage(key)),
            rank: ranking.terms(key).map(|terms| cairn_schema::RankTerms {
                urgency: real(terms.urgency),
                late: real(terms.late),
                gravity_norm: real(terms.gravity_norm),
                leverage_norm: real(terms.leverage_norm),
                rank: real(terms.rank),
            }),
        }
    }

    /// The keys sorted by one signal (C9, C10), ties in `ranking`'s order.
    pub(crate) fn sorted(&self, keys: &[NodeKey], by: SortBy, ranking: &Ranking) -> Vec<NodeKey> {
        let dates = self.derived.dates();
        let priority = self.derived.priority();
        let leverage = |key: &NodeKey| {
            ranking
                .leverage(key)
                .unwrap_or_else(|| priority.leverage(key))
        };
        let signal: Order<'_> = match by {
            SortBy::Rank => return ranking.sorted(keys),
            SortBy::Effort => return ranking.by_effort(self.graph, priority, keys),
            SortBy::Slack => Box::new(|a, b| least_first(dates.slack_days(a), dates.slack_days(b))),
            SortBy::Due => Box::new(|a, b| least_first(dates.due(a), dates.due(b))),
            SortBy::Gravity => Box::new(|a, b| priority.gravity(b).cmp(&priority.gravity(a))),
            SortBy::Leverage => Box::new(|a, b| leverage(b).cmp(&leverage(a))),
        };
        let mut sorted = keys.to_vec();
        sorted.sort_by(|a, b| signal(a, b).then_with(|| ranking.compare(a, b)));
        sorted
    }

    /// E4: the participation kinds the viewer's entities hold on the node.
    pub(crate) fn kinds_held(
        &self,
        key: &NodeKey,
        viewer: &BTreeSet<EntityKey>,
    ) -> BTreeSet<KindKey> {
        let participation = self.derived.participation();
        let mut kinds: Vec<KindKey> = self
            .graph
            .document()
            .participation_kinds
            .as_map()
            .keys()
            .cloned()
            .collect();
        kinds.push(KindKey::owner());
        kinds
            .into_iter()
            .filter(|kind| !participation.entities(key, kind).is_disjoint(viewer))
            .collect()
    }
}

/// One signal's order between two nodes.
type Order<'a> = Box<dyn Fn(&NodeKey, &NodeKey) -> Ordering + 'a>;

/// Smaller first, none last.
fn least_first<T: Ord>(first: Option<T>, second: Option<T>) -> Ordering {
    match (first, second) {
        (Some(first), Some(second)) => first.cmp(&second),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// A rank term, which pass 7 asserts is finite and non-negative.
fn real(value: f64) -> Real {
    Real::try_from(value).unwrap_or_else(|error| panic!("{error}"))
}

/// A page of `items` from the cursor, at most `size`, with the cursor after it when more
/// remain.
pub(crate) fn page<T: Clone>(items: &[T], cursor: Cursor, size: u32) -> (Vec<T>, Option<Cursor>) {
    assert!(size > 0, "a page holds an item");
    let from = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
    let size = usize::try_from(size).unwrap_or(usize::MAX);
    let kept: Vec<T> = items.iter().skip(from).take(size).cloned().collect();
    let end = from.saturating_add(kept.len());
    let next = (end < items.len()).then(|| Cursor::at(u32::try_from(end).unwrap_or(u32::MAX)));
    (kept, next)
}

/// A count of at most `node_count_max` or a page's items, as the schema's `u32`.
pub(crate) fn count(items: usize) -> u32 {
    u32::try_from(items).unwrap_or_else(|_| panic!("{items} items fit a u32"))
}
