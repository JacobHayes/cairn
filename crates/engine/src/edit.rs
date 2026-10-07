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
/// every edge into or out of it, and everything attached to those nodes. Only the generated
/// operations remove one node at a time; the engine itself removes in batches.
#[cfg(feature = "testing")]
pub(crate) fn full_removal(graph: &Graph, node: &NodeKey) -> Removal {
    match full_removals(graph, std::slice::from_ref(node)).pop() {
        Some(removal) => removal,
        None => unreachable!("one removal per node asked for"),
    }
}

/// A18: the full removal of each of `nodes`, from one [`RemovalIndex`]: O(n log n + edges)
/// once, then each removal's subtree with its nodes' edges, so many removals (an upgrade's
/// orphans) cost at most depth x (n + edges) together.
pub(crate) fn full_removals(graph: &Graph, nodes: &[NodeKey]) -> Vec<Removal> {
    let mut index = RemovalIndex::build(graph);
    nodes
        .iter()
        .map(|node| {
            let inside = index.subtree(graph, node);
            let mut removal = Removal {
                node: node.clone(),
                descendants: inside.iter().skip(1).cloned().collect(),
                edges: BTreeSet::new(),
                resources: BTreeSet::new(),
                annotations: BTreeSet::new(),
                participations: BTreeSet::new(),
            };
            for key in &inside {
                removal.edges.extend(index.edges(graph, key));
                removal.annotations.extend(index.annotations(graph, key));
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

/// What a removal reaches, indexed once for a graph (A18): each node's children, the edges
/// into and out of each node, and the notes on each. Built in O(n log n + edges + notes).
///
/// The index may hold more than the graph does, never less: every lookup is read through
/// the graph as it now stands, so a node, edge, or note the graph no longer holds, or a
/// child that has moved to another parent, is skipped, and a skipped child's subtree is not
/// walked (a removed child went with it; a moved one is indexed under its new parent too).
/// A removal only takes things away, so it leaves the index true; any other write to the
/// graph is folded in by [`RemovalIndex::observe`], which indexes what it added. Each lookup
/// then costs its own subtree and that subtree's edges and notes, never the whole graph, so
/// a patch's removals cost the index once plus what they remove, whatever else the patch
/// writes between them.
pub(crate) struct RemovalIndex {
    children: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
    edges: BTreeMap<NodeKey, BTreeSet<Edge>>,
    annotations: BTreeMap<NodeKey, BTreeSet<AttachmentKey>>,
    /// The nodes, edges, and notes visited since last taken, building included (rung 3
    /// budgets it).
    operations: u64,
}

impl RemovalIndex {
    /// Indexes `graph`.
    pub(crate) fn build(graph: &Graph) -> Self {
        let mut index = RemovalIndex {
            children: BTreeMap::new(),
            edges: BTreeMap::new(),
            annotations: BTreeMap::new(),
            operations: 0,
        };
        for node in graph.nodes.values() {
            index.index_node(node);
        }
        for note in graph.state.annotations.values() {
            index.index_annotation(note);
        }
        index
    }

    /// Indexes a node as `graph` holds it: under its parent, and with its edges.
    fn index_node(&mut self, node: &Node<KeyRefs>) {
        self.operations += 1;
        if let Some(parent) = &node.parent {
            self.children
                .entry(parent.clone())
                .or_default()
                .insert(node.key.clone());
        }
        for requirement in node.requires.iter() {
            self.index_edge(&Edge {
                node: node.key.clone(),
                requires: requirement.clone(),
            });
        }
    }

    fn index_edge(&mut self, edge: &Edge) {
        self.operations += 1;
        for end in [&edge.node, &edge.requires] {
            self.edges
                .entry(end.clone())
                .or_default()
                .insert(edge.clone());
        }
    }

    fn index_annotation(&mut self, note: &cairn_schema::Annotation) {
        self.operations += 1;
        if let Some(node) = &note.body.node {
            self.annotations
                .entry(node.clone())
                .or_default()
                .insert(note.body.key.clone());
        }
    }

    /// Folds in a write `graph` (the indexed graph, `id`) has just taken: a node put whole or
    /// a field of it, an edge, or a note is indexed as the graph now holds it. Anything else
    /// a graph write puts (state, resources, participations, keys) is read from the graph at
    /// lookup, and a removal only takes things away, so neither needs indexing. Returns
    /// false when the write replaced the graph wholesale and the index must be built again.
    #[must_use]
    pub(crate) fn observe(
        &mut self,
        id: &cairn_schema::GraphId,
        graph: &Graph,
        write: &cairn_schema::Write,
    ) -> bool {
        use cairn_schema::{GraphRecord, Record, RecordKey, Write};
        match write {
            Write::Put(Record::Graph {
                graph: written,
                record,
            }) if written == id => {
                match record {
                    GraphRecord::Node(cairn_schema::Node { key, .. })
                    | GraphRecord::NodeField { node: key, .. } => {
                        if let Some(node) = graph.nodes.get(key) {
                            self.index_node(node);
                        }
                    }
                    GraphRecord::Edge(edge) => self.index_edge(edge),
                    GraphRecord::Annotation(note) => self.index_annotation(note),
                    // Read from the graph at lookup, or not reach at all.
                    GraphRecord::Role(_)
                    | GraphRecord::Kind(_)
                    | GraphRecord::DefaultOwner(_)
                    | GraphRecord::Participation { .. }
                    | GraphRecord::Resource { .. }
                    | GraphRecord::RetiredKey(_)
                    | GraphRecord::NodeState { .. }
                    | GraphRecord::LocalEdit { .. }
                    | GraphRecord::Answer { .. }
                    | GraphRecord::RoleFill { .. }
                    | GraphRecord::Pin { .. }
                    | GraphRecord::Snooze { .. }
                    | GraphRecord::Overrides { .. }
                    | GraphRecord::Tombstone(_) => {}
                }
                true
            }
            Write::CopyGraph { to, .. } => to != id,
            Write::Remove(RecordKey::Graph(removed)) => removed != id,
            Write::Remove(RecordKey::Domain(_)) => false,
            Write::Put(_) | Write::Remove(_) => true,
        }
    }

    /// `node` and every descendant `graph` holds, the node first, then depth first in the
    /// order [`crate::graph::Tree::descendants`] gives. A child the graph no longer holds
    /// under this parent is skipped, with its subtree.
    pub(crate) fn subtree(&mut self, graph: &Graph, node: &NodeKey) -> Vec<NodeKey> {
        let children = |parent: &NodeKey| self.children.get(parent).into_iter().flatten();
        let mut found = vec![node.clone()];
        let mut seen = BTreeSet::from([node]);
        let mut stack: Vec<(&NodeKey, &NodeKey)> =
            children(node).map(|child| (node, child)).collect();
        let mut visited: u64 = 0;
        while let Some((parent, next)) = stack.pop() {
            visited += 1;
            let held = graph
                .nodes
                .get(next)
                .is_some_and(|found| found.parent.as_ref() == Some(parent));
            // A cycle in a broken candidate must not loop forever.
            if !held || !seen.insert(next) {
                continue;
            }
            found.push(next.clone());
            stack.extend(children(next).map(|child| (next, child)));
        }
        self.operations += visited;
        assert!(found.len() <= graph.nodes.len() + 1);
        found
    }

    /// The edges into and out of `node` that `graph` holds.
    pub(crate) fn edges(&mut self, graph: &Graph, node: &NodeKey) -> Vec<Edge> {
        let indexed = self.edges.get(node);
        self.operations +=
            indexed.map_or(0, |edges| u64::try_from(edges.len()).unwrap_or(u64::MAX));
        indexed
            .into_iter()
            .flatten()
            .filter(|edge| {
                graph
                    .nodes
                    .get(&edge.node)
                    .is_some_and(|dependent| dependent.requires.as_set().contains(&edge.requires))
            })
            .cloned()
            .collect()
    }

    /// The notes on `node` that `graph` holds.
    pub(crate) fn annotations(&mut self, graph: &Graph, node: &NodeKey) -> Vec<AttachmentKey> {
        let indexed = self.annotations.get(node);
        self.operations += indexed.map_or(0, |keys| u64::try_from(keys.len()).unwrap_or(u64::MAX));
        indexed
            .into_iter()
            .flatten()
            .filter(|key| {
                graph
                    .state
                    .annotations
                    .get(key)
                    .is_some_and(|note| note.body.node.as_ref() == Some(node))
            })
            .cloned()
            .collect()
    }

    /// The nodes, edges, and notes visited since the last call, building included, and the
    /// count reset.
    pub(crate) fn take_operations(&mut self) -> u64 {
        std::mem::take(&mut self.operations)
    }
}
