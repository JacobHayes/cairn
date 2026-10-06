//! Dependency cycles over explicit and containment edges (PRD Invariants: the dependency
//! graph is acyclic; Containment). Each node has a start and a finish, and a container also
//! has an entry (ARCHITECTURE, Read path: derive): a child's entry or start waits for its
//! parent's entry, the parent's start waits for each child's finish, a node's finish waits for
//! its start, and an explicit `requires` makes the dependent's entry or start wait for the
//! requirement's finish. A requirement on a container thus reaches its descendants through
//! the entry chain, and a cycle among these instants is a dependency cycle, inherited edges
//! included. Condition gates and stage openings join the graph with relevance (2.2).
//!
//! Cost at the limits: at most 3 x 2,000 instants and 3 x 2,000 + 64 x 2,000 waits; finding
//! the strongly connected components is two iterative depth-first passes, O(instants + waits).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{NodeKey, Subject, Violation, ViolationCode};

use super::GraphCheck;

/// An instant: a node's start, finish, or entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    Start,
    Finish,
    Entry,
}

/// The instants and their waits: `waits[i]` lists the instants instant `i` waits for.
struct Network<'a> {
    nodes: Vec<&'a NodeKey>,
    waits: Vec<Vec<usize>>,
}

impl<'a> Network<'a> {
    fn build(check: &GraphCheck<'a>) -> Self {
        let nodes: Vec<&NodeKey> = check.document.nodes.as_map().keys().collect();
        let index: BTreeMap<&NodeKey, usize> =
            nodes.iter().enumerate().map(|(i, key)| (*key, i)).collect();
        let mut network = Network {
            waits: vec![Vec::new(); nodes.len() * 3],
            nodes,
        };
        let tree = check.tree;
        for (i, key) in network.nodes.clone().into_iter().enumerate() {
            network.wait(i, Point::Finish, i, Point::Start);
            if tree.is_container(key) {
                network.wait(i, Point::Start, i, Point::Entry);
            }
            // Only children a root reaches: a parent cycle is reported as containment.
            for child in tree.children(key) {
                let (Some(&c), Some(_)) = (index.get(child), tree.path(child)) else {
                    continue;
                };
                network.wait(c, anchor(tree, child), i, Point::Entry);
                network.wait(i, Point::Start, c, Point::Finish);
            }
        }
        for node in check.document.nodes.values() {
            let Some(&dependent) = index.get(&node.key) else {
                continue;
            };
            for requirement in node.requires.iter() {
                // Edges to self, ancestors, or descendants are reported on their own (A3).
                let related = *requirement == node.key
                    || tree.is_ancestor(requirement, &node.key)
                    || tree.is_ancestor(&node.key, requirement);
                if let (Some(&required), false) = (index.get(requirement), related) {
                    network.wait(dependent, anchor(tree, &node.key), required, Point::Finish);
                }
            }
        }
        network
    }

    fn wait(&mut self, from: usize, from_point: Point, to: usize, to_point: Point) {
        let source = instant(from, from_point);
        if let Some(waits) = self.waits.get_mut(source) {
            waits.push(instant(to, to_point));
        }
    }

    /// The strongly connected components with more than one instant (Kosaraju, iterative).
    fn cycles(&self) -> Vec<BTreeSet<&'a NodeKey>> {
        let order = self.finish_order();
        let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); self.waits.len()];
        for (from, targets) in self.waits.iter().enumerate() {
            for &to in targets {
                if let Some(sources) = reverse.get_mut(to) {
                    sources.push(from);
                }
            }
        }
        let mut component = vec![usize::MAX; self.waits.len()];
        let mut cycles = Vec::new();
        for &root in order.iter().rev() {
            if component.get(root) != Some(&usize::MAX) {
                continue;
            }
            let mut members = Vec::new();
            let mut stack = vec![root];
            while let Some(next) = stack.pop() {
                match component.get_mut(next) {
                    Some(slot) if *slot == usize::MAX => *slot = root,
                    Some(_) | None => continue,
                }
                members.push(next);
                stack.extend(reverse.get(next).into_iter().flatten());
            }
            if members.len() > 1 {
                let keys = members
                    .iter()
                    .filter_map(|i| self.nodes.get(i / 3).copied());
                cycles.push(keys.collect());
            }
        }
        cycles
    }

    /// Instants in the order a depth-first search finishes them.
    fn finish_order(&self) -> Vec<usize> {
        let mut visited = vec![false; self.waits.len()];
        let mut order = Vec::with_capacity(self.waits.len());
        for start in 0..self.waits.len() {
            if visited.get(start) == Some(&true) {
                continue;
            }
            // (instant, next wait to explore)
            let mut stack = vec![(start, 0_usize)];
            if let Some(seen) = visited.get_mut(start) {
                *seen = true;
            }
            while let Some((current, next)) = stack.pop() {
                let targets = self.waits.get(current).map_or(&[][..], Vec::as_slice);
                if let Some(&target) = targets.get(next) {
                    stack.push((current, next + 1));
                    if visited.get(target) == Some(&false) {
                        if let Some(seen) = visited.get_mut(target) {
                            *seen = true;
                        }
                        stack.push((target, 0));
                    }
                } else {
                    order.push(current);
                }
            }
        }
        assert_eq!(order.len(), self.waits.len());
        order
    }
}

fn instant(node: usize, point: Point) -> usize {
    node * 3
        + match point {
            Point::Start => 0,
            Point::Finish => 1,
            Point::Entry => 2,
        }
}

/// Where requirements on a node wait: a container's entry, any other node's start.
fn anchor(tree: &crate::graph::Tree, key: &NodeKey) -> Point {
    if tree.is_container(key) {
        Point::Entry
    } else {
        Point::Start
    }
}

/// Reports each dependency cycle once, naming its nodes.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let network = Network::build(check);
    assert_eq!(network.waits.len(), check.document.nodes.len() * 3);
    for members in network.cycles() {
        let Some(first) = members.first() else {
            continue;
        };
        let mut found = super::at_node(
            check.tree,
            first,
            ViolationCode::DependencyCycle,
            "these nodes wait on each other through explicit and containment edges",
        );
        found.related = members
            .iter()
            .map(|key| Subject::Node((*key).clone()))
            .collect();
        out.push(found);
    }
}
