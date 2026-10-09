//! Node removal (A18, B4): a node goes with its subtree, every incident edge, and everything
//! attached to them, but only as far as its author saw: a removal names all of it, and one
//! whose subtree or edges gained anything it does not name (a child added or moved in, an
//! edge, a resource, a note, a participation) is rejected rather than widened, so a safe
//! retry (H5) never removes work its author never saw. Names that no longer apply (a child
//! moved out since) are ignored: removing less than the author saw is safe. Every removed key
//! is retired, and in a journey each route-copied node in the subtree gets a tombstone so an
//! upgrade does not bring it back (B4).
//!
//! Cost at the limits: the patch's first removal indexes the graph once ([`RemovalIndex`],
//! O(n log n) plus the edges and notes); each removal then walks its own subtree with that
//! subtree's edges and notes, and makes one write per removed record, so a patch of
//! `node_count_max` removals costs the index once plus what it removes, not a graph pass
//! per removal. Every other write folds what it adds into the index (`mutate::apply`).

use std::collections::BTreeSet;

use cairn_schema::{
    Edge, GraphKey, GraphRecord, LocalEdit, NodeKey, ParticipationRef, Provenance, Removal,
    RetiredKey, Subject, ViolationCode, Write,
};

use super::Session;
use super::structure::retire;
use crate::edit::RemovalIndex;

/// What the subtree holds now.
struct Reach {
    nodes: Vec<NodeKey>,
    edges: BTreeSet<Edge>,
    resources: BTreeSet<cairn_schema::AttachmentKey>,
    annotations: BTreeSet<cairn_schema::AttachmentKey>,
    participations: BTreeSet<ParticipationRef>,
}

impl Reach {
    fn of(graph: &cairn_schema::Graph, index: &mut RemovalIndex, node: &NodeKey) -> Self {
        let nodes = index.subtree(graph, node);
        let mut reach = Reach {
            edges: BTreeSet::new(),
            resources: BTreeSet::new(),
            annotations: BTreeSet::new(),
            participations: BTreeSet::new(),
            nodes: Vec::new(),
        };
        for key in &nodes {
            reach.edges.extend(index.edges(graph, key));
            reach.annotations.extend(index.annotations(graph, key));
            let Some(found) = graph.nodes.get(key) else {
                continue;
            };
            reach
                .resources
                .extend(found.resources.iter().map(|resource| resource.key.clone()));
            reach
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
        assert_eq!(
            nodes.first(),
            Some(node),
            "the removed node leads its subtree"
        );
        reach.nodes = nodes;
        reach
    }

    /// What the subtree holds that the removal does not name.
    fn unnamed(&self, removal: &Removal) -> Vec<Subject> {
        let mut extra: Vec<Subject> = Vec::new();
        let descendants = self.nodes.iter().skip(1);
        extra.extend(
            descendants
                .filter(|key| !removal.descendants.contains(*key))
                .map(|key| Subject::Node(key.clone())),
        );
        extra.extend(
            self.edges
                .difference(&removal.edges)
                .cloned()
                .map(Subject::Edge),
        );
        let attachments = self
            .resources
            .difference(&removal.resources)
            .chain(self.annotations.difference(&removal.annotations));
        extra.extend(attachments.cloned().map(Subject::Attachment));
        extra.extend(
            self.participations
                .difference(&removal.participations)
                .map(|participation| Subject::Node(participation.node.clone())),
        );
        assert!(
            extra.len()
                <= self.nodes.len()
                    + self.edges.len()
                    + self.resources.len()
                    + self.annotations.len()
                    + self.participations.len()
        );
        extra
    }
}

/// A18: removes the node and everything its removal names.
pub(super) fn remove(session: &mut Session<'_>, removal: &Removal) -> Vec<Write> {
    if !session.require(&removal.node) {
        return Vec::new();
    }
    let cached = session.removal_index.take();
    let Some(graph) = session.graph() else {
        unreachable!("a graph patch targets an existing graph")
    };
    let mut index = cached.unwrap_or_else(|| RemovalIndex::build(graph));
    let reach = Reach::of(graph, &mut index, &removal.node);
    session.removal_operations += index.take_operations();
    session.removal_index = Some(index);
    let extra = reach.unnamed(removal);
    if !extra.is_empty() {
        session.reject(
            ViolationCode::RemovalWidened,
            Some(&removal.node),
            "the subtree holds things this removal does not name (A18): fetch it again and name them",
        );
        if let Some(found) = session.violations.last_mut() {
            found.related = extra;
        }
        return Vec::new();
    }
    let inside: BTreeSet<&NodeKey> = reach.nodes.iter().collect();
    let mut writes = Vec::new();
    // Edges into the subtree from outside hang off nodes that stay.
    for edge in &reach.edges {
        if !inside.contains(&edge.node) {
            writes.push(session.remove(GraphKey::Edge(edge.clone())));
            // B4: the node that stays lost a route-copied edge.
            writes.extend(session.marker(&edge.node, LocalEdit::Requires(edge.requires.clone())));
        }
    }
    for annotation in &reach.annotations {
        let node = session.journey().and_then(|journey| {
            let found = journey.graph.state.annotations.get(annotation);
            found.and_then(|found| found.body.node.clone())
        });
        writes.push(session.remove(GraphKey::Annotation {
            annotation: annotation.clone(),
            node,
        }));
    }
    for key in &reach.nodes {
        writes.push(session.remove(GraphKey::Node(key.clone())));
        writes.push(retire(session, RetiredKey::Node(key.clone())));
        if from_route(session, key) {
            writes.push(session.put(GraphRecord::Tombstone(key.clone())));
        }
    }
    // B13: a removed member leaves its insertion, which goes with its last (no tombstone: a
    // member is not route-copied).
    writes.extend(super::insertion::narrowed(session, |member| {
        !inside.contains(member)
    }));
    assert!(writes.len() >= reach.nodes.len() * 2);
    writes
}

/// B4: whether a journey copied this node from its route.
fn from_route(session: &Session<'_>, key: &NodeKey) -> bool {
    session.journey().is_some_and(|journey| {
        journey
            .graph
            .state
            .nodes
            .get(key)
            .is_some_and(|stored| stored.provenance == Provenance::FromRoute)
    })
}
