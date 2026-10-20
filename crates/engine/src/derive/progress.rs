//! Container progress (C2, C4): done, total, overdue and decisions needed over the work
//! beneath each container, one rule for every surface.
//!
//! A counted item is a node that is not a group, is in scope (relevant or undecided), and is
//! neither stored `skipped` nor skipped through a container. Every proper descendant is
//! visited, so a kept node under a skipped container and a force-included node under a
//! not-relevant one count on their own terms. A group is structure, not work, so an empty
//! group counts for nothing.
//!
//! Cost at `node_count_max`: one post-order pass over the tree, O(nodes) with B-tree lookups
//! and one `Progress` per container; no per-container descendant set.

use std::collections::BTreeMap;

use cairn_schema::{NodeKey, NodeKind, Progress, State};

use super::{Derived, stored_state};
use crate::graph::Graph;

impl Derived {
    /// C2, C4: the progress of every container of `graph` (the one derived), over the work
    /// beneath it. A node with no children has no entry.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived.
    #[must_use]
    pub fn progress(&self, graph: &Graph) -> BTreeMap<NodeKey, Progress> {
        let tree = graph.tree();
        // Parents before children; reversed, every child is settled before its parent.
        let mut order: Vec<&NodeKey> = Vec::new();
        let mut stack: Vec<&NodeKey> = tree.roots().iter().collect();
        while let Some(key) = stack.pop() {
            order.push(key);
            stack.extend(tree.children(key));
        }
        assert!(order.len() <= graph.document().nodes.len());
        let mut found: BTreeMap<NodeKey, Progress> = BTreeMap::new();
        for key in order.into_iter().rev() {
            let children = tree.children(key);
            if children.is_empty() {
                continue;
            }
            let mut sum = Progress::default();
            for child in children {
                let below = found.get(child).copied().unwrap_or_default();
                sum.done += below.done;
                sum.total += below.total;
                sum.overdue += below.overdue;
                sum.decisions_needed += below.decisions_needed;
                self.count_item(graph, child, &mut sum);
            }
            assert!(
                sum.done <= sum.total
                    && sum.overdue <= sum.total
                    && sum.decisions_needed <= sum.total,
                "progress counts are of the total"
            );
            found.insert(key.clone(), sum);
        }
        found
    }

    /// Adds `key` to `sum` when it is a counted item.
    fn count_item(&self, graph: &Graph, key: &NodeKey, sum: &mut Progress) {
        let Some(node) = graph.node(key) else {
            panic!("the graph holds {key}");
        };
        let state = stored_state(graph.document(), node);
        if node.kind() == NodeKind::Group
            || !self.relevance.in_scope(key)
            || state == State::Skipped
            || self.skips.skipped_by(key).is_some()
        {
            return;
        }
        sum.total += 1;
        if state.is_terminal() || self.blocking.auto_reached(key) {
            sum.done += 1;
        }
        if self.dates.overdue(key) {
            sum.overdue += 1;
        }
        if node.kind() == NodeKind::Decision && self.blocking.actionable(key) {
            sum.decisions_needed += 1;
        }
    }
}
