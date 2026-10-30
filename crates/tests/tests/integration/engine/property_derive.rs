//! Property tests for the derive passes (PRACTICES, Testing; Gating, D1a,
//! D6): over generated journeys with deep containment, conditions, stage openings, stored
//! states, force includes, and keeps.

#[cfg(test)]
mod property {
    use std::collections::BTreeSet;

    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::{arb_journey, plan_held};
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::{
        Deployment, IdentityKey, KindKey, NodeKey, NodeKind, ParticipationOrigin,
        ParticipationSource, Relevance, RoleKey, State, ViolationCode,
    };
    use patina_dst_proptest::prelude::*;

    use crate::engine::support;

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

    /// The graph with every open decision skipped: every decision terminal. Skipping takes
    /// nodes out of scope and brings others in, so the plan is held again over the new scope.
    fn every_decision_terminal(graph: &Graph) -> Graph {
        let mut document = graph.document().clone();
        for stored in document.state.nodes.values_mut() {
            if stored.state == State::Open {
                stored.state = State::Skipped;
                stored.reason = Some("Generated.".parse().unwrap());
            }
        }
        plan_held(document)
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
            if state(graph, &parent).skips_beneath(graph.node(&parent).unwrap().kind()) {
                return Some(parent);
            }
            below = parent;
        }
        None
    }

    /// D1a by its definition: the items (not groups) a skip of `container` would cover: the
    /// non-terminal, in-scope ones beneath it with no keep on the way up and no nearer container
    /// that skips what is beneath it.
    fn naive_covered(graph: &Graph, derived: &Derived, container: &NodeKey) -> u32 {
        let covers = |item: &NodeKey| {
            let mut below = item.clone();
            while let Some(parent) = graph.tree().parent(&below).cloned() {
                if has_override(graph, &below, true) {
                    return false;
                }
                if parent == *container {
                    return true;
                }
                if state(graph, &parent).skips_beneath(graph.node(&parent).unwrap().kind()) {
                    return false;
                }
                below = parent;
            }
            false
        };
        let count = graph
            .tree()
            .descendants(container)
            .into_iter()
            .filter(|item| {
                graph.node(item).unwrap().kind() != NodeKind::Group
                    && !state(graph, item).is_terminal()
                    && derived.relevance().in_scope(item)
                    && covers(item)
            })
            .count();
        u32::try_from(count).unwrap()
    }

    /// D1a kept work by its definition: every kept, in-scope node beneath a skipped
    /// container.
    fn naive_kept_work(graph: &Graph, derived: &Derived, container: &NodeKey) -> BTreeSet<NodeKey> {
        let skipped = state(graph, container).skips_beneath(graph.node(container).unwrap().kind())
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

    fn members(graph: &Graph, role: &RoleKey) -> BTreeSet<IdentityKey> {
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
    ) -> (Option<ParticipationOrigin>, BTreeSet<IdentityKey>) {
        let Some((at, source)) = nearest(graph, Some(key), kind) else {
            return (None, BTreeSet::new());
        };
        let identities = match &source {
            ParticipationSource::Identities(set) => set.iter().cloned().collect(),
            ParticipationSource::Role(role) => members(graph, role),
        };
        let origin = match (at, source) {
            (None, ParticipationSource::Role(role)) => ParticipationOrigin::DefaultOwner(role),
            (Some(at), _) if at != *key => ParticipationOrigin::Ancestor(at),
            (_, ParticipationSource::Identities(_)) => ParticipationOrigin::Explicit,
            (_, ParticipationSource::Role(role)) => ParticipationOrigin::Role(role),
        };
        (Some(origin), identities)
    }

    /// B10 by its definition: one explicit identity, not a member of the role the node would
    /// otherwise inherit.
    fn naive_lost(graph: &Graph, key: &NodeKey, kind: &KindKey) -> bool {
        let own = graph
            .node(key)
            .unwrap()
            .participations
            .as_map()
            .get(kind)
            .cloned();
        let Some(ParticipationSource::Identities(identities)) = own else {
            return false;
        };
        let inherited = nearest(graph, graph.tree().parent(key), kind);
        let Some((_, ParticipationSource::Role(role))) = inherited else {
            return false;
        };
        identities.len() == 1
            && !identities
                .iter()
                .all(|identity| members(graph, &role).contains(identity))
    }

    /// D1a: with every decision terminal, no node's relevance is undecided.
    fn assert_decided_once_terminal(graph: &Graph) -> Result<(), TestCaseError> {
        let derived = derived(&every_decision_terminal(graph));
        for (key, found) in derived.relevance().iter() {
            prop_assert_ne!(found.value, Relevance::Undecided, "{}", key);
        }
        Ok(())
    }

    /// Skipping a decision can bring a contradictory chain into the plan: a group due before
    /// a milestone one day ahead of it, with the milestone inside the group beneath a node
    /// that waits for the answer to a second decision. That answer counts only once the
    /// second decision is itself relevant, which it is when the first is skipped and not
    /// while the first is open. The plan holds with the first open; skipped, the chain
    /// contradicts, and the terminal graph is built with that chain's dates dropped.
    #[test]
    fn a_skipped_decision_can_bring_a_contradictory_chain_into_scope() {
        let mut mutations = support::add_nodes(&[
            "{key: n_first, id: first, kind: decision, title: First, prompt: First?, answer_type: boolean}",
            "{key: n_second, id: second, kind: decision, title: Second, prompt: Second?, answer_type: boolean}",
            "{key: n_group, id: group, kind: group, title: Group, due_by: {before: n_ms, offset: 1}}",
            "{key: n_inner, id: inner, kind: group, title: Inner, parent: n_group, relevant_when: {answered: n_second}}",
            "{key: n_ms, id: ms, kind: milestone, title: Ms, parent: n_inner}",
        ]);
        mutations.push_str("- op: answer\n  decision: n_second\n  value: {boolean: true}\n");
        mutations.push_str("- op: set_node_field\n  node: n_second\n  value: {relevant_when: {not: {equals: {decision: n_first, value: true}}}}\n");
        let records = support::journey(&mutations);
        let graph = support::journey_graph(&records, support::JOURNEY);
        let mut document = graph.document().clone();
        for stored in document.state.nodes.values_mut() {
            if stored.state == State::Open {
                stored.state = State::Skipped;
                stored.reason = Some("Generated.".parse().unwrap());
            }
        }
        let Err(violations) = Graph::new(document, &Deployment::default()) else {
            panic!("skipping the decision should bring the chain into the plan");
        };
        assert!(
            violations
                .as_slice()
                .iter()
                .any(|found| found.code == ViolationCode::ContradictoryChain),
            "{violations:#?}"
        );
        assert_decided_once_terminal(&graph).unwrap();
    }

    proptest! {
        #[test]
        fn relevance_is_decided_once_every_decision_is_terminal(graph in arb_journey(1..=120)) {
            assert_decided_once_terminal(&graph)?;
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
                prop_assert_eq!(
                    derived.skips().covered(key),
                    naive_covered(&graph, &derived, key),
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
                    let (origin, identities) = naive_participation(&graph, key, kind);
                    prop_assert_eq!(participation.origin(key, kind), origin.as_ref(), "{} {}", key, kind);
                    prop_assert_eq!(participation.identities(key, kind), &identities, "{} {}", key, kind);
                    prop_assert_eq!(participation.membership_lost(key).contains(kind), naive_lost(&graph, key, kind), "{} {}", key, kind);
                }
                let owned = naive_participation(&graph, key, &KindKey::owner()).1;
                prop_assert_eq!(participation.is_unassigned(key), owned.is_empty(), "{}", key);
            }
        }
    }
}
