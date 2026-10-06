//! Cycles among gate edges (PRD Invariants: the dependency graph, including implicit gates,
//! is acyclic). The strongly connected components of the full set's gate edges, found with
//! Kosaraju's two depth-first passes, each iterative with an explicit stack (PRACTICES, No
//! recursion): O(instants + edges), about 120,000 steps at the limits. Date-only edges are
//! left out, so a date-only cycle (a `gates: false` opening whose milestone requires its
//! group) is legal here and left to the date network (F5).

use std::collections::BTreeSet;

use super::{Dependencies, Edge, EdgeClass, EdgeSet, Instant, NodeIndex, Point};

impl Dependencies {
    /// Each strongly connected component of more than one instant among the full set's gate
    /// edges, as its instants.
    #[must_use]
    pub(crate) fn gate_cycles(&self) -> Vec<BTreeSet<Instant>> {
        let order = self.finish_order();
        let unassigned = usize::MAX;
        let mut component = vec![unassigned; self.slots()];
        let mut cycles = Vec::new();
        for &root in order.iter().rev() {
            if component.get(root.slot()) != Some(&unassigned) {
                continue;
            }
            let mut members = BTreeSet::new();
            let mut stack = vec![root];
            while let Some(next) = stack.pop() {
                match component.get_mut(next.slot()) {
                    Some(slot) if *slot == unassigned => *slot = root.slot(),
                    Some(_) | None => continue,
                }
                members.insert(next);
                stack.extend(
                    self.waited_by(next, EdgeSet::Full)
                        .filter(|edge| is_gate(edge))
                        .map(|edge| edge.dependent),
                );
            }
            if members.len() > 1 {
                cycles.push(members);
            }
        }
        cycles
    }

    fn slots(&self) -> usize {
        self.keys.len() * Point::ALL.len()
    }

    fn instants(&self) -> impl Iterator<Item = Instant> {
        (0..self.keys.len()).flat_map(|node| {
            let node = NodeIndex(u32::try_from(node).unwrap_or(u32::MAX));
            Point::ALL
                .into_iter()
                .map(move |point| Instant { node, point })
        })
    }

    /// Instants in the order a depth-first search along gate waits finishes them.
    fn finish_order(&self) -> Vec<Instant> {
        let mut visited = vec![false; self.slots()];
        let mut order = Vec::with_capacity(self.slots());
        for start in self.instants() {
            if visited.get(start.slot()) == Some(&true) {
                continue;
            }
            if let Some(seen) = visited.get_mut(start.slot()) {
                *seen = true;
            }
            // (instant, its gate waits, the next one to explore)
            let mut stack: Vec<(Instant, Vec<Instant>, usize)> =
                vec![(start, self.gate_waits(start), 0)];
            while let Some((current, targets, next)) = stack.pop() {
                let Some(&target) = targets.get(next) else {
                    order.push(current);
                    continue;
                };
                stack.push((current, targets, next + 1));
                if visited.get(target.slot()) == Some(&false) {
                    if let Some(seen) = visited.get_mut(target.slot()) {
                        *seen = true;
                    }
                    stack.push((target, self.gate_waits(target), 0));
                }
            }
        }
        assert_eq!(order.len(), self.slots());
        order
    }

    fn gate_waits(&self, instant: Instant) -> Vec<Instant> {
        self.waits(instant, EdgeSet::Full)
            .filter(|edge| is_gate(edge))
            .map(|edge| edge.requirement)
            .collect()
    }
}

fn is_gate(edge: &Edge) -> bool {
    edge.class == EdgeClass::Gate
}
