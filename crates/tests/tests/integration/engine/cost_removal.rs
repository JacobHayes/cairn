//! Node removals at the limits (PRACTICES, Explicit limits; the validation ladder:
//! cost tests budgeted in operations, not wall-clock time; A18). One patch removes every
//! node of `generated::date_limits` (`node_count_max` nodes at the depth limit, each with
//! its edges), deepest first, one removal per node, alone and with a retitle of the last
//! node between each two removals. The removals index the graph once, other writes fold into
//! the index, and each removal then pays for its own subtree, edges, and notes; indexing per
//! removal would cost `node_count_max` graph passes.

#[cfg(test)]
mod cost {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use cairn_engine::testing::generated::date_limits;
    use cairn_engine::testing::{fixed_inputs, journey_id, removal_operation_count};
    use cairn_engine::{Records, apply};
    use cairn_schema::limits::NODE_COUNT_MAX;
    use cairn_schema::{
        Edge, Graph, Journey, JourneyHeader, JourneyStatus, Mutation, Mutations, NodeKey,
        ParticipationRef, Patch, PatchTarget, Removal, Revision,
    };

    /// Operations the removals may spend per node, edge, and note of the graph: the index
    /// built once, then each node's children, edges, and notes visited a few times.
    const OPERATIONS_PER_ITEM_MAX: u64 = 4;

    fn parse<T: std::str::FromStr>(text: &str) -> T
    where
        T::Err: std::fmt::Debug,
    {
        text.parse().unwrap()
    }

    fn records(graph: Graph) -> Records {
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
        let id = journey_id();
        records.journeys.insert(
            id.clone(),
            Journey {
                header: JourneyHeader {
                    id,
                    name: parse("Limits"),
                    description: None,
                    status: JourneyStatus::Active,
                    lineage: None,
                    created_at: parse("2026-09-20T00:00:00Z"),
                    created_on: parse("2026-09-20"),
                },
                revision: Revision::NONE.next(),
                graph,
            },
        );
        records
    }

    fn depth(graph: &Graph, key: &NodeKey) -> usize {
        let mut depth = 0;
        let mut current = key;
        while let Some(parent) = graph
            .nodes
            .get(current)
            .and_then(|node| node.parent.as_ref())
        {
            depth += 1;
            current = parent;
        }
        depth
    }

    /// A removal of `key` alone, as a leaf: its edges, resources, notes, and participations.
    /// Its descendants go first, so naming none is right; edges already gone with them are
    /// names that no longer apply.
    fn leaf_removal(graph: &Graph, key: &NodeKey) -> Mutation {
        let node = graph.nodes.get(key).unwrap();
        let mut edges: Vec<Edge> = node
            .requires
            .iter()
            .map(|requires| Edge {
                node: key.clone(),
                requires: requires.clone(),
            })
            .collect();
        edges.extend(
            graph
                .nodes
                .values()
                .filter(|dependent| dependent.requires.as_set().contains(key))
                .map(|dependent| Edge {
                    node: dependent.key.clone(),
                    requires: key.clone(),
                }),
        );
        let annotations = graph
            .state
            .annotations
            .values()
            .filter(|note| note.body.node.as_ref() == Some(key))
            .map(|note| note.body.key.clone());
        Mutation::RemoveNode {
            removal: Removal {
                node: key.clone(),
                descendants: std::collections::BTreeSet::new(),
                edges: edges.into_iter().collect(),
                resources: node
                    .resources
                    .iter()
                    .map(|found| found.key.clone())
                    .collect(),
                annotations: annotations.collect(),
                participations: node
                    .participations
                    .as_map()
                    .keys()
                    .map(|kind| ParticipationRef {
                        node: key.clone(),
                        kind: kind.clone(),
                    })
                    .collect(),
            },
        }
    }

    fn retitle(key: &NodeKey, step: usize) -> Mutation {
        cairn_schema::from_yaml(&format!(
            "op: set_node_field\nnode: {key}\nvalue: {{title: Retitled {step}}}\n"
        ))
        .unwrap()
    }

    #[test]
    fn removing_every_node_at_the_limits_indexes_the_graph_once() {
        let graph = date_limits().into_document();
        assert_eq!(graph.nodes.len(), NODE_COUNT_MAX as usize);
        let mut by_depth: BTreeMap<std::cmp::Reverse<usize>, Vec<&NodeKey>> = BTreeMap::new();
        for key in graph.nodes.as_map().keys() {
            by_depth
                .entry(std::cmp::Reverse(depth(&graph, key)))
                .or_default()
                .push(key);
        }
        let order: Vec<&NodeKey> = by_depth.values().flatten().copied().collect();
        let removals: Vec<Mutation> = order.iter().map(|key| leaf_removal(&graph, key)).collect();
        // The same removals with the last node to go retitled between each two.
        let last = *order.last().unwrap();
        let mut interleaved = Vec::new();
        for (step, removal) in removals.iter().enumerate() {
            if step > 0 {
                interleaved.push(retitle(last, step));
            }
            interleaved.push(removal.clone());
        }
        let edges: usize = graph.nodes.values().map(|node| node.requires.len()).sum();
        let items = graph.nodes.len() + edges + graph.state.annotations.len();
        let items = u64::try_from(items).unwrap();
        let records = records(graph);
        for (name, mutations) in [("alone", removals), ("interleaved", interleaved)] {
            let patch = Patch {
                id: parse("p_remove_all"),
                target: PatchTarget::Journey(journey_id()),
                base_revision: Revision::NONE.next(),
                deployment_revision: None,
                mutations: Mutations::new(mutations).unwrap(),
            };
            let operations = removal_operation_count(&records, &patch);
            let started = Instant::now();
            let applied = apply(&records, &patch, &fixed_inputs());
            println!(
                "removals at the limits, {name}: {} mutations over {items} nodes, edges, and notes spent {operations} operations; applied in {} ms",
                patch.mutations.len(),
                started.elapsed().as_millis()
            );
            let applied = applied.unwrap_or_else(|rejection| panic!("{name}: {rejection:#?}"));
            assert!(
                applied.records().journeys[&journey_id()]
                    .graph
                    .nodes
                    .is_empty()
            );
            assert!(
                operations <= OPERATIONS_PER_ITEM_MAX * items,
                "{name}: {operations} operations for {items} items"
            );
        }
    }
}
