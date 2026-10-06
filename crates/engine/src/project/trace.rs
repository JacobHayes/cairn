//! C7, trace: a node's upstream and downstream sets over the full structural graph's gate
//! edges (ARCHITECTURE, Read path: tracing reads the full set), so terminal and not-relevant
//! nodes are included and levels do not matter.
//!
//! Upstream is every other node whose finish the node's finish waits on, transitively: its
//! requirements, inherited ones, gating decisions, stage openings, and its children. Downstream
//! is every other node with an instant that waits on the node's finish, transitively: its
//! dependents, its ancestors (each waits on its children), and the nodes whose relevance it
//! determines (condition gates). Every instant of a node leads to its finish, so a node is
//! upstream of another exactly when the other is downstream of it. The gravity contributors
//! (pass 6, over the pruned set) are marked within the downstream set.
//!
//! Cost at `node_count_max`: two walks over at most 8,000 instants and about 110,000 edges,
//! O(instants + edges) each, with a flag per instant.

use std::collections::BTreeSet;

use cairn_schema::{NodeKey, Trace};

use super::{DerivedJourney, ProjectionError};
use crate::derive::dependencies::{Dependencies, Edge, EdgeClass, EdgeSet, Instant, Point};

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
    ///
    /// # Panics
    ///
    /// When a gravity contributor falls outside the downstream set, which pass 6's pruned set
    /// being a subset of the full one rules out.
    pub fn trace(&self, key: &NodeKey) -> Result<Trace, ProjectionError> {
        self.known(key)?;
        let dependencies = self.derived.dependencies();
        let upstream = walk(dependencies, key, Direction::Up);
        let downstream = walk(dependencies, key, Direction::Down);
        let gravity_contributors: Vec<NodeKey> = self
            .derived
            .priority()
            .gravity_from(key)
            .into_iter()
            .map(|contribution| contribution.node)
            .filter(|node| node != key)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        assert!(
            gravity_contributors
                .iter()
                .all(|node| downstream.binary_search(node).is_ok()),
            "gravity's set is within the downstream set"
        );
        Ok(Trace {
            node: key.clone(),
            upstream,
            downstream,
            gravity_contributors,
        })
    }
}

/// The other nodes reached from the node's finish along full-set gate edges: upstream, those
/// whose finish is reached; downstream, those with any instant reached. In key order.
fn walk(dependencies: &Dependencies, key: &NodeKey, direction: Direction) -> Vec<NodeKey> {
    let Some(node) = dependencies.node_index(key) else {
        return Vec::new();
    };
    let start = Instant::new(node, Point::Finish);
    let mut seen: BTreeSet<Instant> = BTreeSet::from([start]);
    let mut stack = vec![start];
    let mut found: BTreeSet<&NodeKey> = BTreeSet::new();
    while let Some(instant) = stack.pop() {
        let edges: Box<dyn Iterator<Item = &Edge>> = match direction {
            Direction::Up => Box::new(dependencies.waits(instant, EdgeSet::Full)),
            Direction::Down => Box::new(dependencies.waited_by(instant, EdgeSet::Full)),
        };
        for edge in edges.filter(|edge| edge.class == EdgeClass::Gate) {
            let next = match direction {
                Direction::Up => edge.requirement,
                Direction::Down => edge.dependent,
            };
            if !seen.insert(next) {
                continue;
            }
            stack.push(next);
            let counts = direction == Direction::Down || next.point == Point::Finish;
            if counts && next.node != node {
                found.extend(dependencies.key(next.node));
            }
        }
    }
    found.into_iter().cloned().collect()
}
