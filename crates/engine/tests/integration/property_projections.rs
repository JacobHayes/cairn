//! Property tests (rung 3) for the projections (ARCHITECTURE, Engine > Projections). Over
//! generated journeys: a level's visible set is exactly the nodes within the container whose
//! kind and relevance class are shown and that no collapsed container above them hides, each
//! node sits under its nearest visible ancestor, every drawn edge stands for at least one
//! underlying edge between its ends' stand-ins and every edge with two distinct stand-ins is
//! drawn, no edge is drawn twice or onto one node, the marker only sits on blocked nodes (or
//! cards holding one), and
//! showing more kinds never hides a node that was visible; a trace's sets are consistent with
//! the graph: upstream and downstream mirror each other, are closed under the relation, and
//! hold every direct dependency and every ancestor; the snapshot's pages never exceed their
//! limit and together list exactly the in-scope nodes within its depth, its counts add up, and
//! its frontier is the next list's; history pages are contiguous and complete.

#[cfg(test)]
mod property {
    use std::collections::{BTreeMap, BTreeSet};

    use cairn_engine::derive::EdgeSet;
    use cairn_engine::derive::dependencies::{EdgeClass, EdgeSource};
    use cairn_engine::derive::pending::{RelevanceClass, classify};
    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::{Derived, DerivedJourney, Graph, derive, history};
    use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
    use cairn_schema::{
        Cursor, Deployment, Event, Level, LevelDisplay, LevelQuery, NextQuery, NodeKey, NodeKind,
        PatchId, SnapshotScope, Trace,
    };
    use patina_dst_proptest::prelude::*;

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    /// The kinds a five-bit mask shows.
    fn kinds(mask: u8) -> BTreeSet<NodeKind> {
        NodeKind::ALL
            .into_iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, kind)| kind)
            .collect()
    }

    /// What a level request asks for, with each node's relevance class shown or not.
    struct Rules {
        shown: BTreeSet<NodeKind>,
        container: Option<NodeKey>,
        collapsed: BTreeSet<NodeKey>,
        class_shown: BTreeMap<NodeKey, bool>,
    }

    impl Rules {
        fn query(&self, display: &BTreeSet<LevelDisplay>) -> LevelQuery {
            LevelQuery {
                shown: self.shown.clone(),
                container: self.container.clone(),
                collapsed: self.collapsed.clone(),
                display: display.clone(),
            }
        }

        /// A node is visible by its definition: within the container, its kind and class
        /// shown, and no collapsed container between it and the container above it.
        fn visible(&self, graph: &Graph, key: &NodeKey) -> bool {
            let tree = graph.tree();
            let within = |key: &NodeKey| {
                self.container
                    .as_ref()
                    .is_none_or(|top| tree.is_ancestor(top, key))
            };
            let mut above = tree.parent(key);
            while let Some(node) = above.filter(|node| within(node)) {
                if self.collapsed.contains(node) {
                    return false;
                }
                above = tree.parent(node);
            }
            self.shown.contains(&graph.node(key).unwrap().kind()) && self.class_shown[key]
        }

        /// A node's stand-in by its definition: walking up from it within the container, the
        /// first visible node.
        fn stand_in(&self, graph: &Graph, key: &NodeKey) -> Option<NodeKey> {
            let tree = graph.tree();
            let mut current = Some(key);
            while let Some(node) = current.filter(|node| {
                self.container
                    .as_ref()
                    .is_none_or(|top| tree.is_ancestor(top, node))
            }) {
                if self.visible(graph, node) {
                    return Some(node.clone());
                }
                current = tree.parent(node);
            }
            None
        }
    }

    /// Which relevance classes a three-bit mask shows, and each node's class shown by it.
    fn displays(graph: &Graph, mask: u8) -> (BTreeSet<LevelDisplay>, BTreeMap<NodeKey, bool>) {
        let display: BTreeSet<LevelDisplay> = LevelDisplay::ALL
            .into_iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, class)| class)
            .collect();
        let classes = classify(graph, &Deployment::default());
        let shown = classes
            .iter()
            .map(|(key, found)| {
                let class = match found.class() {
                    RelevanceClass::Relevant => LevelDisplay::Relevant,
                    RelevanceClass::Undecided | RelevanceClass::Pending => {
                        LevelDisplay::Conditional
                    }
                    RelevanceClass::Settled => LevelDisplay::NotRelevant,
                };
                (key.clone(), display.contains(&class))
            })
            .collect();
        (display, shown)
    }

    fn level_holds(graph: &Graph, derived: &Derived, level: &Level, rules: &Rules) {
        let visible: BTreeSet<&NodeKey> = level.nodes.iter().map(|node| &node.key).collect();
        for key in graph.document().nodes.as_map().keys() {
            let expected = rules.stand_in(graph, key).as_ref() == Some(key);
            assert_eq!(visible.contains(key), expected, "{key} visible");
        }
        for node in &level.nodes {
            let parent = graph.tree().parent(&node.key);
            let expected = parent.and_then(|parent| rules.stand_in(graph, parent));
            assert_eq!(
                node.parent, expected,
                "{} is drawn under its nearest",
                node.key
            );
            if !node.hidden_prerequisites.is_empty() {
                let blocked = |key: &NodeKey| derived.blocking().blocked(key);
                assert!(blocked(&node.key) || node.rolled_up.iter().any(blocked));
            }
        }
        let mut drawn: BTreeMap<(NodeKey, NodeKey), usize> = BTreeMap::new();
        for edge in &level.edges {
            assert_ne!(edge.from, edge.to, "no edge onto one node");
            assert_ne!(edge.underlying.len(), 0);
            for underlying in &edge.underlying {
                let from = rules.stand_in(graph, &underlying.requirement);
                let to = rules.stand_in(graph, &underlying.dependent);
                assert_eq!(
                    (from.as_ref(), to.as_ref()),
                    (Some(&edge.from), Some(&edge.to))
                );
            }
            *drawn
                .entry((edge.from.clone(), edge.to.clone()))
                .or_default() += 1;
        }
        assert!(drawn.values().all(|count| *count == 1), "collapsed once");
        let dependencies = derived.dependencies();
        for edge in dependencies.edges(EdgeSet::Full) {
            let canvas = matches!(
                edge.source,
                EdgeSource::Explicit | EdgeSource::Condition | EdgeSource::StageOpening
            );
            let requirement = dependencies.key(edge.requirement.node).unwrap();
            let dependent = dependencies.key(edge.dependent.node).unwrap();
            let from = rules.stand_in(graph, requirement);
            let to = rules.stand_in(graph, dependent);
            if let (true, Some(from), Some(to)) = (canvas, from, to) {
                assert!(
                    from == to || drawn.contains_key(&(from, to)),
                    "every edge drawn"
                );
            }
        }
    }

    fn trace_holds(graph: &Graph, derived: &Derived, traces: &BTreeMap<NodeKey, Trace>) {
        let dependencies = derived.dependencies();
        for (key, trace) in traces {
            for up in &trace.upstream {
                assert!(
                    traces[up].downstream.contains(key),
                    "{up} upstream of {key}"
                );
                assert!(
                    traces[up]
                        .upstream
                        .iter()
                        .all(|further| further == key || trace.upstream.contains(further))
                );
            }
            for down in &trace.downstream {
                assert!(
                    traces[down].upstream.contains(key),
                    "{down} downstream of {key}"
                );
            }
            for direct in dependencies.of(key, EdgeSet::Full) {
                if direct.class == EdgeClass::Gate && direct.node != *key {
                    assert!(
                        trace.upstream.contains(&direct.node),
                        "{} of {key}",
                        direct.node
                    );
                }
            }
            let mut ancestor = graph.tree().parent(key);
            while let Some(above) = ancestor {
                assert!(
                    trace.downstream.contains(above),
                    "ancestor {above} of {key}"
                );
                ancestor = graph.tree().parent(above);
            }
        }
    }

    /// Every page of the snapshot from the start: each within the limit; together, the in-scope
    /// nodes within the depth in tree order; the counts add up; the frontier is the next list's.
    fn snapshot_holds(graph: &Graph, derived: &Derived, scope: &SnapshotScope) {
        let journey = DerivedJourney::new(graph, derived);
        let limit = usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap();
        let mut scope = scope.clone();
        let mut listed: Vec<NodeKey> = Vec::new();
        let first = journey.snapshot(&scope).unwrap();
        loop {
            let page = journey.snapshot(&scope).unwrap();
            assert!(page.nodes.len() <= limit && page.acting_frontier.len() <= limit);
            listed.extend(page.nodes.iter().map(|node| node.row.key.clone()));
            match page.next {
                Some(cursor) => scope.cursor = cursor,
                None => break,
            }
        }
        let tree = graph.tree();
        let top = first.scope.subtree.as_ref();
        let depth_of = |key: &NodeKey| {
            let mut depth = 0_u32;
            let mut current = Some(key);
            while let Some(node) = current.filter(|node| Some(*node) != top) {
                depth += 1;
                current = tree.parent(node);
            }
            depth
        };
        let expected: Vec<NodeKey> = graph
            .document()
            .nodes
            .as_map()
            .keys()
            .filter(|key| top.is_none_or(|top| *key == top || tree.is_ancestor(top, key)))
            .filter(|key| derived.relevance().in_scope(key))
            .filter(|key| first.scope.depth.is_none_or(|most| depth_of(key) <= most))
            .cloned()
            .collect();
        let as_set = |keys: &[NodeKey]| keys.iter().cloned().collect::<BTreeSet<NodeKey>>();
        assert_eq!(as_set(&listed), as_set(&expected));
        assert_eq!(listed.len(), expected.len(), "each node once");
        let counts = &first.counts;
        assert_eq!(counts.listed as usize, listed.len());
        assert_eq!(counts.by_state.values().sum::<u32>(), counts.in_scope);
        let shown = first.acting_frontier.len() + first.acting_frontier_rest.len();
        assert_eq!(counts.acting_frontier as usize, shown);
        let query = NextQuery {
            within: top.cloned(),
            ..NextQuery::default()
        };
        let next = journey.next(&query, &BTreeSet::new()).unwrap().items;
        assert_eq!(
            first.acting_frontier.as_slice(),
            &next[..first.acting_frontier.len()]
        );
    }

    /// The vendor evaluation's events, the pool generated histories draw from.
    fn pool() -> Vec<Event> {
        let (_, applied) = crate::support::run("vendor-evaluation");
        applied
            .iter()
            .flat_map(|step| step.events().iter().cloned())
            .collect()
    }

    proptest! {
        /// C2: a level holds its rules, and showing one more kind never hides a node.
        #[test]
        fn a_level_holds_the_roll_up_rules(
            graph in arb_journey(1..=80),
            mask in 0_u8..32,
            extra in 0_usize..5,
            drill in prop::option::of(0_usize..80),
            collapse in prop::collection::vec(0_usize..80, 0..4),
            class_mask in 1_u8..8,
        ) {
            let derived = derived(&graph);
            let journey = DerivedJourney::new(&graph, &derived);
            let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
            let (display, class_shown) = displays(&graph, class_mask);
            let rules = Rules {
                shown: kinds(mask),
                container: drill.map(|at| keys[at % keys.len()].clone()),
                collapsed: collapse.iter().map(|at| keys[at % keys.len()].clone()).collect(),
                class_shown,
            };
            let level = journey.level(&rules.query(&display), &Deployment::default()).unwrap();
            level_holds(&graph, &derived, &level, &rules);
            let mut wider_rules = Rules { shown: rules.shown.clone(), ..rules };
            wider_rules.shown.insert(NodeKind::ALL[extra]);
            let wider = journey.level(&wider_rules.query(&display), &Deployment::default()).unwrap();
            let visible: BTreeSet<&NodeKey> = wider.nodes.iter().map(|node| &node.key).collect();
            prop_assert!(level.nodes.iter().all(|node| visible.contains(&node.key)));
        }

        /// C7: traces mirror each other, are closed, and hold the direct dependencies and the
        /// ancestors.
        #[test]
        fn traces_are_consistent_with_the_graph(graph in arb_journey(1..=60)) {
            let derived = derived(&graph);
            let journey = DerivedJourney::new(&graph, &derived);
            let traces: BTreeMap<NodeKey, Trace> = graph
                .document()
                .nodes
                .as_map()
                .keys()
                .map(|key| (key.clone(), journey.trace(key).unwrap()))
                .collect();
            trace_holds(&graph, &derived, &traces);
        }

        /// I3: the snapshot's pages, scoping, counts, and frontier.
        #[test]
        fn snapshot_pages_are_bounded_complete_and_agree_with_next(
            graph in arb_journey(1..=300),
            subtree in prop::option::of(0_usize..300),
            depth in prop::option::of(0_u32..6),
        ) {
            let derived = derived(&graph);
            let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
            let scope = SnapshotScope {
                subtree: subtree.map(|at| keys[at % keys.len()].clone()),
                depth,
                cursor: Cursor::START,
            };
            snapshot_holds(&graph, &derived, &scope);
        }

        /// J4: history pages, followed from the start, are contiguous and complete: each
        /// within the limit, together every event naming the node in log order, grouped by
        /// patch with no group split within a page.
        #[test]
        fn history_pages_are_contiguous_and_complete(
            sizes in prop::collection::vec(1_usize..300, 1..6),
            pick in 0_usize..64,
            per_node in any::<bool>(),
        ) {
            let pool = pool();
            let mut events = Vec::new();
            for (patch, size) in sizes.iter().enumerate() {
                for at in 0..*size {
                    let mut event = pool[(pick + patch * 7 + at) % pool.len()].clone();
                    event.patch_id = format!("p_{patch}").parse::<PatchId>().unwrap();
                    event.ordinal = u32::try_from(at).unwrap();
                    events.push(event);
                }
            }
            let node = per_node.then(|| {
                let named: Vec<NodeKey> = pool[pick % pool.len()].nodes().into_iter().collect();
                named.first().cloned().unwrap_or_else(|| "n_access".parse().unwrap())
            });
            let expected: Vec<&Event> = events
                .iter()
                .filter(|event| node.as_ref().is_none_or(|node| event.nodes().contains(node)))
                .collect();
            let mut cursor = Cursor::START;
            let mut seen: Vec<Event> = Vec::new();
            loop {
                let page = history(&events, node.as_ref(), cursor);
                prop_assert_eq!(page.total as usize, expected.len());
                let count: usize = page.patches.iter().map(|patch| patch.events.len()).sum();
                prop_assert!(count <= PAGE_ITEM_COUNT_MAX as usize);
                for pair in page.patches.windows(2) {
                    prop_assert_ne!(&pair[0].patch_id, &pair[1].patch_id);
                }
                for patch in &page.patches {
                    prop_assert!(patch.events.iter().all(|event| event.patch_id == patch.patch_id));
                    seen.extend(patch.events.iter().cloned());
                }
                match page.next {
                    Some(next) => cursor = next,
                    None => break,
                }
            }
            let expected: Vec<Event> = expected.into_iter().cloned().collect();
            prop_assert_eq!(seen, expected);
        }
    }
}
