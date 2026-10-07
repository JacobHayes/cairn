//! Property tests (rung 3) for passes 6 and 7 (PRD Priority; PRACTICES, Property tests). Over
//! generated journeys: gravity is a plain walk's count-once sum, never decreases upstream along
//! a dependency between in-scope nodes, and is at least an open node's own effective weight;
//! what completing a node unblocks is exactly what derive finds newly unblocked once the node
//! is done, each target once; ranks are in [0, 1] under the default constants and the
//! frontiers are in rank order; a viewer's ranking leaves the global one unchanged; the
//! schema's `Derived` agrees with the engine and reads back from JSON.

#[cfg(test)]
mod property {
    use std::collections::{BTreeMap, BTreeSet};

    use cairn_engine::derive::EdgeSet;
    use cairn_engine::derive::dependencies::{EdgeClass, Instant, Point};
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::{
        Deployment, EntityKey, NodeKey, NodeKind, Payload, Relevance, Score, State,
    };
    use patina_dst_proptest::prelude::*;

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    /// The graph with no `auto_reach` milestone: completing a node can move a milestone's
    /// effective date to today, and leverage completes no node but the one completed.
    fn without_auto_reach(graph: &Graph) -> Graph {
        let mut document = graph.document().clone();
        let keys: Vec<NodeKey> = document.nodes.as_map().keys().cloned().collect();
        for key in keys {
            let mut node = document.nodes.get(&key).unwrap().clone();
            if let Payload::Milestone(milestone) = &mut node.payload {
                milestone.auto_reach = false;
            }
            document.nodes.put(node).unwrap();
        }
        Graph::new(document, &Deployment::default()).unwrap()
    }

    /// What a node counts in millionths, from the PRD's words: its weight, halved when
    /// undecided, when it is in scope and nothing is left to do on it.
    fn counted(graph: &Graph, derived: &Derived, key: &NodeKey) -> u64 {
        let relevance = derived.relevance().value(key);
        if relevance == Relevance::NotRelevant || derived.blocking().closed(key) {
            return 0;
        }
        let weight = u64::from(graph.node(key).unwrap().effective_weight().get());
        let discount = if relevance == Relevance::Undecided {
            500
        } else {
            1_000
        };
        weight * discount * 1_000
    }

    /// The nodes with an instant reachable from the node's finish along pruned gate edges, by
    /// a plain breadth-first walk.
    fn reachable(derived: &Derived, key: &NodeKey) -> BTreeSet<NodeKey> {
        let dependencies = derived.dependencies();
        let start = Instant::new(dependencies.node_index(key).unwrap(), Point::Finish);
        let mut seen = BTreeSet::from([start]);
        let mut queue = vec![start];
        let mut nodes = BTreeSet::new();
        while let Some(instant) = queue.pop() {
            for edge in dependencies.waited_by(instant, EdgeSet::Pruned) {
                if edge.class == EdgeClass::Gate && seen.insert(edge.dependent) {
                    nodes.insert(dependencies.key(edge.dependent.node).unwrap().clone());
                    queue.push(edge.dependent);
                }
            }
        }
        nodes
    }

    /// The graph with `key` completed: done, or reached for a milestone; none for a kind that
    /// completes another way.
    fn completed(graph: &Graph, key: &NodeKey) -> Option<Graph> {
        let state = match graph.node(key)?.kind() {
            NodeKind::Action | NodeKind::Deliverable => State::Done,
            NodeKind::Milestone => State::Reached,
            NodeKind::Decision | NodeKind::Group => return None,
        };
        let mut document = graph.document().clone();
        document.state.snoozes.remove(key);
        let stored = document.state.nodes.get_mut(key)?;
        stored.state = state;
        stored.finished_on = Some("2026-10-06".parse().unwrap());
        Graph::new(document, &Deployment::default()).ok()
    }

