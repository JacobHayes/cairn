//! The projections at the limits (PRACTICES, Explicit limits; Back-of-the-envelope first; the
//! validation ladder). On `generated::date_limits` (`node_count_max` nodes at the depth
//! limit): every projection's output stays within its bound (a level within the nodes and the
//! canvas edges, each edge drawn once; a page within `page_item_count_max`; an explanation page
//! within `explanation_entry_count_max`). Each module in `project` states its cost in operations.

#[cfg(test)]
mod cost {
    use std::collections::BTreeSet;

    use cairn_engine::derive::EdgeSet;
    use cairn_engine::derive::dependencies::EdgeSource;
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::{Derived, DerivedJourney, derive};
    use cairn_schema::limits::{EXPLANATION_ENTRY_COUNT_MAX, NODE_COUNT_MAX, PAGE_ITEM_COUNT_MAX};
    use cairn_schema::{
        Cursor, Deployment, ExplainedField, LevelQuery, ListFlag, ListQuery, NextQuery, NodeKey,
        NodeKind, SnapshotScope,
    };

    /// The level, by kinds and filtered, within the nodes and the canvas edges.
    fn levels_stay_within_their_bounds(
        journey: &DerivedJourney<'_>,
        derived: &Derived,
        nodes: usize,
    ) {
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
            let level = journey
                .level(
                    &LevelQuery::of_kinds(shown, None),
                    &Deployment::default(),
                    &BTreeSet::new(),
                )
                .unwrap();
            assert!(level.nodes.len() <= nodes);
            let underlying: usize = level.edges.iter().map(|edge| edge.underlying.len()).sum();
            assert!(level.edges.len() <= underlying && underlying <= canvas);
            assert!(level.date_links.is_empty(), "no selection, no date links");
        }
        let filtered = {
            let query = LevelQuery {
                flags: [ListFlag::Unassigned, ListFlag::Blocked].into(),
                text: "e".parse().ok(),
                top: [NodeKind::Group, NodeKind::Milestone].into(),
                ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
            };
            journey
                .level(&query, &Deployment::default(), &BTreeSet::new())
                .unwrap()
        };
        assert!(filtered.nodes.len() <= nodes);
        // The selected node of a filtered level: a second chain walk, at the deepest node.
        let tree = journey.graph().tree();
        let depth =
            |key: &NodeKey| std::iter::successors(Some(key), |key| tree.parent(key)).count();
        let deepest = journey
            .graph()
            .document()
            .nodes
            .as_map()
            .keys()
            .max_by_key(|key| depth(key))
            .unwrap()
            .clone();
        let selected = {
            let query = LevelQuery {
                flags: [ListFlag::Unassigned, ListFlag::Blocked].into(),
                text: "e".parse().ok(),
                top: [NodeKind::Group, NodeKind::Milestone].into(),
                selected: Some(deepest.clone()),
                ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
            };
            journey
                .level(&query, &Deployment::default(), &BTreeSet::new())
                .unwrap()
        };
        assert!(selected.nodes.iter().any(|node| node.key == deepest));
        date_links_stay_around_the_selected_card(journey, &deepest, nodes);
    }

    /// The date links of a selected card: at most one per card, each touching it.
    fn date_links_stay_around_the_selected_card(
        journey: &DerivedJourney<'_>,
        selected: &NodeKey,
        nodes: usize,
    ) {
        let around = journey
            .level(
                &LevelQuery {
                    selected: Some(selected.clone()),
                    ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
                },
                &Deployment::default(),
                &BTreeSet::new(),
            )
            .unwrap();
        assert!(around.date_links.len() <= nodes);
        assert!(
            around
                .date_links
                .iter()
                .all(|link| &link.from == selected || &link.to == selected)
        );
    }

    #[test]
    fn every_projection_stays_within_its_bounds_at_the_limits() {
        let graph = date_limits();
        let derived = derive(&graph, None, &derive_inputs(Deployment::default()));
        let journey = DerivedJourney::new(&graph, &derived);
        let nodes = graph.document().nodes.len();
        assert_eq!(nodes, NODE_COUNT_MAX as usize);
        levels_stay_within_their_bounds(&journey, &derived, nodes);
        let page = PAGE_ITEM_COUNT_MAX as usize;
        let snapshot = journey
            .snapshot(
                &SnapshotScope::default(),
                &cairn_store::build::journey_header("j_plain", "Plain", None),
            )
            .unwrap();
        assert!(snapshot.nodes.len() <= page && snapshot.acting_frontier.len() <= page);
        let list = journey
            .list(&ListQuery::default(), &BTreeSet::new())
            .unwrap();
        assert!(list.rows.len() <= page);
        assert_eq!(list.total as usize, nodes);
        let next = journey
            .next(&NextQuery::default(), &BTreeSet::new())
            .unwrap();
        assert_eq!(next.items.len(), derived.ranking().acting_frontier().len());
        let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
        let heaviest = keys
            .iter()
            .max_by_key(|key| derived.priority().gravity(key))
            .unwrap();
        journey.trace(heaviest).unwrap();
        let explained = journey
            .explanations(heaviest, ExplainedField::Gravity, Cursor::START)
            .unwrap();
        assert!(explained.entries.len() <= EXPLANATION_ENTRY_COUNT_MAX as usize);
        assert!(journey.decision_view().decisions.len() <= nodes);
        let timeline = journey.timeline();
        assert!(timeline.entries.len() <= nodes);
        assert!(journey.status_summary().remaining as usize <= nodes);
    }
}
