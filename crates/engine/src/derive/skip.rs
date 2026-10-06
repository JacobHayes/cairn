//! Effective skip and kept work (D1a; ARCHITECTURE, Read path: derive, pass 2). Skipping a
//! container gives its non-terminal, in-scope descendants an effective skip; stored states
//! are untouched, so reopening the container restores them while their own explicit skips
//! stay. A `keep` override exempts a node and its subtree from skips inherited from above
//! it; a skip below the kept node still cascades. Skip travels through containment only: a
//! dependent of a skipped node is not skipped.
//!
//! Kept work: a skipped container, stored or effective, satisfies its dependents only once
//! the explicitly kept, in-scope work beneath it does ("skipped, kept work pending"). This
//! pass lists that work for each skipped container: every kept, in-scope node beneath it,
//! nested keeps included, since a kept node's own state need not cover what it holds (it may
//! be done while a kept child is reopened, or not relevant while a kept child is
//! force-included). Whether the
//! work satisfies dependencies is blocking's to decide (2.4: `deps_done` reads it).
//!
//! Cost at `node_count_max`: the inherited skips are one walk down the tree, O(nodes); kept
//! work is a walk up from each kept node, at most `containment_depth_max` steps, so at most
//! 2,000 x 16 steps and as many entries.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{NodeKey, State};

use super::relevance::Relevances;
use super::{kept, stored_state};
use crate::graph::Graph;

/// Pass 2's skip output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skips {
    /// Each effectively skipped node and the skipped ancestor its skip comes from.
    effective: BTreeMap<NodeKey, NodeKey>,
    /// Each skipped container, stored or effective, and the kept work beneath it.
    kept_work: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
}

impl Skips {
    /// D1a: the skipped ancestor the node's effective skip comes from; none when the node is
    /// not effectively skipped (its stored state may still be `skipped`).
    #[must_use]
    pub fn skipped_by(&self, key: &NodeKey) -> Option<&NodeKey> {
        self.effective.get(key)
    }

    /// Every effectively skipped node, with the ancestor its skip comes from, in key order.
    pub fn effective(&self) -> impl Iterator<Item = (&NodeKey, &NodeKey)> {
        self.effective.iter()
    }

    /// D1a: the explicitly kept, in-scope work beneath a skipped container, empty for any
    /// other node.
    #[must_use]
    pub fn kept_work(&self, key: &NodeKey) -> &BTreeSet<NodeKey> {
        static EMPTY: BTreeSet<NodeKey> = BTreeSet::new();
        self.kept_work.get(key).unwrap_or(&EMPTY)
    }
}

/// For every node under a skip it is not kept from, the nearest skipped ancestor: one walk
/// down from the roots with an explicit stack (PRACTICES, No recursion). Depends on stored
/// states and keeps only, never on relevance, so pass 1 can read it.
#[must_use]
pub(crate) fn inherited(graph: &Graph) -> BTreeMap<NodeKey, NodeKey> {
    let document = graph.document();
    let tree = graph.tree();
    let mut found: BTreeMap<NodeKey, NodeKey> = BTreeMap::new();
    let mut stack: Vec<&NodeKey> = tree.roots().iter().collect();
    while let Some(parent) = stack.pop() {
        let parent_skipped = document
            .nodes
            .get(parent)
            .is_some_and(|node| stored_state(document, node) == State::Skipped);
        let from_above = found.get(parent).cloned();
        for child in tree.children(parent) {
            stack.push(child);
            if kept(document, child) {
                continue;
            }
            let source = if parent_skipped {
                Some(parent.clone())
            } else {
                from_above.clone()
            };
            if let Some(source) = source {
                found.insert(child.clone(), source);
            }
        }
    }
    assert!(found.len() <= document.nodes.len());
    found
}

/// Pass 2 (skip): effective skips from the inherited ones, and the kept work beneath each
/// skipped container.
#[must_use]
pub(crate) fn pass(
    graph: &Graph,
    relevances: &Relevances,
    inherited: &BTreeMap<NodeKey, NodeKey>,
) -> Skips {
    let document = graph.document();
    let mut skips = Skips::default();
    for (key, source) in inherited {
        let Some(node) = document.nodes.get(key) else {
            continue;
        };
        if !stored_state(document, node).is_terminal() && relevances.in_scope(key) {
            skips.effective.insert(key.clone(), source.clone());
        }
    }
    for node in document.nodes.values() {
        if !kept(document, &node.key) || !relevances.in_scope(&node.key) {
            continue;
        }
        let mut current = node.parent.as_ref();
        // The depth limit bounds the walk up to the root.
        while let Some(ancestor) = current {
            let skipped = document
                .nodes
                .get(ancestor)
                .is_some_and(|found| stored_state(document, found) == State::Skipped)
                || skips.effective.contains_key(ancestor);
            if skipped {
                let work = skips.kept_work.entry(ancestor.clone()).or_default();
                work.insert(node.key.clone());
            }
            current = document
                .nodes
                .get(ancestor)
                .and_then(|found| found.parent.as_ref());
        }
    }
    let stored_skip = |key: &NodeKey| {
        document
            .nodes
            .get(key)
            .is_some_and(|found| stored_state(document, found) == State::Skipped)
    };
    assert!(
        skips.effective.values().all(stored_skip),
        "an effective skip comes from a stored one"
    );
    assert!(
        skips
            .kept_work
            .keys()
            .all(|key| stored_skip(key) || skips.effective.contains_key(key)),
        "only a skipped container holds kept work"
    );
    skips
}