    proptest! {
        /// Priority, Gravity: each node's gravity is its own counted weight plus that of every
        /// node a plain walk reaches from its finish, each once; it is at least an open
        /// in-scope node's own effective weight, and never less upstream along a dependency
        /// between in-scope nodes.
        #[test]
        fn gravity_is_a_count_once_sum_that_never_decreases_upstream(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let priority = derived.priority();
            for key in graph.document().nodes.as_map().keys() {
                let own = counted(&graph, &derived, key);
                let rest: u64 = reachable(&derived, key).iter().map(|other| counted(&graph, &derived, other)).sum();
                prop_assert_eq!(priority.gravity(key), Score::from_millionths(own + rest), "{}", key);
                prop_assert!(priority.gravity(key).millionths() >= own, "{}", key);
                if !derived.relevance().in_scope(key) {
                    continue;
                }
                for dependency in derived.dependencies().of(key, EdgeSet::Pruned) {
                    if dependency.class != EdgeClass::Gate || !derived.relevance().in_scope(&dependency.node) {
                        continue;
                    }
                    prop_assert!(
                        priority.gravity(&dependency.node) >= priority.gravity(key),
                        "{} upstream of {}", dependency.node, key
                    );
                }
            }
        }

        /// Priority, Leverage: what completing a node unblocks is what derive finds newly
        /// `deps_done` in the normalization set once that node is done, each target once.
        #[test]
        fn leverage_unblocks_what_completing_the_node_does(graph in arb_journey(1..=60)) {
            let graph = without_auto_reach(&graph);
            let before = self::derived(&graph);
            let derived = &before;
            let priority = derived.priority();
            let ranked: Vec<&NodeKey> = graph.document().nodes.as_map().keys()
                .filter(|key| priority.in_normalization_set(key))
                .collect();
            for key in ranked.iter().take(8) {
                let listed: Vec<NodeKey> = priority.leverage_from(key).into_iter().map(|found| found.node).collect();
                let unique: BTreeSet<NodeKey> = listed.iter().cloned().collect();
                prop_assert_eq!(unique.len(), listed.len(), "each target once");
                let Some(after) = completed(&graph, key) else {
                    continue;
                };
                let after = self::derived(&after);
                let freed: BTreeSet<NodeKey> = ranked.iter()
                    .filter(|other| **other != *key)
                    .filter(|other| !derived.blocking().deps_done(other) && after.blocking().deps_done(other))
                    .map(|other| (*other).clone())
                    .collect();
                prop_assert_eq!(&unique, &freed, "{}", key);
            }
        }

        /// Priority, Leverage, F1: with `auto_reach` milestones kept and read late enough that
        /// any dated one is due, what completing a node unblocks is still what derive finds
        /// newly `deps_done`, wherever completing it moves no effective date.
        #[test]
        fn leverage_unblocks_through_due_auto_reach_milestones(graph in arb_journey(1..=60)) {
            let mut inputs = derive_inputs(Deployment::default());
            inputs.today = "2027-06-01".parse().unwrap();
            let derived = derive(&graph, None, &inputs);
            let priority = derived.priority();
            let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
            let ranked: Vec<&NodeKey> = keys.iter().copied().filter(|key| priority.in_normalization_set(key)).collect();
            for key in ranked.iter().take(8) {
                let Some(after) = completed(&graph, key) else {
                    continue;
                };
                let after = derive(&after, None, &inputs);
                let moved = keys.iter().any(|other| derived.dates().effective_date(other) != after.dates().effective_date(other));
                if moved {
                    continue;
                }
                let listed: BTreeSet<NodeKey> = priority.leverage_from(key).into_iter().map(|found| found.node).collect();
                let freed: BTreeSet<NodeKey> = ranked.iter()
                    .filter(|other| **other != *key)
                    .filter(|other| !derived.blocking().deps_done(other) && after.blocking().deps_done(other))
                    .map(|other| (*other).clone())
                    .collect();
                prop_assert_eq!(&listed, &freed, "{}", key);
            }
        }

        /// Priority, Rank: under the default constants every rank is in [0, 1], exactly the
        /// normalization set is ranked, and both frontiers are in rank order; a viewer's
        /// ranking leaves the global one unchanged, and two derives rank alike (D6).
        #[test]
        fn ranks_are_bounded_and_order_the_frontiers(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let ranking = derived.ranking();
            for key in graph.document().nodes.as_map().keys() {
                let rank = ranking.rank(key);
                prop_assert_eq!(rank.is_some(), derived.priority().in_normalization_set(key), "{}", key);
                if let Some(rank) = rank {
                    prop_assert!((0.0..=1.0).contains(&rank), "{} {}", key, rank);
                }
            }
            for frontier in [ranking.frontier(), ranking.acting_frontier()] {
                prop_assert_eq!(frontier.to_vec(), ranking.sorted(frontier));
                for pair in frontier.windows(2) {
                    prop_assert!(ranking.rank(&pair[0]) >= ranking.rank(&pair[1]));
                }
            }
            let keys: BTreeSet<&NodeKey> = ranking.frontier().iter().collect();
            let unranked: BTreeSet<&NodeKey> = derived.blocking().frontier().iter().collect();
            prop_assert_eq!(keys, unranked);
            let owners: BTreeMap<&NodeKey, &BTreeSet<EntityKey>> = graph.document().nodes.as_map().keys()
                .map(|key| (key, derived.participation().entities(key, &cairn_schema::KindKey::owner())))
                .collect();
            let global = ranking.clone();
            for viewer in owners.values() {
                let mine = derived.rank_for(viewer);
                prop_assert_eq!(mine.frontier().len(), global.frontier().len());
            }
            prop_assert_eq!(derived.ranking(), &global);
            let again = self::derived(&graph);
            prop_assert_eq!(derived.ranking(), again.ranking());
        }

        /// D3: the schema's `Derived` agrees with the engine's values for every node, lists
        /// the frontiers in rank order, and reads back from JSON as written.
        #[test]
        fn the_projection_agrees_and_reads_back(graph in arb_journey(1..=60)) {
            let derived = derived(&graph);
            let projected = derived.to_schema(&graph);
            prop_assert_eq!(projected.frontier.as_slice(), derived.ranking().frontier());
            prop_assert_eq!(projected.acting_frontier.as_slice(), derived.ranking().acting_frontier());
            for (key, node) in &projected.nodes {
                prop_assert_eq!(node.gravity, derived.priority().gravity(key));
                prop_assert_eq!(node.leverage, derived.priority().leverage(key));
                prop_assert_eq!(node.rank.map(cairn_schema::Real::get), derived.ranking().rank(key));
                prop_assert_eq!(node.actionable, derived.blocking().actionable(key));
                prop_assert_eq!(node.relevance.value, derived.relevance().value(key));
                let sum: u64 = node.gravity_from.entries.as_slice().iter().map(|found| found.score.millionths()).sum();
                prop_assert!(sum <= node.gravity.millionths());
            }
            let json = cairn_schema::to_json(&projected).unwrap();
            prop_assert_eq!(cairn_schema::from_json::<cairn_schema::Derived>(&json).unwrap(), projected);
        }
    }
}
