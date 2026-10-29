//! Graph structure (A1, A3, A6, A7, A10, A18, B5, B10): nodes, edges, roles, kinds,
//! participations, and resources, in a journey or a route's draft.

use cairn_schema::{
    Edge, GraphKey, GraphRecord, KeyRefs, Limit, Mutation, Node, NodeFieldValue, NodeKey,
    NodeState, RetiredKey, Subject, ViolationCode, Write,
};

use super::Session;

/// Applies a structural mutation to the patch's graph.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::AddNode { node } => add_node(session, node),
        Mutation::SetNodeField { node, value } => set_field(session, node, value),
        Mutation::ReplaceNode { node } => replace_node(session, node),
        Mutation::RemoveNode { removal } => super::removal::remove(session, removal),
        Mutation::InsertRoute { .. } => super::insertion::apply(session, mutation),
        Mutation::AddEdge { edge } => change_edge(session, edge, true),
        Mutation::RemoveEdge { edge } => change_edge(session, edge, false),
        Mutation::SetParticipation { node, kind, source } => {
            if !session.require(node) {
                return Vec::new();
            }
            let full = session.node(node).is_some_and(|found| {
                let map = found.participations.as_map();
                !map.contains_key(kind) && Limit::KindCount.check(map.len() + 1).is_err()
            });
            if full {
                limit(session, node, Limit::KindCount);
                return Vec::new();
            }
            let record = GraphRecord::Participation {
                node: node.clone(),
                kind: kind.clone(),
                source: source.clone(),
            };
            vec![session.put(record)]
        }
        Mutation::ClearParticipation { node, kind } => {
            if !session.require(node) {
                return Vec::new();
            }
            let has = session
                .node(node)
                .is_some_and(|found| found.participations.as_map().contains_key(kind));
            if !has {
                session.reject(
                    ViolationCode::UnresolvedReference,
                    Some(node),
                    format!("the node has no {kind} participation"),
                );
                return Vec::new();
            }
            let key = GraphKey::Participation {
                node: node.clone(),
                kind: kind.clone(),
            };
            vec![session.remove(key)]
        }
        other => super::graph_parts::apply(session, other),
    }
}

pub(super) fn limit(session: &mut Session<'_>, node: &NodeKey, limit: Limit) {
    session.reject(
        ViolationCode::LimitExceeded,
        Some(node),
        format!("past {}", limit.name()),
    );
    if let Some(found) = session.violations.last_mut() {
        found.limit = Some(limit);
    }
}

/// A1: a new node, in its initial state in a journey (B1). A key
/// the graph used or retired is never reused (Invariants).
fn add_node(session: &mut Session<'_>, node: &Node<KeyRefs>) -> Vec<Write> {
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let problem = if graph.nodes.get(&node.key).is_some() {
        Some((
            ViolationCode::DuplicateKey,
            "a node with this key exists".to_owned(),
        ))
    } else if graph.retired_keys.nodes.contains(&node.key) {
        Some((
            ViolationCode::RetiredKeyReused,
            crate::validate::retired_key_message(&RetiredKey::Node(node.key.clone())),
        ))
    } else if Limit::NodeCount.check(graph.nodes.len() + 1).is_err() {
        Some((
            ViolationCode::LimitExceeded,
            "the graph is at node_count_max".to_owned(),
        ))
    } else {
        None
    };
    if let Some((code, message)) = problem {
        session.reject(code, Some(&node.key), message);
        if code == ViolationCode::LimitExceeded
            && let Some(found) = session.violations.last_mut()
        {
            found.limit = Some(Limit::NodeCount);
        }
        return Vec::new();
    }
    assert!(graph.nodes.get(&node.key).is_none() && !graph.retired_keys.nodes.contains(&node.key));
    let mut writes = vec![session.put(GraphRecord::Node(node.clone()))];
    if session.journey().is_some() {
        writes.push(session.put(GraphRecord::NodeState {
            node: node.key.clone(),
            state: NodeState::initial(node.kind()),
        }));
    }
    writes
}

