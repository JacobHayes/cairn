//! Small edits to a node inside a graph document, shared by the write applier and the
//! mutation handlers that check an edit before writing it.

use cairn_schema::{
    BoundedSet, Graph, KeyRefs, KindKey, Node, NodeKey, ParticipationSource, Participations,
    Resource,
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
