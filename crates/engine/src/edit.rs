//! Small edits to a node inside a graph document, shared by the write applier and the
//! mutation handlers that check an edit before writing it.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, BoundedSet, Edge, Graph, KeyRefs, KindKey, LocalEdit, Node, NodeField,
    NodeFieldValue, NodeKey, ParticipationRef, ParticipationSource, Participations, Payload,
    Removal, Resource,
};

/// Runs `edit` on the node with `key` and puts it back.
///
/// # Panics
///
/// When the node does not exist or the edit changes its key: a write the engine never
/// produces.
pub(crate) fn update_node<T>(
    graph: &mut Graph,
    key: &NodeKey,
    edit: impl FnOnce(&mut Node<KeyRefs>) -> T,
) -> T {
    let Some(mut node) = graph.nodes.remove(key) else {
        panic!("a write addresses node {key}, which does not exist")
    };
    let result = edit(&mut node);
    assert_eq!(node.key, *key, "an edit changed a node's key");
    let put = graph.nodes.put(node);
    assert!(
        put.is_ok(),
        "putting a node back cannot pass the node limit"
    );
    result
}

/// Adds `requirement` to the node's `requires`; false when that would pass
/// `edge_count_per_node_max`.
#[must_use]
pub(crate) fn add_requirement(node: &mut Node<KeyRefs>, requirement: &NodeKey) -> bool {
    if node.requires.as_set().contains(requirement) {
        return true;
    }
    let items = node
        .requires
        .iter()
        .cloned()
        .chain(std::iter::once(requirement.clone()));
    match BoundedSet::new(items) {
        Ok(requires) => {
            node.requires = requires;
            true
        }
        Err(_) => false,
    }
}

/// Removes `requirement` from the node's `requires`.
pub(crate) fn remove_requirement(node: &mut Node<KeyRefs>, requirement: &NodeKey) {
    let items = node
        .requires
        .iter()
        .filter(|existing| *existing != requirement)
        .cloned();
    let removed = BoundedSet::new(items);
    assert!(removed.is_ok(), "a smaller set is always within its limit");
    if let Ok(requires) = removed {
        node.requires = requires;
    }
}

/// Sets or clears the node's participation of `kind`; false when setting would pass
/// `kind_count_max`.
#[must_use]
pub(crate) fn set_participation(
    node: &mut Node<KeyRefs>,
    kind: &KindKey,
    source: Option<ParticipationSource<KeyRefs>>,
) -> bool {
    let mut map = node.participations.as_map().clone();
    match source {
        Some(source) => map.insert(kind.clone(), source),
        None => map.remove(kind),
    };
    match Participations::try_from(map) {
        Ok(participations) => {
            node.participations = participations;
            true
        }
        Err(_) => false,
    }
}

/// Puts a resource on the node, replacing the one with its key.
pub(crate) fn put_resource(node: &mut Node<KeyRefs>, resource: Resource<KeyRefs>) {
    match node
        .resources
        .iter_mut()
        .find(|existing| existing.key == resource.key)
    {
        Some(existing) => *existing = resource,
        None => node.resources.push(resource),
    }
}

/// What differs between two versions of a node, as the local-edit markers a replacement sets
/// (B4) and a re-link keeps (B9).
pub(crate) fn differences(old: &Node<KeyRefs>, new: &Node<KeyRefs>) -> BTreeSet<LocalEdit> {
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

/// A18: a removal naming everything removing `node` reaches as the graph stands: its subtree,
/// every edge into or out of it, and everything attached to those nodes.
pub(crate) fn full_removal(graph: &Graph, node: &NodeKey) -> Removal {
    match full_removals(graph, std::slice::from_ref(node)).pop() {
        Some(removal) => removal,
        None => unreachable!("one removal per node asked for"),
    }
}

/// A18: the full removal of each of `nodes`, from one tree and one index of edges and notes
/// by node: O(n log n + edges) once, then each removal's subtree with its nodes' edges, so
/// many removals (an upgrade's orphans) cost at most depth x (n + edges) together.
pub(crate) fn full_removals(graph: &Graph, nodes: &[NodeKey]) -> Vec<Removal> {
    let tree = crate::graph::Tree::build(graph);
    let mut edges: BTreeMap<&NodeKey, Vec<Edge>> = BTreeMap::new();
    for dependent in graph.nodes.values() {
        for requirement in dependent.requires.iter() {
            let edge = Edge {
                node: dependent.key.clone(),
                requires: requirement.clone(),
            };
            edges.entry(&dependent.key).or_default().push(edge.clone());
            edges.entry(requirement).or_default().push(edge);
        }
    }
    let mut annotations: BTreeMap<&NodeKey, Vec<&AttachmentKey>> = BTreeMap::new();
    for note in graph.state.annotations.values() {
        if let Some(node) = &note.body.node {
            annotations.entry(node).or_default().push(&note.body.key);
        }
    }
    nodes
        .iter()
        .map(|node| {
            let descendants: BTreeSet<NodeKey> = tree.descendants(node).into_iter().collect();
            let mut removal = Removal {
                node: node.clone(),
                descendants,
                edges: BTreeSet::new(),
                resources: BTreeSet::new(),
                annotations: BTreeSet::new(),
                participations: BTreeSet::new(),
            };
            let inside: Vec<NodeKey> = removal.nodes().cloned().collect();
            for key in &inside {
                removal
                    .edges
                    .extend(edges.get(key).into_iter().flatten().cloned());
                removal.annotations.extend(
                    annotations
                        .get(key)
                        .into_iter()
                        .flatten()
                        .map(|note| (*note).clone()),
                );
                let Some(found) = graph.nodes.get(key) else {
                    continue;
                };
                removal
                    .resources
                    .extend(found.resources.iter().map(|resource| resource.key.clone()));
                removal
                    .participations
                    .extend(
                        found
                            .participations
                            .as_map()
                            .keys()
                            .map(|kind| ParticipationRef {
                                node: key.clone(),
                                kind: kind.clone(),
                            }),
                    );
            }
            removal
        })
        .collect()
}
