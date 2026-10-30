//! C2: every node in plan order: each container before what it holds, its children by
//! schedule (a container by the soonest thing beneath it), the undated after the dated, the
//! final milestone last, and key order between any two that tie.
//!
//! Cost: O(nodes log nodes), iterative over an explicit stack.

use std::collections::BTreeMap;

use cairn_schema::{DisplayState, NodeDerived, NodeKey, Payload, When};

use crate::graph::Graph;

/// The soonest date a node has: when it can start, when it is due, or when it was finished. A
/// snoozed node's is the later of that and when its snooze lifts, and none when the hold has no
/// date yet.
fn own_date(graph: &Graph, nodes: &BTreeMap<NodeKey, NodeDerived>, key: &NodeKey) -> Option<When> {
    let derived = nodes.get(key)?;
    let finished = graph
        .document()
        .state
        .nodes
        .get(key)
        .and_then(|stored| stored.finished_on)
        .map(When::Day);
    let due = derived.dates.due.as_ref().map(|bound| bound.date);
    let start = derived
        .dates
        .earliest_start
        .as_ref()
        .map(|bound| When::Day(bound.date));
    let current = [start, due, finished]
        .into_iter()
        .flatten()
        .min()
        // A due time on the day it can start is where it falls, so two of them order by the minute.
        .map(|soonest| match due {
            Some(moment) if moment.is_moment() && moment.day() == soonest.day() => moment,
            _ => soonest,
        });
    if derived.display_state != DisplayState::Snoozed {
        return current;
    }
    let lifts = derived.snooze_lifts?;
    Some(current.map_or(lifts, |current| current.max(lifts)))
}

/// Every node of `graph` in plan order, given each node's derived values.
pub(super) fn plan_order(graph: &Graph, nodes: &BTreeMap<NodeKey, NodeDerived>) -> Vec<NodeKey> {
    let tree = graph.tree();
    // Parents come before their children in this walk, so reversed it settles children first.
    let mut walk: Vec<&NodeKey> = Vec::new();
    let mut stack: Vec<&NodeKey> = tree.roots().iter().collect();
    while let Some(key) = stack.pop() {
        walk.push(key);
        stack.extend(tree.children(key));
    }
    let mut soonest: BTreeMap<&NodeKey, Option<When>> = BTreeMap::new();
    for key in walk.into_iter().rev() {
        let below = tree
            .children(key)
            .iter()
            .filter_map(|child| soonest.get(child).copied().flatten());
        soonest.insert(
            key,
            own_date(graph, nodes, key).into_iter().chain(below).min(),
        );
    }
    let last = |key: &NodeKey| {
        graph.node(key).is_some_and(
            |node| matches!(&node.payload, Payload::Milestone(milestone) if milestone.is_final),
        )
    };
    let sorted = |siblings: &[NodeKey]| {
        let mut siblings = siblings.to_vec();
        siblings.sort_by(|a, b| {
            let (from, to) = (
                soonest.get(a).copied().flatten(),
                soonest.get(b).copied().flatten(),
            );
            last(a)
                .cmp(&last(b))
                .then(from.is_none().cmp(&to.is_none()))
                .then(from.cmp(&to))
                .then(a.cmp(b))
        });
        siblings
    };
    let mut order = Vec::with_capacity(nodes.len());
    let mut stack = sorted(tree.roots());
    stack.reverse();
    while let Some(key) = stack.pop() {
        order.push(key.clone());
        let mut kids = sorted(tree.children(&key));
        kids.reverse();
        stack.extend(kids);
    }
    order
}
