//! D7: what a patch, or a proposal preview, newly caused in derived state: nodes that became
//! `stale` or gained a stale reason, new or larger shortfalls, newly `overdue` nodes, and the
//! journey becoming `stalled`, each with its explanation. A pure function of the two sides'
//! graphs and derivations, which the caller derives with the same inputs (ARCHITECTURE, Write
//! path), so the passing of midnight is never blamed on a patch. Computed for the response,
//! never stored.
//!
//! Cost at `node_count_max`: one pass over the after side's nodes, O(nodes) lookups, plus the
//! stale reasons of each stale node (at most about 1,300 edges walked each) and the chain of
//! each new or larger shortfall (at most one step per instant slot).

use cairn_schema::{Consequences, NodeKey, ShortfallConsequence, StaleConsequence};

use super::Derived;
use crate::graph::Graph;

/// D7: what the `after` side newly shows against the `before` side. A node the before side
/// does not hold (added by the patch) had nothing before.
///
/// # Panics
///
/// When the two sides were derived for different days, or a derivation is not of its graph.
#[must_use]
pub fn consequences(
    before_graph: &Graph,
    before: &Derived,
    after_graph: &Graph,
    after: &Derived,
) -> Consequences {
    assert_eq!(
        before.today(),
        after.today(),
        "both sides are derived with the same today (D7)"
    );
    let mut found = Consequences::default();
    let existed = |key: &NodeKey| before_graph.node(key).is_some();
    for key in after_graph.document().nodes.as_map().keys() {
        if after.is_stale(key) {
            let mut reasons = after.stale(after_graph, key);
            if existed(key) && before.is_stale(key) {
                let earlier = before.stale(before_graph, key);
                reasons.retain(|reason| !earlier.contains(reason));
            }
            if !reasons.is_empty() {
                found.stale.push(StaleConsequence {
                    node: key.clone(),
                    reasons,
                });
            }
        }
        if let Some(days) = after.dates().shortfall_days(key) {
            let earlier = existed(key)
                .then(|| before.dates().shortfall_days(key))
                .flatten();
            if earlier.is_none_or(|earlier| days > earlier)
                && let Some(shortfall) = after.dates().node(after_graph, key).shortfall
            {
                found.shortfalls.push(ShortfallConsequence {
                    node: key.clone(),
                    shortfall,
                });
            }
        }
        if after.dates().overdue(key) && !(existed(key) && before.dates().overdue(key)) {
            found.overdue.push(key.clone());
        }
    }
    if before.blocking().stalled().is_none() {
        found.stalled = after.blocking().stalled().cloned();
    }
    found
}
