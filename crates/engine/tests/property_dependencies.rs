//! Property tests (rung 3) for the effective dependency graph (PRD Containment, Gating;
//! Invariants; ARCHITECTURE, Read path: the reference test). Over generated deep trees, the
//! entry chain agrees with the plain expansion that copies every inherited requirement and
//! condition onto each descendant, both in what each node directly depends on and in what it
//! transitively reaches, for gate edges and for all edges, in the full and pruned sets.

#[cfg(test)]
mod property {
    use std::collections::BTreeSet;

    use cairn_engine::derive::dependencies::{EdgeClass, Instant, Point};
    use cairn_engine::derive::{Dependencies, EdgeSet};
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::{DependencyVia, Deployment, NodeKey, Payload};
    use patina_dst_proptest::prelude::*;

    type Dependency = (NodeKey, EdgeClass);

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    fn forced(graph: &Graph, key: &NodeKey) -> bool {
        let overrides = graph.document().state.overrides.get(key);
        overrides.is_some_and(|found| found.force_include.is_some())
    }

    fn opening(graph: &Graph, key: &NodeKey) -> Option<Dependency> {
        match &graph.node(key)?.payload {
            Payload::Group(group) => {
                let class = if group.gates {
                    EdgeClass::Gate
                } else {
                    EdgeClass::DateOnly
                };
                group.opens_at.clone().map(|milestone| (milestone, class))
            }
            _ => None,
        }
    }

    fn reads(graph: &Graph, key: &NodeKey) -> BTreeSet<NodeKey> {
        let condition = graph.node(key).and_then(|node| node.relevant_when.as_ref());
        condition
            .into_iter()
            .flat_map(|condition| condition.decisions().into_iter().cloned())
            .collect()
    }

    /// The plain expansion: a node depends on its children, its own requirements, opening,
    /// and condition decisions, and copies of every ancestor's. In the pruned set, force
    /// include stops conditions (the node's own and its ancestors') and not-relevant nodes
    /// depend on nothing and are depended on by nothing.
    fn plain(
        graph: &Graph,
        derived: &Derived,
        key: &NodeKey,
        set: EdgeSet,
    ) -> BTreeSet<Dependency> {
        let pruned = set == EdgeSet::Pruned;
        let in_scope = |other: &NodeKey| !pruned || derived.relevance().in_scope(other);
        if !in_scope(key) {
            return BTreeSet::new();
        }
        let tree = graph.tree();
        let mut found: BTreeSet<Dependency> = tree
            .children(key)
            .iter()
            .map(|child| (child.clone(), EdgeClass::Gate))
            .collect();
        let mut conditions_apply = true;
        let mut current = Some(key.clone());
        while let Some(at) = current {
            conditions_apply &= !(pruned && forced(graph, &at));
            let node = graph.node(&at).unwrap();
            found.extend(node.requires.iter().map(|r| (r.clone(), EdgeClass::Gate)));
            found.extend(opening(graph, &at));
            if conditions_apply {
                found.extend(reads(graph, &at).into_iter().map(|d| (d, EdgeClass::Gate)));
            }
            current = tree.parent(&at).cloned();
        }
        found.retain(|(other, _)| in_scope(other));
        found
    }

    fn listed(graph: &Dependencies, key: &NodeKey, set: EdgeSet) -> BTreeSet<Dependency> {
        let entries = graph.of(key, set).into_iter();
        entries.map(|entry| (entry.node, entry.class)).collect()
    }

    fn closure_plain(
        graph: &Graph,
        derived: &Derived,
        key: &NodeKey,
        set: EdgeSet,
        gates_only: bool,
    ) -> BTreeSet<NodeKey> {
        let mut found = BTreeSet::new();
        let mut stack = vec![key.clone()];
        while let Some(at) = stack.pop() {
            for (other, class) in plain(graph, derived, &at, set) {
                if (!gates_only || class == EdgeClass::Gate) && found.insert(other.clone()) {
                    stack.push(other);
                }
            }
        }
        found
    }

    fn closure_chain(
        graph: &Dependencies,
        key: &NodeKey,
        set: EdgeSet,
        gates_only: bool,
    ) -> BTreeSet<NodeKey> {
        let from = Instant::new(graph.node_index(key).unwrap(), Point::Finish);
        let mut seen = BTreeSet::from([from]);
        let mut stack = vec![from];
        let mut found = BTreeSet::new();
        while let Some(instant) = stack.pop() {
            for edge in graph.waits(instant, set) {
                if gates_only && edge.class != EdgeClass::Gate {
                    continue;
                }
                if edge.requirement.point == Point::Finish {
                    found.insert(graph.key(edge.requirement.node).unwrap().clone());
                }
                if seen.insert(edge.requirement) {
                    stack.push(edge.requirement);
                }
            }
        }
        found
    }

