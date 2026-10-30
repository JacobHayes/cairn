//! C7, trace: a node's upstream and downstream sets over the full structural graph's gate
//! edges (ARCHITECTURE, Read path: tracing reads the full set), so terminal and not-relevant
//! nodes are included and levels do not matter.
//!
//! Upstream is every other node whose finish the node's finish waits on, transitively: its
//! requirements, inherited ones, gating decisions, and its children. Downstream
//! is every other node with an instant that waits on the node's finish, transitively: its
//! dependents, its ancestors (each waits on its children), and the nodes whose relevance it
//! determines (condition gates). Every instant of a node leads to its finish, so a node is
//! upstream of another exactly when the other is downstream of it. The gravity contributors
//! (pass 6, over the pruned set) are marked within the downstream set.
//!
//! A container that skips what is beneath it (a skipped one, or a group marked done) waits
//! only on the kept work still pending beneath it, so its other children are in neither set.
//!
//! A card that stands for hidden nodes (a collapsed stage, a deliverable with its actions rolled
//! up) traces what they trace: the walks start from the node and every node it stands for, in
//! one pass, and what the card stands for is not part of the answer.
//!
//! Cost at `node_count_max`: two walks over at most 8,000 instants and about 110,000 edges,
//! O(instants + edges) each, with a flag per instant, however many nodes the card stands for.

use std::collections::BTreeSet;

use cairn_schema::{NodeKey, Trace};

use super::{DerivedJourney, ProjectionError};
use crate::derive::dependencies::{Dependencies, Edge, EdgeSet, EdgeSource, Instant, Point};
use crate::derive::skips_beneath;

/// Which way a walk follows the edges.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// From a dependent to what it waits on.
    Up,
    /// From a requirement to what waits on it.
    Down,
}

impl DerivedJourney<'_> {
    /// C7: the node's upstream and downstream sets, with its gravity contributors marked.
    ///
    /// # Errors
    ///
    /// When the node is not in the journey.
    pub fn trace(&self, key: &NodeKey) -> Result<Trace, ProjectionError> {
        self.trace_standing(key, &[])
    }

    /// C7: the trace of a card that stands for the nodes `standing` as well as `key`: the sets
    /// of all of them together, less the nodes themselves.
    ///
    /// # Errors
    ///
    /// When the node, or one it stands for, is not in the journey.
    pub fn trace_standing(
        &self,
        key: &NodeKey,
        standing: &[NodeKey],
    ) -> Result<Trace, ProjectionError> {
        self.known(key)?;
        for each in standing {
            self.known(each)?;
        }
        let starts: BTreeSet<&NodeKey> = std::iter::once(key).chain(standing).collect();
        let dependencies = self.derived.dependencies();
        let kept_paths = self.kept_paths();
        let waits_on_child = |edge: &Edge| self.waits_on_child(dependencies, &kept_paths, edge);
        let upstream = walk(dependencies, &starts, Direction::Up, &waits_on_child);
        let downstream = walk(dependencies, &starts, Direction::Down, &waits_on_child);
        let gravity_contributors: Vec<NodeKey> = starts
            .iter()
            .flat_map(|start| self.derived.priority().gravity_from(start))
            .map(|contribution| contribution.node)
            .filter(|node| !starts.contains(node))
            // Gravity reads the pruned set, which still holds the containment a skipping container
            // ignores; only what the trace shows as unblocked is marked.
            .filter(|node| downstream.binary_search(node).is_ok())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(Trace {
            node: key.clone(),
            upstream,
            downstream,
            gravity_contributors,
        })
    }
}

impl DerivedJourney<'_> {
    /// D1a: the nodes a skipping container still waits on: each kept item that is still pending
    /// and the containers between it and the skipping one, found once per trace.
    fn kept_paths(&self) -> BTreeSet<&NodeKey> {
        let blocking = self.derived.blocking();
        let mut held = BTreeSet::new();
        for (container, work) in self.derived.skips().kept_work_all() {
            for kept in work.iter().filter(|kept| !blocking.satisfies(kept)) {
                let mut at = Some(kept);
                // The depth limit bounds the climb to the container.
                while let Some(key) = at.filter(|key| *key != container) {
                    held.insert(key);
                    at = self.graph.tree().parent(key);
                }
            }
        }
        held
    }

    /// D1a: whether a containment edge still holds: a container that skips what is beneath it
    /// (a skipped one, or a group marked done) waits only on the kept work still pending
    /// beneath it, so its other children are neither needed by it nor unblocked by it.
    fn waits_on_child(
        &self,
        dependencies: &Dependencies,
        kept_paths: &BTreeSet<&NodeKey>,
        edge: &Edge,
    ) -> bool {
        if edge.source != EdgeSource::Containment {
            return true;
        }
        let (Some(parent), Some(child)) = (
            dependencies.key(edge.dependent.node),
            dependencies.key(edge.requirement.node),
        ) else {
            return true;
        };
        let Some(node) = self.graph.node(parent) else {
            return true;
        };
        let skipping = skips_beneath(self.graph.document(), node)
            || self.derived.skips().skipped_by(parent).is_some();
        !skipping || kept_paths.contains(child)
    }
}

/// The other nodes reached from the start nodes' finishes along full-set gate edges: upstream,
/// those whose finish is reached; downstream, those with any instant reached. The start nodes
/// are not in the answer. In key order.
fn walk(
    dependencies: &Dependencies,
    starts: &BTreeSet<&NodeKey>,
    direction: Direction,
    holds: &dyn Fn(&Edge) -> bool,
) -> Vec<NodeKey> {
    let mut seen: BTreeSet<Instant> = BTreeSet::new();
    let mut stack = Vec::new();
    let mut own = BTreeSet::new();
    for key in starts {
        if let Some(node) = dependencies.node_index(key) {
            let start = Instant::new(node, Point::Finish);
            own.insert(node);
            if seen.insert(start) {
                stack.push(start);
            }
        }
    }
    let mut found: BTreeSet<&NodeKey> = BTreeSet::new();
    while let Some(instant) = stack.pop() {
        let edges: Box<dyn Iterator<Item = &Edge>> = match direction {
            Direction::Up => Box::new(dependencies.waits(instant, EdgeSet::Full)),
            Direction::Down => Box::new(dependencies.waited_by(instant, EdgeSet::Full)),
        };
        for edge in edges.filter(|edge| holds(edge)) {
            let next = match direction {
                Direction::Up => edge.requirement,
                Direction::Down => edge.dependent,
            };
            if !seen.insert(next) {
                continue;
            }
            stack.push(next);
            let counts = direction == Direction::Down || next.point == Point::Finish;
            if counts && !own.contains(&next.node) {
                found.extend(dependencies.key(next.node));
            }
        }
    }
    found.into_iter().cloned().collect()
}
