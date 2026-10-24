//! Property tests for the projections (ARCHITECTURE, Engine > Projections). Over
//! generated journeys: a level's visible set is exactly the nodes within the container whose
//! kind and relevance class are shown, whose top-level ancestor's kind the top-level rule
//! keeps, that the list's filters match or that hold a match, and that no collapsed container
//! above them hides, except that the selected node and the containers above it within the
//! level are always visible; showing a class left out draws exactly the count the level gives for it
//! and hides nothing; each
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
        AnswerSpec, Cursor, DateLinkKind, DateRuleKind, DateSource, Deployment, Event,
        FrontierEntry, Level, LevelDisplay, LevelQuery, ListFlag, ListQuery, NextQuery, NodeKey,
        NodeKind, PatchId, Payload, SnapshotScope, State, Title, Trace,
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

    /// The list's flags a five-bit mask filters by.
    fn flags(mask: u8) -> BTreeSet<ListFlag> {
        [
            ListFlag::Unassigned,
            ListFlag::NextUp,
            ListFlag::Blocked,
            ListFlag::Active,
            ListFlag::DecisionsNeeded,
        ]
        .into_iter()
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, flag)| flag)
        .collect()
    }

    /// The filters of a level request, and the nodes the list matches for them.
    struct Filters {
        flags: BTreeSet<ListFlag>,
        text: Option<Title>,
        /// The list's matches, every page; none when nothing filters.
        matches: Option<BTreeSet<NodeKey>>,
    }

    impl Filters {
        fn of(
            journey: &DerivedJourney<'_>,
            flags: BTreeSet<ListFlag>,
            text: Option<Title>,
        ) -> Self {
            let matches = (!flags.is_empty() || text.is_some()).then(|| {
                let mut query = ListQuery {
                    flags: flags.clone(),
                    text: text.clone(),
                    ..ListQuery::default()
                };
                let mut keys = BTreeSet::new();
                loop {
                    let page = journey.list(&query, &BTreeSet::new()).unwrap();
                    keys.extend(page.rows.into_iter().map(|row| row.key));
                    match page.next {
                        Some(cursor) => query.cursor = cursor,
                        None => break keys,
                    }
                }
            });
            Self {
                flags,
                text,
                matches,
            }
        }

        /// Kept by its definition: no filter, or the node or one beneath it matches.
        fn keeps(&self, graph: &Graph, key: &NodeKey) -> bool {
            self.matches.as_ref().is_none_or(|matches| {
                matches
                    .iter()
                    .any(|found| found == key || graph.tree().is_ancestor(key, found))
            })
        }
    }

    /// What a level request asks for, with each node's relevance class shown or not.
    struct Rules {
        shown: BTreeSet<NodeKind>,
        container: Option<NodeKey>,
        collapsed: BTreeSet<NodeKey>,
        class_shown: BTreeMap<NodeKey, bool>,
        top: BTreeSet<NodeKind>,
        filters: Filters,
        selected: Option<NodeKey>,
    }

    impl Rules {
        fn query(&self, display: &BTreeSet<LevelDisplay>) -> LevelQuery {
            LevelQuery {
                shown: self.shown.clone(),
                container: self.container.clone(),
                collapsed: self.collapsed.clone(),
                display: display.clone(),
                top: self.top.clone(),
                flags: self.filters.flags.clone(),
                owner: None,
                text: self.filters.text.clone(),
                selected: self.selected.clone(),
            }
        }

        /// Within the container drilled into, by its definition.
        fn within(&self, graph: &Graph, key: &NodeKey) -> bool {
            self.container
                .as_ref()
                .is_none_or(|top| graph.tree().is_ancestor(top, key))
        }

        /// The selected node, by its definition, when it is within the level.
        fn selection(&self, graph: &Graph) -> Option<&NodeKey> {
            self.selected
                .as_ref()
                .filter(|selected| self.within(graph, selected))
        }

        /// Drawn because selected: the selected node, or a container above it within the level.
        fn on_chain(&self, graph: &Graph, key: &NodeKey) -> bool {
            self.selection(graph).is_some_and(|selected| {
                self.within(graph, key)
                    && (key == selected || graph.tree().is_ancestor(key, selected))
            })
        }

        /// A container above the selected node: opened, whatever collapsed it.
        fn opened(&self, graph: &Graph, key: &NodeKey) -> bool {
            self.on_chain(graph, key) && self.selection(graph) != Some(key)
        }

        /// Left out by the top-level rule, by its definition: its root's kind is not kept.
        fn left_out(&self, graph: &Graph, key: &NodeKey) -> bool {
            let tree = graph.tree();
            let mut root = key;
            while let Some(parent) = tree.parent(root) {
                root = parent;
            }
            !self.top.is_empty() && !self.top.contains(&graph.node(root).unwrap().kind())
        }

        /// A node is visible by its definition: selected or above the selected node within the
        /// level, or else within the container, its kind and class shown, not left out at the
        /// top, kept by the filters, and no collapsed container (one above the selected node
        /// is open) between it and the container above it.
        fn visible(&self, graph: &Graph, key: &NodeKey) -> bool {
            if self.on_chain(graph, key) {
                return true;
            }
            let tree = graph.tree();
            let mut above = tree.parent(key);
            while let Some(node) = above.filter(|node| self.within(graph, node)) {
                if self.collapsed.contains(node) && !self.opened(graph, node) {
                    return false;
                }
                above = tree.parent(node);
            }
            self.shown.contains(&graph.node(key).unwrap().kind())
                && self.class_shown[key]
                && !self.left_out(graph, key)
                && self.filters.keeps(graph, key)
        }

        /// A node's stand-in by its definition: none when the top-level rule leaves it out
        /// (unless it is drawn because selected), else walking up from it within the container,
        /// the first visible node.
        fn stand_in(&self, graph: &Graph, key: &NodeKey) -> Option<NodeKey> {
            if !self.on_chain(graph, key) && self.left_out(graph, key) {
                return None;
            }
            let tree = graph.tree();
            let mut current = Some(key);
            while let Some(node) = current.filter(|node| self.within(graph, node)) {
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
        date_links_hold(graph, derived, level, rules);
        let dependencies = derived.dependencies();
        for edge in dependencies.edges(EdgeSet::Full) {
            let canvas = match edge.source {
                EdgeSource::Explicit | EdgeSource::Condition => true,
                EdgeSource::StageOpening => edge.class == EdgeClass::Gate,
                EdgeSource::Work
                | EdgeSource::Entry
                | EdgeSource::Chain
                | EdgeSource::Containment => false,
            };
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

    /// Every due-by and not-before rule with both ends in scope and on different cards, one of
    /// them the selected card, is among the level's date links, and every rule link the level
    /// holds is such a rule. A level with no selection holds none.
    fn date_links_hold(graph: &Graph, derived: &Derived, level: &Level, rules: &Rules) {
        let in_scope = |key: &NodeKey| derived.relevance().in_scope(key);
        let card = rules
            .selection(graph)
            .and_then(|selected| rules.stand_in(graph, selected));
        let mut expected: BTreeSet<(NodeKey, NodeKey, DateRuleKind)> = BTreeSet::new();
        for node in graph.document().nodes.values() {
            for (rule, found) in [
                (DateRuleKind::DueBy, &node.due_by),
                (DateRuleKind::NotBefore, &node.not_before),
            ] {
                for source in found.iter().flat_map(|found| found.sources.as_set()) {
                    let DateSource::Node(source) = source else {
                        continue;
                    };
                    let kind = graph.node(source).map(cairn_schema::Node::kind);
                    let dated = match kind {
                        Some(NodeKind::Milestone) => true,
                        Some(NodeKind::Decision) => matches!(
                            graph.node(source).map(|found| &found.payload),
                            Some(Payload::Decision(decision))
                                if matches!(decision.answer, AnswerSpec::Date { .. })
                        ),
                        _ => false,
                    };
                    let ends = (
                        rules.stand_in(graph, &node.key),
                        rules.stand_in(graph, source),
                    );
                    if dated
                        && in_scope(&node.key)
                        && in_scope(source)
                        && matches!(&ends, (Some(a), Some(b)) if a != b
                            && (card.as_ref() == Some(a) || card.as_ref() == Some(b)))
                    {
                        expected.insert((node.key.clone(), source.clone(), rule));
                    }
                }
            }
        }
        let mut found: BTreeSet<(NodeKey, NodeKey, DateRuleKind)> = BTreeSet::new();
        for link in &level.date_links {
            assert_ne!(link.from, link.to, "no date link onto one card");
            assert_ne!(link.links, [] as [cairn_schema::DateLink; 0]);
            for stands_for in &link.links {
                let ends = [
                    rules.stand_in(graph, &stands_for.node),
                    rules.stand_in(graph, &stands_for.source),
                ];
                assert!(
                    ends.contains(&Some(link.from.clone()))
                        && ends.contains(&Some(link.to.clone())),
                    "a date link joins the stand-ins of its relationships' ends"
                );
                if let DateLinkKind::Rule { rule, .. } = stands_for.kind {
                    found.insert((stands_for.node.clone(), stands_for.source.clone(), rule));
                }
            }
        }
        assert_eq!(found, expected, "the rule links are the rules");
    }

    fn trace_holds(graph: &Graph, derived: &Derived, traces: &BTreeMap<NodeKey, Trace>) {
        let dependencies = derived.dependencies();
        // A container that skips what is beneath it (skipped, or a group marked done) waits only on
        // its kept work, so the containment it ignores is left out of the checks of direct
        // requirements and ancestors.
        let skipping = |key: &NodeKey| {
            let stored = graph.document().state.nodes.get(key).map(|node| node.state);
            let group = graph
                .node(key)
                .is_some_and(|node| node.kind() == NodeKind::Group);
            stored == Some(State::Skipped)
                || (group && stored == Some(State::Done))
                || derived.skips().skipped_by(key).is_some()
        };
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
                if direct.class == EdgeClass::Gate
                    && direct.node != *key
                    && !(skipping(key) && graph.tree().parent(&direct.node) == Some(key))
                {
                    assert!(
                        trace.upstream.contains(&direct.node),
                        "{} of {key}",
                        direct.node
                    );
                }
            }
            let mut ancestor = graph.tree().parent(key);
            let mut held = true;
            while let Some(above) = ancestor {
                held &= !skipping(above);
                assert!(
                    trace.downstream.contains(above) || !held,
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
        let first = journey
            .snapshot(
                &scope,
                &cairn_store::build::journey_header("j_plain", "Plain", None),
            )
            .unwrap();
        loop {
            let page = journey
                .snapshot(
                    &scope,
                    &cairn_store::build::journey_header("j_plain", "Plain", None),
                )
                .unwrap();
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
        let entries: Vec<_> = next[..first.acting_frontier.len()]
            .iter()
            .map(FrontierEntry::of)
            .collect();
        assert_eq!(first.acting_frontier, entries);
    }

    /// The vendor evaluation's events, the pool generated histories draw from.
    fn pool() -> Vec<Event> {
        let (_, applied) = crate::engine::support::run("vendor-evaluation");
        applied
            .iter()
            .flat_map(|step| step.events().iter().cloned())
            .collect()
    }

    /// C1: showing each class the level leaves out draws exactly the count it gives for that
    /// class, and hides nothing it drew.
    fn class_counts_hold(journey: &DerivedJourney<'_>, level: &Level, rules: &Rules) {
        if LevelDisplay::is_every(&level.display) {
            assert!(level.hidden_by_class.is_empty(), "nothing hidden by class");
        }
        let drawn: BTreeSet<&NodeKey> = level.nodes.iter().map(|node| &node.key).collect();
        for class in LevelDisplay::ALL
            .into_iter()
            .filter(|class| !level.display.contains(class))
        {
            let mut display = level.display.clone();
            display.insert(class);
            let wider = journey
                .level(
                    &rules.query(&display),
                    &Deployment::default(),
                    &BTreeSet::new(),
                )
                .unwrap();
            let shown: BTreeSet<&NodeKey> = wider.nodes.iter().map(|node| &node.key).collect();
            assert!(drawn.is_subset(&shown), "showing {class:?} hides nothing");
            let added = u32::try_from(shown.len() - drawn.len()).unwrap();
            assert_eq!(
                level.hidden_by_class.get(&class).copied().unwrap_or(0),
                added,
                "{class:?} count"
            );
        }
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
            top_mask in prop::option::of(1_u8..32),
            flag_mask in 0_u8..32,
            search in prop::option::of((0_usize..80, 0_usize..3)),
            select in prop::option::of(0_usize..80),
        ) {
            let derived = derived(&graph);
            let journey = DerivedJourney::new(&graph, &derived);
            let keys: Vec<&NodeKey> = graph.document().nodes.as_map().keys().collect();
            let (display, class_shown) = displays(&graph, class_mask);
            // A search for a few letters of a node's title, so it matches something.
            let text = search.map(|(at, start)| {
                let title = graph.node(keys[at % keys.len()]).unwrap().title.as_str().to_owned();
                let from = start.min(title.len().saturating_sub(1));
                let end = (from + 3).min(title.len());
                title.get(from..end).filter(|part| !part.trim().is_empty()).unwrap_or("a").parse::<Title>().unwrap()
            });
            let rules = Rules {
                shown: kinds(mask),
                container: drill.map(|at| keys[at % keys.len()].clone()),
                collapsed: collapse.iter().map(|at| keys[at % keys.len()].clone()).collect(),
                class_shown,
                top: top_mask.map(kinds).unwrap_or_default(),
                filters: Filters::of(&journey, flags(flag_mask), text),
                selected: select.map(|at| keys[at % keys.len()].clone()),
            };
            let level = journey.level(&rules.query(&display), &Deployment::default(), &BTreeSet::new()).unwrap();
            level_holds(&graph, &derived, &level, &rules);
            class_counts_hold(&journey, &level, &rules);
            let mut wider_rules = Rules { shown: rules.shown.clone(), ..rules };
            wider_rules.shown.insert(NodeKind::ALL[extra]);
            let wider = journey.level(&wider_rules.query(&display), &Deployment::default(), &BTreeSet::new()).unwrap();
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
