//! Graph structure (A1, A3, A6, A7, A10, A18, B4, B5, B10): nodes, edges, roles, kinds,
//! participations, and resources, in a journey or a route's draft. In a journey, an edit to a
//! route-copied node sets its local-edit marker (B4) in the same event.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, Edge, GraphKey, GraphRecord, KeyRefs, Limit, LocalEdit, Mutation, Node,
    NodeField, NodeFieldValue, NodeKey, NodeState, Payload, Provenance, Resource, RetiredKey,
    Subject, ViolationCode, Write,
};

use super::Session;

/// Applies a structural mutation to the patch's graph.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::AddNode { node } => add_node(session, node),
        Mutation::SetNodeField { node, value } => set_field(session, node, value),
        Mutation::ReplaceNode { node } => replace_node(session, node),
        Mutation::RemoveNode { removal } => super::removal::remove(session, removal),
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
            marked(
                session,
                node,
                LocalEdit::Participation(kind.clone()),
                session.put(record),
            )
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
            marked(
                session,
                node,
                LocalEdit::Participation(kind.clone()),
                session.remove(key),
            )
        }
        other => super::graph_parts::apply(session, other),
    }
}

/// The writes of an edit to a node, with its local-edit marker when one is due (B4).
pub(super) fn marked(
    session: &Session<'_>,
    node: &NodeKey,
    edit: LocalEdit,
    write: Write,
) -> Vec<Write> {
    let mut writes = vec![write];
    writes.extend(session.marker(node, edit));
    writes
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

/// A1: a new node, in its initial state with `local` provenance in a journey (B1, B4). A key
/// the graph used or retired is never reused (Invariants).
fn add_node(session: &mut Session<'_>, node: &Node<KeyRefs>) -> Vec<Write> {
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let problem = if graph.nodes.get(&node.key).is_some() {
        Some((ViolationCode::DuplicateKey, "a node with this key exists"))
    } else if graph.retired_keys.nodes.contains(&node.key) {
        Some((
            ViolationCode::RetiredKeyReused,
            "this key was removed from the graph and cannot come back",
        ))
    } else if Limit::NodeCount.check(graph.nodes.len() + 1).is_err() {
        Some((
            ViolationCode::LimitExceeded,
            "the graph is at node_count_max",
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
            state: NodeState::initial(node.kind(), Provenance::Local),
        }));
    }
    writes
}

/// B4, B5: one field of a node; the kind must have it (A1a).
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
            format!("a {} has no {field:?} field (A1a)", kind.name()),
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
    marked(session, key, LocalEdit::Field(field), session.put(record))
}

/// B7: a node replaced whole, as a kind or answer-type change does. Every field, edge,
/// participation, and resource that differs is marked (B4). A new kind has a new machine,
/// so the node starts over in its initial state, keeping its provenance.
fn replace_node(session: &mut Session<'_>, node: &Node<KeyRefs>) -> Vec<Write> {
    if !session.require(&node.key) {
        return Vec::new();
    }
    let Some(old) = session.node(&node.key).cloned() else {
        unreachable!("the node was required")
    };
    assert_eq!(old.key, node.key);
    let mut writes = vec![session.put(GraphRecord::Node(node.clone()))];
    for edit in differences(&old, node) {
        writes.extend(session.marker(&node.key, edit));
    }
    let stored = session
        .journey()
        .and_then(|journey| journey.graph.state.nodes.get(&node.key).cloned());
    if let Some(stored) = stored
        && old.kind() != node.kind()
    {
        writes.push(session.put(GraphRecord::NodeState {
            node: node.key.clone(),
            state: NodeState::initial(node.kind(), stored.provenance),
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

/// What a replacement changes, as local-edit markers.
fn differences(old: &Node<KeyRefs>, new: &Node<KeyRefs>) -> BTreeSet<LocalEdit> {
    let mut edits = BTreeSet::new();
    for field in NodeField::ALL {
        if NodeFieldValue::read(field, old) != NodeFieldValue::read(field, new) {
            edits.insert(LocalEdit::Field(field));
        }
    }
    let requires = old
        .requires
        .as_set()
        .symmetric_difference(new.requires.as_set());
    edits.extend(requires.cloned().map(LocalEdit::Requires));
    let (before, after) = (old.participations.as_map(), new.participations.as_map());
    for kind in before.keys().chain(after.keys()) {
        if before.get(kind) != after.get(kind) {
            edits.insert(LocalEdit::Participation(kind.clone()));
        }
    }
    // By key, so a node with many resources compares in O(r log r).
    let (before, after) = (resources_by_key(old), resources_by_key(new));
    for key in before.keys().chain(after.keys()) {
        if before.get(key) != after.get(key) {
            edits.insert(LocalEdit::Resource((*key).clone()));
        }
    }
    let answer_type = |node: &Node<KeyRefs>| match &node.payload {
        Payload::Decision(decision) => Some(decision.answer.answer_type()),
        Payload::Deliverable(_)
        | Payload::Action(_)
        | Payload::Milestone(_)
        | Payload::Group(_) => None,
    };
    if old.kind() != new.kind() || answer_type(old) != answer_type(new) {
        // B7: a replacement's change of kind or answer type has no field of its own.
        edits.insert(LocalEdit::Shape);
    }
    assert!(
        old != new || edits.is_empty(),
        "an unchanged node differs in nothing"
    );
    edits
}

fn resources_by_key(node: &Node<KeyRefs>) -> BTreeMap<&AttachmentKey, &Resource<KeyRefs>> {
    node.resources
        .iter()
        .map(|resource| (&resource.key, resource))
        .collect()
}

/// A3: an explicit edge, stored on its dependent; a journey marks the dependent (B4).
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
    marked(
        session,
        &edge.node,
        LocalEdit::Requires(edge.requires.clone()),
        write,
    )
}

/// The retired-key record a removal writes, so the key never comes back (Invariants).
pub(super) fn retire(session: &Session<'_>, key: RetiredKey) -> Write {
    session.put(GraphRecord::RetiredKey(key))
}