    /// Kahn's algorithm over the full set's gate edges: true when every instant is ordered.
    fn gates_acyclic(graph: &Dependencies) -> bool {
        let slots = graph.node_count() * 4;
        let mut waiting = vec![0_usize; slots];
        let gates: Vec<_> = graph
            .edges(EdgeSet::Full)
            .filter(|edge| edge.class == EdgeClass::Gate)
            .collect();
        for edge in &gates {
            waiting[edge.dependent.slot()] += 1;
        }
        let mut ready: Vec<usize> = (0..slots).filter(|&slot| waiting[slot] == 0).collect();
        let mut ordered = 0;
        while let Some(slot) = ready.pop() {
            ordered += 1;
            for edge in gates.iter().filter(|edge| edge.requirement.slot() == slot) {
                waiting[edge.dependent.slot()] -= 1;
                if waiting[edge.dependent.slot()] == 0 {
                    ready.push(edge.dependent.slot());
                }
            }
        }
        ordered == slots
    }

    /// The node a listed source names holds the reference it claims.
    fn source_holds(graph: &Graph, key: &NodeKey, node: &NodeKey, via: &DependencyVia) -> bool {
        let tree = graph.tree();
        let self_or_ancestor = |owner: &NodeKey| owner == key || tree.is_ancestor(owner, key);
        let requires = |owner: &NodeKey| {
            graph
                .node(owner)
                .unwrap()
                .requires
                .iter()
                .any(|r| r == node)
        };
        match via {
            DependencyVia::Explicit => requires(key),
            DependencyVia::Containment => tree.parent(node) == Some(key),
            DependencyVia::Inherited { ancestor } => {
                tree.is_ancestor(ancestor, key) && requires(ancestor)
            }
            DependencyVia::Condition { condition_on } => {
                self_or_ancestor(condition_on) && reads(graph, condition_on).contains(node)
            }
            DependencyVia::StageOpening { group } => {
                self_or_ancestor(group) && opening(graph, group).is_some_and(|(m, _)| m == *node)
            }
        }
    }

    proptest! {
        #[test]
        fn entry_chain_matches_the_plain_expansion(graph in arb_journey(1..=80)) {
            let derived = derived(&graph);
            let dependencies = derived.dependencies();
            for key in graph.document().nodes.as_map().keys() {
                for set in [EdgeSet::Full, EdgeSet::Pruned] {
                    prop_assert_eq!(listed(dependencies, key, set), plain(&graph, &derived, key, set), "{} {:?}", key, set);
                    for gates_only in [true, false] {
                        prop_assert_eq!(
                            closure_chain(dependencies, key, set, gates_only),
                            closure_plain(&graph, &derived, key, set, gates_only),
                            "{} {:?} gates only: {}", key, set, gates_only
                        );
                    }
                }
            }
        }

        #[test]
        fn every_dependency_names_its_source(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            for key in graph.document().nodes.as_map().keys() {
                for entry in derived.dependencies().of(key, EdgeSet::Full) {
                    prop_assert!(source_holds(&graph, key, &entry.node, &entry.via), "{} {:?}", key, entry);
                }
            }
        }

        #[test]
        fn the_pruned_set_is_a_subgraph_of_the_full_one(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let dependencies = derived.dependencies();
            let full: BTreeSet<_> = dependencies.edges(EdgeSet::Full).collect();
            prop_assert!(dependencies.edges(EdgeSet::Pruned).all(|edge| full.contains(edge)));
        }

        #[test]
        fn accepted_graphs_have_no_gate_cycle_and_linear_edges(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let dependencies = derived.dependencies();
            prop_assert!(gates_acyclic(dependencies));
            let document = graph.document();
            let explicit: usize = document.nodes.values().map(|node| node.requires.len()).sum();
            let referenced: usize = document.nodes.as_map().keys().map(|key| reads(&graph, key).len() + usize::from(opening(&graph, key).is_some())).sum();
            let edges = dependencies.edges(EdgeSet::Full).count();
            prop_assert!(edges <= 6 * document.nodes.len() + explicit + referenced, "{} edges", edges);
        }
    }
}