/// B5: one field of a node; the kind must have it (A1a).
fn set_field(
    session: &mut Session<'_>,
    key: &NodeKey,
    value: &NodeFieldValue<KeyRefs>,
) -> Vec<Write> {
    if !session.require(key) {
        return Vec::new();
    }
    let field = value.field();
    // A kind has a field exactly when the field reads on it (A1a).
    let (kind, has_field) = match session.node(key) {
        Some(node) => (node.kind(), NodeFieldValue::read(field, node).is_some()),
        None => unreachable!("the node was required"),
    };
    if !has_field {
        session.reject(
            ViolationCode::FieldNotOnKind,
            Some(key),
            format!(
                "a {} has no {} field",
                kind.name(),
                cairn_schema::wire_name(&field)
            ),
        );
        if let Some(found) = session.violations.last_mut() {
            found.at.field = Some(field);
        }
        return Vec::new();
    }
    assert_eq!(value.field(), field);
    let record = GraphRecord::NodeField {
        node: key.clone(),
        value: value.clone(),
    };
    let mut writes = vec![session.put(record)];
    // G5: an item the new list drops loses its tick in the same write.
    if let NodeFieldValue::Checklist(checklist) = value {
        writes.extend(dropped_ticks(session, key, Some(checklist)));
    }
    writes
}

/// G5: what leaves with the items that are not in `checklist`: the node's ticks for them,
/// and the bypass failures recorded for them, so neither attaches to a later item that takes
/// the id.
fn dropped_ticks(
    session: &Session<'_>,
    node: &NodeKey,
    checklist: Option<&cairn_schema::Checklist>,
) -> Vec<Write> {
    let Some(journey) = session.journey() else {
        return Vec::new();
    };
    let state = &journey.graph.state;
    let mut writes: Vec<Write> = crate::edit::ticks_dropped(state, node, checklist)
        .into_iter()
        .map(|key| session.remove(key))
        .collect();
    if let Some(mut overrides) = state.overrides.get(node).cloned()
        && crate::edit::prune_bypass(&mut overrides, checklist)
    {
        writes.push(session.put(GraphRecord::Overrides {
            node: node.clone(),
            overrides,
        }));
    }
    writes
}

/// A node replaced whole, as a kind or answer-type change does. A new kind has a new
/// machine, so the node starts over in its initial state.
fn replace_node(session: &mut Session<'_>, node: &Node<KeyRefs>) -> Vec<Write> {
    if !session.require(&node.key) {
        return Vec::new();
    }
    let Some(old) = session.node(&node.key).cloned() else {
        unreachable!("the node was required")
    };
    assert_eq!(old.key, node.key);
    let mut writes = vec![session.put(GraphRecord::Node(node.clone()))];
    // G5: an item the new node does not have (every item, for a kind with no checklist)
    // loses its tick in the same write.
    writes.extend(dropped_ticks(session, &node.key, node.payload.checklist()));
    let stored = session
        .journey()
        .is_some_and(|journey| journey.graph.state.nodes.contains_key(&node.key));
    if stored && old.kind() != node.kind() {
        writes.push(session.put(GraphRecord::NodeState {
            node: node.key.clone(),
            state: NodeState::initial(node.kind()),
        }));
        let answered = session
            .journey()
            .is_some_and(|journey| journey.graph.state.answers.contains_key(&node.key));
        if answered {
            writes.push(session.remove(GraphKey::Answer(node.key.clone())));
        }
    }
    writes
}

/// A3: an explicit edge, stored on its dependent.
fn change_edge(session: &mut Session<'_>, edge: &Edge, add: bool) -> Vec<Write> {
    if !session.require(&edge.node) {
        return Vec::new();
    }
    let (present, count) = match session.node(&edge.node) {
        Some(dependent) => (
            dependent.requires.as_set().contains(&edge.requires),
            dependent.requires.len(),
        ),
        None => unreachable!("the node was required"),
    };
    if add && !present && Limit::EdgeCountPerNode.check(count + 1).is_err() {
        limit(session, &edge.node, Limit::EdgeCountPerNode);
        return Vec::new();
    }
    if !add && !present {
        session.reject(
            ViolationCode::UnresolvedReference,
            Some(&edge.node),
            format!("the node does not require {}", edge.requires),
        );
        if let Some(found) = session.violations.last_mut() {
            found.related.push(Subject::Edge(edge.clone()));
        }
        return Vec::new();
    }
    assert!(add || present, "a removal names an edge that exists");
    let write = if add {
        session.put(GraphRecord::Edge(edge.clone()))
    } else {
        session.remove(GraphKey::Edge(edge.clone()))
    };
    vec![write]
}

/// The retired-key record a removal writes, so the key never comes back (Invariants).
pub(super) fn retire(session: &Session<'_>, key: RetiredKey) -> Write {
    session.put(GraphRecord::RetiredKey(key))
}
