//! What one direct write touches (I5: "unless one change would touch more than ten nodes"),
//! resolved against its journey as it stands
//! (decisions/2026-10-06-the-write-wrapper-decides-on-the-patch-a-tool-would-submit.md): every
//! node a record the patch writes hangs off, by its touched set (H5); a note or link it
//! removes or moves counts the node the stored journey has it on; and skipping or reopening
//! a node counts its whole subtree, which D1a skips or reopens with it. A role fill writes
//! one record and counts none: who holds a role is one answer, as a decision that fills the
//! role is.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, Graph, GraphKey, Mutation, NodeKey, Patch, RecordKey, Transition,
};

/// The nodes `patch` touches, resolved against `journey`, its target's graph as stored
/// (`None` for a domain that is not a journey, or one that does not exist yet). `None` when
/// it touches a whole domain or graph, so it may touch every node.
#[must_use]
pub fn touched_nodes(patch: &Patch, journey: Option<&Graph>) -> Option<BTreeSet<NodeKey>> {
    let mut nodes = BTreeSet::new();
    for key in patch.touched().as_set() {
        match key {
            RecordKey::Domain(_) | RecordKey::Graph(_) | RecordKey::RouteDraft(_) => return None,
            RecordKey::InGraph { key, .. } => {
                nodes.extend(node_of(key, journey));
                // A note or link moved to another node leaves the one it was on.
                if let GraphKey::Annotation { annotation, .. } = key {
                    nodes.extend(stored_annotation_node(journey, annotation));
                }
            }
            RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::DeletedJourney(_)
            | RecordKey::Entity(_)
            | RecordKey::EntityAlias(_)
            | RecordKey::Proposal { .. } => {}
        }
    }
    if let Some(graph) = journey {
        let children = children_of(graph);
        for mutation in patch.mutations.as_slice() {
            if let Mutation::Transition {
                node,
                transition: Transition::Skip { .. } | Transition::Reopen,
            } = mutation
            {
                nodes.extend(subtree(&children, node));
            }
        }
    }
    Some(nodes)
}

/// The node a record inside a graph hangs off, if it hangs off one: a removed annotation's
/// is read from the stored journey.
fn node_of(key: &GraphKey, journey: Option<&Graph>) -> Option<NodeKey> {
    match key {
        GraphKey::Node(node)
        | GraphKey::NodeField { node, .. }
        | GraphKey::Participation { node, .. }
        | GraphKey::Resource { node, .. }
        | GraphKey::NodeState(node)
        | GraphKey::LocalEdit { node, .. }
        | GraphKey::Answer(node)
        | GraphKey::Pin(node)
        | GraphKey::Snooze(node)
        | GraphKey::Overrides(node)
        | GraphKey::Tombstone(node)
        | GraphKey::Edge(cairn_schema::Edge { node, .. })
        | GraphKey::Annotation {
            node: Some(node), ..
        } => Some(node.clone()),
        GraphKey::Annotation {
            annotation,
            node: None,
        } => stored_annotation_node(journey, annotation),
        GraphKey::Role(_)
        | GraphKey::Kind(_)
        | GraphKey::DefaultOwner
        | GraphKey::RetiredKey(_)
        | GraphKey::RoleFill(_) => None,
    }
}

/// The node the stored journey has `annotation` on, if it has it on one.
fn stored_annotation_node(journey: Option<&Graph>, annotation: &AttachmentKey) -> Option<NodeKey> {
    let stored = journey?.state.annotations.get(annotation)?;
    stored.body.node.clone()
}

/// Each node's children in `graph`.
fn children_of(graph: &Graph) -> BTreeMap<&NodeKey, Vec<&NodeKey>> {
    let mut children: BTreeMap<&NodeKey, Vec<&NodeKey>> = BTreeMap::new();
    for node in graph.nodes.values() {
        if let Some(parent) = &node.parent {
            children.entry(parent).or_default().push(&node.key);
        }
    }
    children
}

/// `node` and every node beneath it.
fn subtree(children: &BTreeMap<&NodeKey, Vec<&NodeKey>>, node: &NodeKey) -> BTreeSet<NodeKey> {
    let mut found = BTreeSet::new();
    let mut pending = vec![node];
    while let Some(next) = pending.pop() {
        // A containment tree has no cycle (PRD Invariants), so each node is met once.
        if found.insert(next.clone()) {
            pending.extend(children.get(next).into_iter().flatten().copied());
        }
    }
    found
}
