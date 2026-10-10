//! The projections at the limits (PRACTICES, Explicit limits; Back-of-the-envelope first; the
//! validation ladder). On `generated::date_limits` (`node_count_max` nodes at the depth
//! limit): every projection's output stays within its bound (a level within the nodes and the
//! canvas edges, each edge drawn once; a page within `page_item_count_max`; an explanation page
//! within `explanation_entry_count_max`), and the elapsed time of each is printed for the
//! proof, reported rather than gated (ARCHITECTURE, Date network: the benchmark is reported).
//! Each module in `project` states its cost in operations.

#[cfg(test)]
mod cost {
    use std::collections::BTreeSet;
    use std::time::Instant;

    use cairn_engine::derive::EdgeSet;
    use cairn_engine::derive::dependencies::EdgeSource;
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::{DerivedJourney, derive};
    use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, NODE_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
    use cairn_schema::{
        Cursor, Deployment, ExplainedField, LevelQuery, ListQuery, NextQuery, NodeKey, NodeKind,
        SnapshotScope,
    };

    fn timed<T>(name: &str, run: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let value = run();
        println!(
            "projection at the limits: {name} in {} ms",
            started.elapsed().as_millis()
        );
        value
    }

    #[test]
    fn every_projection_stays_within_its_bounds_at_the_limits() {
        let graph = date_limits();
        let derived = derive(&graph, None, &derive_inputs(Deployment::default()));
        let journey = DerivedJourney::new(&graph, &derived);
        let nodes = graph.document().nodes.len();
        assert_eq!(nodes, NODE_COUNT_MAX as usize);
        let canvas = derived
            .dependencies()
            .edges(EdgeSet::Full)
            .filter(|edge| {
                matches!(
                    edge.source,
                    EdgeSource::Explicit | EdgeSource::Condition | EdgeSource::StageOpening
                )
            })
            .count();
        for hidden in [None, Some(NodeKind::Group), Some(NodeKind::Action)] {
            let shown: BTreeSet<NodeKind> = NodeKind::ALL
                .into_iter()
                .filter(|kind| Some(*kind) != hidden)
                .collect();
            let level = timed(&format!("level hiding {hidden:?}"), || {
                journey
                    .level(&LevelQuery::of_kinds(shown, None), &Deployment::default())
                    .unwrap()
            });
            assert!(level.nodes.len() <= nodes);
            let underlying: usize = level.edges.iter().map(|edge| edge.underlying.len()).sum();
            assert!(level.edges.len() <= underlying && underlying <= canvas);
        }
        let page = PAGE_ITEM_COUNT_MAX as usize;
        let snapshot = timed("snapshot", || {
            journey.snapshot(&SnapshotScope::default()).unwrap()
        });
        assert!(snapshot.nodes.len() <= page && snapshot.acting_frontier.len() <= page);
        let list = timed("list", || {
            journey
                .list(&ListQuery::default(), &BTreeSet::new())
                .unwrap()
        });
        assert!(list.rows.len() <= page);
        assert_eq!(list.total as usize, nodes);
        let next = timed("next", || {
            journey
                .next(&NextQuery::default(), &BTreeSet::new())
                .unwrap()
        });
        assert_eq!(next.items.len(), derived.ranking().acting_frontier().len());
        let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
        let heaviest = keys
            .iter()
            .max_by_key(|key| derived.priority().gravity(key))
            .unwrap();
        timed("trace of the heaviest node", || {
            journey.trace(heaviest).unwrap()
        });
        let explained = timed("explanations", || {
            journey
                .explanations(heaviest, ExplainedField::Gravity, Cursor::START)
                .unwrap()
        });
        assert!(explained.entries.len() <= EXPLANATION_ENTRY_COUNT_MAX as usize);
        timed("decision view", || journey.decision_view());
        let timeline = timed("timeline", || journey.timeline());
        assert!(timeline.entries.len() <= nodes);
        timed("status summary", || journey.status_summary());
    }
}
