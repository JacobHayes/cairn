//! Exporting a route version or draft to the file document (A13; ARCHITECTURE, File format):
//! every key kept, every reference written as a path or an id, and everything in one order
//! (nodes by path, so a parent precedes its children; roles and kinds by id; a node's fields
//! in the schema's order), so two exports of one graph are the same bytes and an edit shows
//! as a small version-control diff.
//!
//! Cost at the limits: one tree build (O(n log n)) and one pass over the nodes, each
//! reference a map lookup.

use std::collections::BTreeMap;

use cairn_schema::{
    AttachmentKey, BoundedVec, FileRefs, FormatVersion, KeyRefs, KindKey, MapReferences, Markdown,
    Node, NodeField, NodeKey, ParticipationKind, Path, Role, RoleKey, RouteFile, RouteId,
    RouteKind, Slug, Title, VersionNumber,
};

use crate::graph::Graph;

/// What a route file names besides its graph: the route and the version it extends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteHeading {
    /// The route's id.
    pub route: RouteId,
    /// The route's name.
    pub name: Title,
    /// What the route is for.
    pub description: Option<Markdown>,
    /// Process or segment.
    pub kind: RouteKind,
    /// The published version the exported graph extends, if any.
    pub extends: Option<VersionNumber>,
}

/// A13: the file document for a route version or draft, deterministic.
///
/// # Panics
///
/// When the graph holds a journey's state, which no route has, or more than the limits its
/// type already bounds.
#[must_use]
pub fn export(graph: &Graph, heading: &RouteHeading) -> RouteFile {
    let document = graph.document();
    assert!(
        document.state.is_empty(),
        "a route graph holds no journey state"
    );
    let mut names = Names::of(graph);
    let mut nodes: Vec<(&Path, Node<FileRefs>)> = document
        .nodes
        .values()
        .map(|node| (names.path(&node.key), node.map_references(&mut names)))
        .collect();
    nodes.sort_by_key(|(path, _)| *path);
    let mut roles: Vec<Role<FileRefs>> = document
        .roles
        .values()
        .map(|role| Role {
            key: Some(role.key.clone()),
            id: role.id.clone(),
            title: role.title.clone(),
            multi: role.multi,
        })
        .collect();
    roles.sort_by(|left, right| left.id.cmp(&right.id));
    let mut kinds: Vec<ParticipationKind<FileRefs>> = document
        .participation_kinds
        .values()
        .map(|kind| ParticipationKind {
            key: Some(kind.key.clone()),
            id: kind.id.clone(),
            title: kind.title.clone(),
            multi: kind.multi,
        })
        .collect();
    kinds.sort_by(|left, right| left.id.cmp(&right.id));
    let file = RouteFile {
        format: FormatVersion,
        route: heading.route.clone(),
        name: heading.name.clone(),
        description: heading.description.clone(),
        kind: heading.kind,
        extends: heading.extends,
        default_owner: document
            .default_owner
            .as_ref()
            .map(|role| names.role_id(role)),
        roles: bounded(roles),
        participation_kinds: bounded(kinds),
        nodes: bounded(nodes.into_iter().map(|(_, node)| node).collect()),
    };
    assert_eq!(file.nodes.len(), document.nodes.len());
    file
}

fn bounded<T, L: cairn_schema::collections::LimitOf>(values: Vec<T>) -> BoundedVec<T, L> {
    match BoundedVec::new(values) {
        Ok(values) => values,
        Err(error) => panic!("a graph's collections fit the file's bounds: {error}"),
    }
}

/// The path of every node and the id of every role and kind, by key.
struct Names<'a> {
    graph: &'a Graph,
    roles: BTreeMap<&'a RoleKey, &'a Slug>,
    kinds: BTreeMap<&'a KindKey, &'a Slug>,
}

impl<'a> Names<'a> {
    fn of(graph: &'a Graph) -> Self {
        let document = graph.document();
        Names {
            graph,
            roles: document
                .roles
                .values()
                .map(|role| (&role.key, &role.id))
                .collect(),
            kinds: document
                .participation_kinds
                .values()
                .map(|kind| (&kind.key, &kind.id))
                .collect(),
        }
    }

    fn path(&self, key: &NodeKey) -> &'a Path {
        match self.graph.tree().path(key) {
            Some(path) => path,
            None => unreachable!("every node of a valid graph has a path: {key}"),
        }
    }

    fn role_id(&self, key: &RoleKey) -> Slug {
        match self.roles.get(key) {
            Some(id) => (*id).clone(),
            None => unreachable!("every role a valid graph names exists: {key}"),
        }
    }

    fn kind_id(&self, key: &KindKey) -> Slug {
        if *key == KindKey::owner() {
            return cairn_schema::id::owner_kind_id();
        }
        match self.kinds.get(key) {
            Some(id) => (*id).clone(),
            None => unreachable!("every kind a valid graph names exists: {key}"),
        }
    }
}

impl MapReferences<KeyRefs, FileRefs> for Names<'_> {
    fn node(&mut self, node: &NodeKey, _: Option<NodeField>) -> Option<Path> {
        Some(self.path(node).clone())
    }

    fn role(&mut self, role: &RoleKey, _: Option<NodeField>) -> Option<Slug> {
        Some(self.role_id(role))
    }

    fn kind(&mut self, kind: &KindKey) -> Option<Slug> {
        Some(self.kind_id(kind))
    }

    fn own_key(&mut self, key: &NodeKey) -> Option<NodeKey> {
        Some(key.clone())
    }

    fn resource_key(&mut self, key: &AttachmentKey) -> Option<AttachmentKey> {
        Some(key.clone())
    }

    fn draft_too_long(&mut self, _: &AttachmentKey, bytes: usize) {
        panic!("an exported message draft reads back: it is {bytes} bytes, past the body limit")
    }

    fn kinds_collapsed(&mut self, kind: &KindKey) {
        unreachable!("an export names each kind by its own id: {kind}")
    }
}
