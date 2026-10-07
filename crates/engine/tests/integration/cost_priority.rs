//! Passes 6 and 7 at the limits (PRACTICES, Explicit limits; Back-of-the-envelope first; the
//! validation ladder, rung 3: cost tests budgeted in operations, not wall-clock time). On
//! `generated::date_limits` (`node_count_max` nodes at the depth limit) with every weight at
//! `weight_max`: the gravity sweep is one word operation per word of a row for each pruned
//! gate edge, and leverage's simulations together read each instant, each pruned edge, and
//! each kept-work entry at most once, and once more for each overdue, gated `auto_reach`
//! milestone; the largest gravity and leverage fit a score; the
//! schema's `Derived` keeps at most the response limit of explanation entries per list.

#[cfg(test)]
mod cost {
    use cairn_engine::derive::EdgeSet;
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::limits::{
        CONTAINMENT_DEPTH_MAX, EXPLANATION_ENTRY_COUNT_MAX, NODE_COUNT_MAX, WEIGHT_MAX,
    };
    use cairn_schema::{
        Date, Deployment, NodeKey, NodeState, Payload, Provenance, Relevance, Score, Weight,
    };

    /// The limits graph with every node at the weight limit, and with every node in its
    /// initial state when `open`, so most of the graph is ranked and has something to unblock.
    fn heaviest(open: bool) -> Graph {
        let mut document = date_limits().into_document();
        let keys: Vec<NodeKey> = document.nodes.as_map().keys().cloned().collect();
        for key in keys {
            let mut node = document.nodes.get(&key).unwrap().clone();
            node.weight = Some(Weight::try_from(WEIGHT_MAX).unwrap());
            if open {
                let initial = NodeState::initial(node.kind(), Provenance::Local);
                document.state.nodes.insert(key.clone(), initial);
                document.state.answers.remove(&key);
            }
            document.nodes.put(node).unwrap();
        }
        Graph::new(document, &Deployment::default()).unwrap()
    }

    #[test]
    fn priority_stays_linear_at_the_limits() {
        // Read after the pinned milestones' date too, when the gated `auto_reach` ones are due.
        for (open, today) in [
            (false, "2026-10-06"),
            (true, "2026-10-06"),
            (true, "2026-12-01"),
        ] {
            within_budget(&heaviest(open), today.parse().unwrap());
        }
    }

    /// The ranked `auto_reach` milestones due by today whose dependencies are not satisfied:
    /// each simulation of one repeats the part of a cascade behind it.
    fn overdue_gated_milestones(graph: &Graph, derived: &Derived) -> u64 {
        let count = graph
            .document()
            .nodes
            .values()
            .filter(|node| matches!(&node.payload, Payload::Milestone(milestone) if milestone.auto_reach))
            .filter(|node| derived.priority().in_normalization_set(&node.key))
            .filter(|node| derived.relevance().value(&node.key) == Relevance::Relevant)
            .filter(|node| !derived.blocking().deps_done(&node.key))
            .filter(|node| {
                let due = derived.dates().effective_date(&node.key);
                due.is_some_and(|due| due.date <= derived.today())
            })
            .count();
        u64::try_from(count).unwrap()
    }

    fn within_budget(graph: &Graph, today: Date) {
        let mut inputs = derive_inputs(Deployment::default());
        inputs.today = today;
        let derived = derive(graph, None, &inputs);
        let dependencies = derived.dependencies();
        let nodes = u64::try_from(dependencies.node_count()).unwrap();
        let instants = nodes * 4;
        let edges = u64::try_from(dependencies.edges(EdgeSet::Pruned).count()).unwrap();
        let words = nodes.div_ceil(64);
        let kept = nodes * u64::from(CONTAINMENT_DEPTH_MAX);
        let overdue = overdue_gated_milestones(graph, &derived);
        let budget = words * edges + (1 + overdue) * (instants + edges + kept);
        let operations = derived.priority().operation_count();
        let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
        let gravity_max = keys.iter().map(|key| derived.priority().gravity(key)).max();
        let leverage_max = keys
            .iter()
            .map(|key| derived.priority().leverage(key))
            .max();
        let largest = |score: Option<Score>| score.unwrap_or_default().value();
        println!(
            "priority at the limits on {today}: {instants} instants, {edges} pruned edges, \
             {overdue} overdue gated milestones; {operations} operations of a budget of \
             {budget}; largest gravity {}, leverage {}",
            largest(gravity_max),
            largest(leverage_max)
        );
        assert_eq!(nodes, u64::from(NODE_COUNT_MAX), "the node limit");
        assert!(operations <= budget, "{operations}");
        let ceiling = Score::from_millionths(nodes * u64::from(WEIGHT_MAX) * 1_000_000);
        assert!(gravity_max.unwrap() <= ceiling);
        assert!(gravity_max.unwrap() > Score::from_millionths(u64::from(WEIGHT_MAX) * 1_000_000));
        assert!(ceiling < Score::MAX, "the sums fit a score");
        let projected = derived.to_schema(graph);
        let limit = usize::try_from(EXPLANATION_ENTRY_COUNT_MAX).unwrap();
        let kept: usize = projected
            .nodes
            .values()
            .map(|node| node.gravity_from.entries.len() + node.leverage_from.entries.len())
            .sum();
        println!("projected at the limits: {kept} gravity and leverage entries kept");
        assert!(kept <= dependencies.node_count() * 2 * limit);
    }
}
