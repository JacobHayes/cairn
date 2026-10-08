//! Upgrade at the limits (PRACTICES, Explicit limits; Back-of-the-envelope first; rung 3). A
//! journey of `node_count_max` nodes (`generated::date_limits`, with its state) follows a
//! route of the same structure; the target retitles every node. Drafting the upgrade (the
//! merge, the orphan removals, and the trial apply that validates and derives the candidate
//! once) and applying it are timed and printed for the proof, reported rather than gated.

#[cfg(test)]
mod cost {
    use std::time::Instant;

    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::{Records, apply, upgrade};
    use cairn_schema::limits::NODE_COUNT_MAX;
    use cairn_schema::{
        Journey, JourneyHeader, JourneyState, Lineage, Mutation, Mutations, Patch, PatchTarget,
        Revision, Route, RouteHeader, RouteVersion, VersionNumber,
    };

    fn parse<T: std::str::FromStr>(text: &str) -> T
    where
        T::Err: std::fmt::Debug,
    {
        text.parse().unwrap()
    }

    fn version(number: VersionNumber, graph: cairn_schema::Graph) -> (Lineage, RouteVersion) {
        let lineage = Lineage {
            route: parse("limits"),
            version: number,
        };
        let version = RouteVersion {
            route: lineage.route.clone(),
            version: number,
            published_at: parse("2026-09-01T00:00:00Z"),
            graph,
        };
        (lineage, version)
    }

    /// Every node retitled.
    fn retitled(route: &cairn_schema::Graph) -> cairn_schema::Graph {
        let mut target = route.clone();
        let nodes: Vec<_> = target.nodes.values().cloned().collect();
        for mut node in nodes {
            node.title = parse(&format!("{} again", node.title.as_str()));
            target.nodes.put(node).unwrap();
        }
        target
    }

    /// Only the roots kept, without their edges: every other node becomes an orphan.
    fn roots_only(route: &cairn_schema::Graph) -> cairn_schema::Graph {
        let mut target = route.clone();
        let nodes: Vec<_> = target.nodes.values().cloned().collect();
        for mut node in nodes {
            if node.parent.is_some() {
                target.nodes.remove(&node.key);
            } else {
                node.requires = cairn_schema::BoundedSet::default();
                target.nodes.put(node).unwrap();
            }
        }
        target
    }

    fn records(change: fn(&cairn_schema::Graph) -> cairn_schema::Graph) -> Records {
        let journey = date_limits().into_document();
        let mut route = journey.clone();
        route.state = JourneyState::default();
        let target = change(&route);
        let (first, second) = (VersionNumber::FIRST, VersionNumber::FIRST.next());
        let mut records = Records::default();
        // The entities the generated journey's fills and participations name.
        for key in ["e_a", "e_b", "e_c"] {
            let entity = cairn_schema::Entity {
                key: parse(key),
                name: parse(key),
                emails: std::collections::BTreeSet::new(),
            };
            records.deployment.entities.put(entity).unwrap();
        }
        records.routes.insert(
            parse("limits"),
            Route {
                header: RouteHeader {
                    id: parse("limits"),
                    name: parse("Limits"),
                    description: None,
                    retired: false,
                },
                revision: Revision::NONE.next(),
                versions: [first, second].into(),
                draft: None,
            },
        );
        records
            .versions
            .extend([version(first, route), version(second, target)]);
        let id = cairn_engine::testing::journey_id();
        records.journeys.insert(
            id.clone(),
            Journey {
                header: JourneyHeader {
                    id,
                    name: parse("Limits"),
                    description: None,
                    status: cairn_schema::JourneyStatus::Active,
                    lineage: Some(version(first, cairn_schema::Graph::default()).0),
                    created_at: parse("2026-09-20T00:00:00Z"),
                    created_on: parse("2026-09-20"),
                },
                revision: Revision::NONE.next(),
                graph: journey,
            },
        );
        records
    }

    #[test]
    fn an_upgrade_at_the_limits() {
        let records = records(retitled);
        let inputs = cairn_engine::testing::fixed_inputs();
        let id = cairn_engine::testing::journey_id();
        let patch = Patch {
            id: parse("p_upgrade"),
            target: PatchTarget::Journey(id.clone()),
            base_revision: records.journeys[&id].revision,
            deployment_revision: None,
            mutations: Mutations::new(vec![Mutation::Upgrade {
                to: VersionNumber::FIRST.next(),
            }])
            .unwrap(),
        };
        // For scale: the same journey's apply of a rename, which validates and derives the
        // same candidate without merging.
        let rename = Patch {
            mutations: Mutations::new(vec![
                cairn_schema::from_yaml("op: edit_journey\nname: Renamed\n").unwrap(),
            ])
            .unwrap(),
            ..patch.clone()
        };
        // The draft, the apply, and the rename each start from the same records, so they run
        // side by side: the test takes the longest of them, not their sum.
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let started = Instant::now();
                let draft = upgrade(&records, &id, VersionNumber::FIRST.next(), &inputs).unwrap();
                println!(
                    "upgrade at the limits: drafted in {} ms with {} items",
                    started.elapsed().as_millis(),
                    draft.items.len()
                );
                assert!(
                    draft.items.is_empty(),
                    "every retitle applies cleanly: {:#?}",
                    draft.items.as_slice().first()
                );
            });
            scope.spawn(|| {
                let started = Instant::now();
                apply(&records, &rename, &inputs).unwrap();
                println!(
                    "upgrade at the limits: a rename of the same journey applied in {} ms",
                    started.elapsed().as_millis()
                );
            });
            let started = Instant::now();
            let applied = apply(&records, &patch, &inputs).unwrap();
            println!(
                "upgrade at the limits: applied in {} ms with {} writes",
                started.elapsed().as_millis(),
                applied.events()[0].delta.len()
            );
            let graph = &applied.records().journeys[&id].graph;
            assert_eq!(graph.nodes.len(), NODE_COUNT_MAX as usize);
            assert!(
                graph
                    .nodes
                    .values()
                    .all(|node| node.title.as_str().ends_with(" again"))
            );
        });
    }

    /// The target keeps only the roots: drafting computes every orphan's removal in one pass.
    #[test]
    fn an_orphan_heavy_upgrade_at_the_limits() {
        let records = records(roots_only);
        let inputs = cairn_engine::testing::fixed_inputs();
        let id = cairn_engine::testing::journey_id();
        let started = Instant::now();
        let draft = upgrade(&records, &id, VersionNumber::FIRST.next(), &inputs).unwrap();
        let orphans = draft
            .items
            .as_slice()
            .iter()
            .filter(|item| matches!(item, cairn_schema::ReviewItem::Orphan { .. }))
            .count();
        println!(
            "upgrade at the limits: drafted with {orphans} orphans in {} ms",
            started.elapsed().as_millis()
        );
        let roots = records.journeys[&id]
            .graph
            .nodes
            .values()
            .filter(|node| node.parent.is_none())
            .count();
        assert_eq!(orphans, NODE_COUNT_MAX as usize - roots);
    }
}
