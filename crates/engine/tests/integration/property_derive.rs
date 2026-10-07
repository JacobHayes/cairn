//! Property tests (rung 3) for the derive passes (PRACTICES, Testing > Engine; Gating, D1a,
//! D6): over generated journeys with deep containment, conditions, stage openings, stored
//! states, force includes, and keeps.

#[cfg(test)]
mod property {
    use std::collections::BTreeSet;

    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::{
        Deployment, EntityKey, KindKey, NodeKey, ParticipationOrigin, ParticipationSource,
        Relevance, RoleKey, State,
    };
    use patina_dst_proptest::prelude::*;

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    fn state(graph: &Graph, key: &NodeKey) -> State {
        graph.document().state.nodes[key].state
    }

    fn has_override(graph: &Graph, key: &NodeKey, keep: bool) -> bool {
        graph
            .document()
            .state
            .overrides
            .get(key)
            .is_some_and(|found| {
                if keep {
                    found.keep.is_some()
                } else {
                    found.force_include.is_some()
                }
            })
    }

    /// The graph with every open decision skipped: every decision terminal.
    fn every_decision_terminal(graph: &Graph) -> Graph {
        let mut document = graph.document().clone();
        for stored in document.state.nodes.values_mut() {
            if stored.state == State::Open {
                stored.state = State::Skipped;
                stored.skip_reason = Some("Generated.".parse().unwrap());
            }
        }
        Graph::new(document, &Deployment::default()).unwrap()
    }

    /// D1a by its definition: a non-terminal, in-scope node with a skipped proper ancestor
    /// and no keep on the way down from it, the nearest such ancestor named.
    fn naive_skipped_by(graph: &Graph, derived: &Derived, key: &NodeKey) -> Option<NodeKey> {
        if state(graph, key).is_terminal() || !derived.relevance().in_scope(key) {
            return None;
        }
        let mut below = key.clone();
        while let Some(parent) = graph.tree().parent(&below).cloned() {
            if has_override(graph, &below, true) {
                return None;
            }
            if state(graph, &parent) == State::Skipped {
                return Some(parent);
            }
            below = parent;
        }
        None
    }

    /// D1a kept work by its definition: every kept, in-scope node beneath a skipped
    /// container.
    fn naive_kept_work(graph: &Graph, derived: &Derived, container: &NodeKey) -> BTreeSet<NodeKey> {
        let skipped = state(graph, container) == State::Skipped
            || derived.skips().skipped_by(container).is_some();
        if !skipped {
            return BTreeSet::new();
        }
        let tree = graph.tree();
        tree.descendants(container)
            .into_iter()
            .filter(|key| has_override(graph, key, true) && derived.relevance().in_scope(key))
            .collect()
    }

    /// What a node's nearest declaration of `kind` says, from `from` up, else the default
    /// owner: the declaring node and its source.
    fn nearest(
        graph: &Graph,
        from: Option<&NodeKey>,
        kind: &KindKey,
    ) -> Option<(Option<NodeKey>, ParticipationSource<cairn_schema::KeyRefs>)> {
        let mut current = from.cloned();
        while let Some(at) = current {
            if let Some(source) = graph.node(&at).unwrap().participations.as_map().get(kind) {
                return Some((Some(at), source.clone()));
            }
            current = graph.tree().parent(&at).cloned();
        }
        let role = graph
            .document()
            .default_owner
            .clone()
            .filter(|_| *kind == KindKey::owner())?;
        Some((None, ParticipationSource::Role(role)))
    }

