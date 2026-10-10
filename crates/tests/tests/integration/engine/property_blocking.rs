//! Property tests for pass 5 and the derived guards (PRD Gating, Containment, D2, D4,
//! D5, B6; A17). Over generated journeys: the flags agree with each other and with the lists
//! read off the entry chains; open work always leaves something on the frontier, so a stalled
//! journey is held only by snoozes and `auto_reach` dates and never shows as all blocked.
//! Over generated patches: a node a patch completes is never stale on the graph it produced.

#[cfg(test)]
mod property {
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::testing::{arb_graph, arb_ops, derive_inputs, fixed_inputs, journey_id};
    use cairn_engine::{Derived, Graph, Records, apply, derive};
    use cairn_schema::{Deployment, NodeKind, Relevance, StallCause, State};
    use patina_dst_proptest::prelude::*;

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    proptest! {
        /// D2, Gating: actionable is relevant, open, not a group, and `deps_done`; blocked is
        /// in scope, open, and not `deps_done`, with something listed that blocks it; the
        /// frontier is the actionable nodes and the acting frontier the unsnoozed ones.
        #[test]
        fn the_flags_and_frontiers_agree(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let blocking = derived.blocking();
            let mut actionable = Vec::new();
            for (key, node) in graph.document().nodes.as_map() {
                let open = derived.relevance().in_scope(key) && !blocking.closed(key);
                prop_assert_eq!(blocking.blocked(key), open && !blocking.deps_done(key), "{}", key);
                let expected = open && blocking.deps_done(key) && node.kind() != NodeKind::Group
                    && derived.relevance().value(key) == Relevance::Relevant;
                prop_assert_eq!(blocking.actionable(key), expected, "{}", key);
                prop_assert_eq!(derived.open_dependencies(key).is_empty(), blocking.deps_done(key), "{}", key);
                if blocking.blocked(key) {
                    let through = derived.blocked_through(&graph, key);
                    prop_assert!(!derived.blocked_by(key).is_empty() || !through.is_empty(), "{}", key);
                }
                if blocking.actionable(key) {
                    actionable.push(key.clone());
                }
            }
            prop_assert_eq!(blocking.frontier(), actionable.as_slice());
            for key in blocking.acting_frontier() {
                prop_assert!(blocking.frontier().contains(key));
                prop_assert!(blocking.snoozed(key).is_none(), "{}", key);
            }
        }

        /// D5: following open dependencies always ends on the frontier, so open in-scope work
        /// leaves the frontier non-empty; a stalled journey names a hold for every frontier
        /// node (its own, or the container's snooze it is held through, B6) and is never all
        /// blocked.
        #[test]
        fn open_work_always_reaches_the_frontier(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let blocking = derived.blocking();
            let open_work = graph.document().nodes.as_map().iter().any(|(key, node)| {
                node.kind() != NodeKind::Group && derived.relevance().in_scope(key) && !blocking.closed(key)
            });
            prop_assert_eq!(open_work, !blocking.frontier().is_empty());
            if let Some(stalled) = blocking.stalled() {
                prop_assert!(blocking.acting_frontier().is_empty());
                prop_assert!(!stalled.all_blocked);
                let named = |key: &_| stalled.waiting_on.iter().any(|cause| matches!(
                    cause,
                    StallCause::Snooze { node, .. } | StallCause::AutoReach { node, .. } if node == key
                ));
                for key in blocking.frontier() {
                    prop_assert!(
                        named(key) || blocking.snoozed_via(key).is_some_and(named),
                        "{} is held but unnamed", key
                    );
                }
            } else {
                prop_assert!(!blocking.acting_frontier().is_empty() || !open_work);
            }
        }

        /// D4, A17: guards are checked on the graph a patch produces and a bypass records what
        /// it accepted, so a node the patch completes is never stale right after it.
        #[test]
        fn a_completion_is_never_stale_on_the_graph_it_produced(
            initial in arb_graph(),
            ops in prop::collection::vec(arb_ops(), 1..48),
        ) {
            let mut records = initial;
            for op in ops {
                let patch = op.resolve(&records);
                let Ok(applied) = apply(&records, &patch, &fixed_inputs()) else {
                    continue;
                };
                let after = applied.records().clone();
                for key in completed(&records, &after) {
                    let graph = graph_of(&after);
                    let derived = derive(&graph, None, &derive_inputs(after.deployment.clone()));
                    prop_assert!(!derived.is_stale(&key), "{} {:?}", key, derived.stale(&graph, &key));
                }
                records = after;
            }
        }
    }

    fn graph_of(records: &Records) -> Graph {
        let document = records.journeys[&journey_id()].graph.clone();
        Graph::new(document, &records.deployment).unwrap()
    }

    /// Nodes that a patch moved into `done`, `decided`, or `reached`.
    fn completed(before: &Records, after: &Records) -> Vec<cairn_schema::NodeKey> {
        let state_in = |records: &Records, key| {
            records
                .journeys
                .get(&journey_id())
                .and_then(|journey| journey.graph.state.nodes.get(key))
                .map(|stored| stored.state)
        };
        let Some(journey) = after.journeys.get(&journey_id()) else {
            return Vec::new();
        };
        journey
            .graph
            .state
            .nodes
            .iter()
            .filter(|(_, stored)| {
                matches!(stored.state, State::Done | State::Decided | State::Reached)
            })
            .filter(|(key, stored)| state_in(before, *key) != Some(stored.state))
            .map(|(key, _)| key.clone())
            .collect()
    }
}
