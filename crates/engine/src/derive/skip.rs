//! Effective skip and kept work (D1a; ARCHITECTURE, Read path: derive, pass 2). Skipping a
//! container, or marking a group done, gives its non-terminal, in-scope descendants an
//! effective skip; stored states are untouched, so reopening the container restores them
//! while their own explicit skips stay. A `keep` override exempts a node and its subtree from skips inherited from above
//! it; a skip below the kept node still cascades. Skip travels through containment only: a
//! dependent of a skipped node is not skipped.
//!
//! Kept work: a skipped container, stored or effective, or a group marked done, satisfies its dependents only once
//! the explicitly kept, in-scope work beneath it does ("skipped, kept work pending"). This
//! pass lists that work for each skipped container: every kept, in-scope node beneath it,
//! nested keeps included, since a kept node's own state need not cover what it holds (it may
//! be done while a kept child is reopened, or not relevant while a kept child is
//! force-included). Whether the
//! work satisfies dependencies is blocking's to decide (`deps_done` reads it).
//!
//! Cost at `node_count_max`: the inherited skips are one walk down the tree, O(nodes); kept
//! work and the covered counts are each a walk up from a node, at most `containment_depth_max`
//! steps, so at most 2,000 x 16 steps and as many entries.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{NodeKey, NodeKind, State};

use super::relevance::Relevances;
use super::{kept, skips_beneath, stored_state};
use crate::graph::Graph;

/// Pass 2's skip output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skips {
    /// Each effectively skipped node and the skipped ancestor its skip comes from.
    effective: BTreeMap<NodeKey, NodeKey>,
    /// Each skipped container, stored or effective, or group marked done, and the kept work beneath it.
    kept_work: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
    /// For each container, how many items a skip of it covers (see [`Skips::covered`]).
    covered: BTreeMap<NodeKey, u32>,
}

impl Skips {
    /// D1a: the skipped ancestor, or group marked done, the node's effective skip comes from; none when the node is
    /// not effectively skipped (its stored state may still be `skipped`).
    #[must_use]
    pub fn skipped_by(&self, key: &NodeKey) -> Option<&NodeKey> {
        self.effective.get(key)
    }

    /// D1a: whether the node is skipped, by its own stored state `stored` or by an ancestor.
    #[must_use]
    pub fn skipped(&self, stored: State, key: &NodeKey) -> bool {
        stored == State::Skipped || self.effective.contains_key(key)
    }

    /// Every effectively skipped node, with the ancestor its skip comes from, in key order.
    pub fn effective(&self) -> impl Iterator<Item = (&NodeKey, &NodeKey)> {
        self.effective.iter()
    }

    /// D1a: how many items (nodes that are not groups) a skip of the container covers: the
    /// in-scope, non-terminal ones beneath it that a keep does not exempt and a nearer skipped
    /// container does not already hold. For a container already skipped, or a group marked done,
    /// that is the items skipped through it; for any other, the items skipping it would skip.
    #[must_use]
    pub fn covered(&self, key: &NodeKey) -> u32 {
        self.covered.get(key).copied().unwrap_or(0)
    }

    /// D1a: every skipped container or group marked done that holds kept work, with that work.
    pub fn kept_work_all(&self) -> impl Iterator<Item = (&NodeKey, &BTreeSet<NodeKey>)> {
        self.kept_work.iter()
    }

    /// D1a: the explicitly kept, in-scope work beneath a skipped container or a group marked
    /// done, empty for any
    /// other node.
    #[must_use]
    pub fn kept_work(&self, key: &NodeKey) -> &BTreeSet<NodeKey> {
        static EMPTY: BTreeSet<NodeKey> = BTreeSet::new();
        self.kept_work.get(key).unwrap_or(&EMPTY)
    }
}

/// For every node under a skip it is not kept from, the nearest skipped ancestor or group marked done: one walk
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
            .is_some_and(|node| skips_beneath(document, node));
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
                .is_some_and(|found| skips_beneath(document, found))
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
    skips.covered = covered(graph, relevances);
    let stored_skip = |key: &NodeKey| {
        document
            .nodes
            .get(key)
            .is_some_and(|found| skips_beneath(document, found))
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

/// The items a skip of each container covers, counted from the items up: each in-scope,
/// non-terminal item that is not kept adds one to every container above it, until the walk
/// reaches a kept node (its subtree is exempt from the skips above it) or a node that skips
/// what is beneath it (those items are its own). The depth limit bounds each walk.
fn covered(graph: &Graph, relevances: &Relevances) -> BTreeMap<NodeKey, u32> {
    let document = graph.document();
    let tree = graph.tree();
    let mut counts: BTreeMap<NodeKey, u32> = BTreeMap::new();
    for node in document.nodes.values() {
        let key = &node.key;
        if node.kind() == NodeKind::Group
            || stored_state(document, node).is_terminal()
            || !relevances.in_scope(key)
            || kept(document, key)
        {
            continue;
        }
        let mut below = key;
        while let Some(container) = tree.parent(below) {
            *counts.entry(container.clone()).or_default() += 1;
            let holds = document
                .nodes
                .get(container)
                .is_some_and(|found| skips_beneath(document, found));
            if kept(document, container) || holds {
                break;
            }
            below = container;
        }
    }
    counts
}