    fn members(graph: &Graph, role: &RoleKey) -> BTreeSet<EntityKey> {
        graph
            .document()
            .state
            .role_fills
            .get(role)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// E2 by its definition, walking up from the node.
    fn naive_participation(
        graph: &Graph,
        key: &NodeKey,
        kind: &KindKey,
    ) -> (Option<ParticipationOrigin>, BTreeSet<EntityKey>) {
        let Some((at, source)) = nearest(graph, Some(key), kind) else {
            return (None, BTreeSet::new());
        };
        let entities = match &source {
            ParticipationSource::Entities(set) => set.iter().cloned().collect(),
            ParticipationSource::Role(role) => members(graph, role),
        };
        let origin = match (at, source) {
            (None, ParticipationSource::Role(role)) => ParticipationOrigin::DefaultOwner(role),
            (Some(at), _) if at != *key => ParticipationOrigin::Ancestor(at),
            (_, ParticipationSource::Entities(_)) => ParticipationOrigin::Explicit,
            (_, ParticipationSource::Role(role)) => ParticipationOrigin::Role(role),
        };
        (Some(origin), entities)
    }

    /// B10 by its definition: one explicit entity, not a member of the role the node would
    /// otherwise inherit.
    fn naive_lost(graph: &Graph, key: &NodeKey, kind: &KindKey) -> bool {
        let own = graph
            .node(key)
            .unwrap()
            .participations
            .as_map()
            .get(kind)
            .cloned();
        let Some(ParticipationSource::Entities(entities)) = own else {
            return false;
        };
        let inherited = nearest(graph, graph.tree().parent(key), kind);
        let Some((_, ParticipationSource::Role(role))) = inherited else {
            return false;
        };
        entities.len() == 1
            && !entities
                .iter()
                .all(|entity| members(graph, &role).contains(entity))
    }

    proptest! {
        #[test]
        fn relevance_is_decided_once_every_decision_is_terminal(graph in arb_journey(1..=120)) {
            let terminal = every_decision_terminal(&graph);
            let derived = derived(&terminal);
            for (key, found) in derived.relevance().iter() {
                prop_assert_ne!(found.value, Relevance::Undecided, "{}", key);
            }
        }

        #[test]
        fn relevance_follows_force_include_and_dominance(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let relevance = derived.relevance();
            for (key, found) in relevance.iter() {
                if has_override(&graph, key, false) {
                    prop_assert_eq!(found.value, Relevance::Relevant, "{} is forced", key);
                    continue;
                }
                if let Some(parent) = graph.tree().parent(key) {
                    let rank = |value: Relevance| match value {
                        Relevance::Relevant => 0,
                        Relevance::Undecided => 1,
                        Relevance::NotRelevant => 2,
                    };
                    prop_assert!(rank(found.value) >= rank(relevance.value(parent)), "{}", key);
                }
            }
        }

        #[test]
        fn derive_is_a_function_of_its_inputs(graph in arb_journey(1..=120)) {
            let first = derived(&graph);
            let second = derived(&graph);
            prop_assert_eq!(format!("{first:?}"), format!("{second:?}"));
            prop_assert_eq!(first, second);
        }

        #[test]
        fn effective_skip_and_kept_work_match_their_definitions(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            for key in graph.document().nodes.as_map().keys() {
                prop_assert_eq!(
                    derived.skips().skipped_by(key).cloned(),
                    naive_skipped_by(&graph, &derived, key),
                    "{}", key
                );
                prop_assert_eq!(
                    derived.skips().kept_work(key).clone(),
                    naive_kept_work(&graph, &derived, key),
                    "{}", key
                );
            }
        }

        #[test]
        fn participation_matches_its_definition(graph in arb_journey(1..=120)) {
            let derived = derived(&graph);
            let participation = derived.participation();
            let kinds = [KindKey::owner(), "k_many".parse().unwrap()];
            for key in graph.document().nodes.as_map().keys() {
                for kind in &kinds {
                    let (origin, entities) = naive_participation(&graph, key, kind);
                    prop_assert_eq!(participation.origin(key, kind), origin.as_ref(), "{} {}", key, kind);
                    prop_assert_eq!(participation.entities(key, kind), &entities, "{} {}", key, kind);
                    prop_assert_eq!(participation.membership_lost(key).contains(kind), naive_lost(&graph, key, kind), "{} {}", key, kind);
                }
                let owned = naive_participation(&graph, key, &KindKey::owner()).1;
                prop_assert_eq!(participation.is_unassigned(key), owned.is_empty(), "{}", key);
            }
        }
    }
}
