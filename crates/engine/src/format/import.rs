//! Building a graph from a file document (A13, A14; PRD, Keys and references): supplied
//! keys are kept; against a base version, a keyless node takes the key of the base node at
//! its path, a keyless role or kind the key of the base one with its id, and a keyless
//! resource the key of the base node's resource with its title; the rest are minted through
//! the host's allocator, never as a key the base holds or retired. Every path and id is
//! resolved to a key, and the result is validated like any other graph, so every fixture
//! loads through the model.
//!
//! Cost at the limits: one pass over the file's nodes, roles, and kinds to collect keys and
//! paths (and one over the base's to match them), and one to resolve references by map
//! lookup, O(n log n), then graph validation.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, FileRefs, KeyAllocator, KeyRefs, Keyed, KindKey, MapReferences, Node, NodeField,
    NodeKey, ParticipationKind, Path, Prefixed, RetiredKey, Role, RoleKey, RouteFile, Slug,
    Subject, Violation, ViolationCode, Violations, mint,
};

use crate::graph::{Document, Graph};
use crate::validate::violation;

/// Builds a graph from a route file with no base version: keys kept or minted, references
/// resolved, invariants checked (A13, A15).
///
/// # Errors
///
/// Every key repeated, path that does not resolve, and invariant the graph breaks.
///
/// # Panics
///
/// When the allocator keeps returning keys already in use, or the engine's own key
/// accounting breaks.
pub fn from_file(file: &RouteFile, allocator: &mut dyn KeyAllocator) -> Result<Graph, Violations> {
    import(file, None, allocator)
}

/// A13: builds a graph from a route file against the version it extends: an unchanged node
/// keeps its key by path, a moved node keeps the key it carries, an unknown path mints a new
/// one.
///
/// # Errors
///
/// Every key repeated, path that does not resolve, and invariant the graph breaks.
///
/// # Panics
///
/// When the allocator keeps returning keys already in use, or the engine's own key
/// accounting breaks.
pub fn import(
    file: &RouteFile,
    base: Option<&Document>,
    allocator: &mut dyn KeyAllocator,
) -> Result<Graph, Violations> {
    let matched;
    let (file, reserved) = match base {
        Some(base) => {
            matched = super::matching::with_base_keys(file, base);
            (&matched, super::matching::Reserved::of(base))
        }
        None => (file, super::matching::Reserved::default()),
    };
    let mut keys = Keys::collect(file, allocator, &reserved);
    let mut resolver = Resolver::new(file, &keys);
    let roles = file
        .roles
        .as_slice()
        .iter()
        .zip(keys.roles.drain(..))
        .map(|(role, key)| Role {
            key,
            id: role.id.clone(),
            title: role.title.clone(),
            multi: role.multi,
        });
    let kinds = file
        .participation_kinds
        .as_slice()
        .iter()
        .zip(keys.kinds.drain(..))
        .map(|(kind, key)| ParticipationKind {
            key,
            id: kind.id.clone(),
            title: kind.title.clone(),
            multi: kind.multi,
        });
    let (roles, kinds): (Vec<_>, Vec<_>) = (roles.collect(), kinds.collect());
    let default_owner = file
        .default_owner
        .as_ref()
        .and_then(|role| resolver.role(role, None));
    let nodes: Vec<Node<KeyRefs>> = file
        .nodes
        .as_slice()
        .iter()
        .map(|node| resolver.node(node))
        .collect();
    assert!(
        resolver.node_keys.next().is_none() && resolver.resource_keys.next().is_none(),
        "every node and resource key was used"
    );
    let mut violations = keys.violations;
    if !violations.is_empty() {
        // Two objects with one key cannot be put in one graph to check it further.
        violations.append(&mut resolver.violations);
        return Err(collected(violations));
    }
    let document = Document {
        default_owner,
        roles: keyed(roles),
        participation_kinds: keyed(kinds),
        nodes: keyed(nodes),
        ..Document::default()
    };
    // A15: references that did not resolve were left out, so every invariant the rest of the
    // graph breaks is reported with them.
    violations.append(&mut resolver.violations);
    // A route has no answers, so no deployment changes its relevance.
    Graph::checked(document, violations, &cairn_schema::Deployment::default())
}

fn collected(violations: Vec<Violation>) -> Violations {
    match Violations::new(violations) {
        Ok(violations) => violations,
        Err(error) => unreachable!("collected only when there is one: {error}"),
    }
}

/// Collects values whose keys were checked unique and whose count the file already bounded.
fn keyed<V: cairn_schema::HasKey, L: cairn_schema::collections::LimitOf>(
    values: Vec<V>,
) -> Keyed<V, L> {
    match Keyed::new(values) {
        Ok(keyed) => keyed,
        Err(error) => panic!("keys were checked unique and counts bounded by the file: {error}"),
    }
}

/// Every object's key, supplied or minted, in file order, and the repeats found.
struct Keys {
    nodes: Vec<NodeKey>,
    roles: Vec<RoleKey>,
    kinds: Vec<KindKey>,
    resources: Vec<AttachmentKey>,
    violations: Vec<Violation>,
}

impl Keys {
    fn collect(
        file: &RouteFile,
        allocator: &mut dyn KeyAllocator,
        reserved: &super::matching::Reserved,
    ) -> Self {
        let nodes = file.nodes.as_slice();
        let resources: Vec<Option<AttachmentKey>> = nodes
            .iter()
            .flat_map(|node| node.resources.iter().map(|resource| resource.key.clone()))
            .collect();
        let mut violations = Vec::new();
        let supplied_nodes: Vec<_> = nodes.iter().map(|node| node.key.clone()).collect();
        let roles: Vec<_> = file
            .roles
            .as_slice()
            .iter()
            .map(|role| role.key.clone())
            .collect();
        let kinds: Vec<_> = file
            .participation_kinds
            .as_slice()
            .iter()
            .map(|kind| kind.key.clone())
            .collect();
        let retired = &reserved.retired;
        refuse_retired(
            &supplied_nodes,
            &retired.nodes,
            RetiredKey::Node,
            &mut violations,
        );
        refuse_retired(&roles, &retired.roles, RetiredKey::Role, &mut violations);
        refuse_retired(&kinds, &retired.kinds, RetiredKey::Kind, &mut violations);
        let keys = Keys {
            nodes: fill(
                supplied_nodes,
                allocator,
                &mut violations,
                Subject::Node,
                &reserved.nodes,
            ),
            roles: fill(
                roles,
                allocator,
                &mut violations,
                Subject::Role,
                &reserved.roles,
            ),
            kinds: fill(
                kinds,
                allocator,
                &mut violations,
                Subject::Kind,
                &reserved.kinds,
            ),
            resources: fill(
                resources,
                allocator,
                &mut violations,
                Subject::Attachment,
                &reserved.resources,
            ),
            violations,
        };
        assert_eq!(keys.nodes.len(), nodes.len());
        assert_eq!(keys.roles.len(), file.roles.len());
        keys
    }
}

/// Invariants (no key is ever reused): a supplied key the base retired is rejected.
fn refuse_retired<K: Ord + Clone>(
    supplied: &[Option<K>],
    retired: &BTreeSet<K>,
    retired_key: fn(K) -> RetiredKey,
    violations: &mut Vec<Violation>,
) {
    for key in supplied
        .iter()
        .flatten()
        .filter(|key| retired.contains(*key))
    {
        violations.push(crate::validate::retired_key_reused(
            None,
            &retired_key(key.clone()),
        ));
    }
}

/// Keeps each supplied key, reporting repeats, and mints the missing ones, skipping any body
/// the allocator returns that is already in use or `reserved`.
fn fill<K: Prefixed + Ord + Clone>(
    supplied: Vec<Option<K>>,
    allocator: &mut dyn KeyAllocator,
    violations: &mut Vec<Violation>,
    subject: fn(K) -> Subject,
    reserved: &BTreeSet<K>,
) -> Vec<K> {
    let mut used: BTreeSet<K> = BTreeSet::new();
    for key in supplied.iter().flatten() {
        if !used.insert(key.clone()) {
            let mut found = violation(
                ViolationCode::DuplicateKey,
                format!("the key {key} is given to two objects"),
            );
            found.at.subject = Some(subject(key.clone()));
            violations.push(found);
        }
    }
    let attempts_max = (supplied.len() + reserved.len()) * 2 + 1;
    let mut keys = Vec::with_capacity(supplied.len());
    for key in supplied {
        let key = key.unwrap_or_else(|| {
            for _ in 0..attempts_max {
                let candidate: K = mint(allocator);
                if !reserved.contains(&candidate) && used.insert(candidate.clone()) {
                    return candidate;
                }
            }
            panic!("the key allocator kept returning keys already in use")
        });
        keys.push(key);
    }
    keys
}

/// Resolves paths and ids to keys, collecting every reference that does not resolve.
struct Resolver {
    paths: BTreeMap<Path, NodeKey>,
    roles: BTreeMap<Slug, RoleKey>,
    kinds: BTreeMap<Slug, KindKey>,
    violations: Vec<Violation>,
    /// The node being resolved, for locations.
    at: Option<Path>,
    /// The keys of the nodes not yet resolved, in file order.
    node_keys: std::vec::IntoIter<NodeKey>,
    /// The keys of the resources not yet resolved, in file order.
    resource_keys: std::vec::IntoIter<AttachmentKey>,
}

impl Resolver {
    fn new(file: &RouteFile, keys: &Keys) -> Self {
        let mut resolver = Resolver {
            paths: BTreeMap::new(),
            roles: BTreeMap::new(),
            kinds: BTreeMap::from([(cairn_schema::id::owner_kind_id(), KindKey::owner())]),
            violations: Vec::new(),
            at: None,
            node_keys: keys.nodes.clone().into_iter(),
            resource_keys: keys.resources.clone().into_iter(),
        };
        // A path past the depth limit or a sibling id given twice is the graph check's to
        // report (from the tree); here the first holder of a path keeps it.
        for (node, key) in file.nodes.as_slice().iter().zip(&keys.nodes) {
            let path = match &node.parent {
                None => Ok(Path::root(node.id.clone())),
                Some(parent) => parent.child(node.id.clone()),
            };
            if let Ok(path) = path {
                resolver.paths.entry(path).or_insert_with(|| key.clone());
            }
        }
        for (role, key) in file.roles.as_slice().iter().zip(&keys.roles) {
            resolver.roles.insert(role.id.clone(), key.clone());
        }
        let kinds = file.participation_kinds.as_slice().iter().zip(&keys.kinds);
        for (kind, key) in kinds {
            resolver.kinds.insert(kind.id.clone(), key.clone());
        }
        resolver
    }

    fn unresolved(&mut self, what: String, field: Option<NodeField>, code: ViolationCode) {
        let mut found = violation(code, what);
        found.at.path.clone_from(&self.at);
        found.at.field = field;
        self.violations.push(found);
    }

    fn path(&mut self, path: &Path, field: Option<NodeField>) -> Option<NodeKey> {
        let key = self.paths.get(path).cloned();
        if key.is_none() {
            let what = format!("no node has the path {path}");
            self.unresolved(what, field, ViolationCode::UnresolvedReference);
        }
        key
    }

    fn role(&mut self, id: &Slug, field: Option<NodeField>) -> Option<RoleKey> {
        let key = self.roles.get(id).cloned();
        if key.is_none() {
            let what = format!("no role has the id {id}");
            self.unresolved(what, field, ViolationCode::UnresolvedReference);
        }
        key
    }

    fn kind(&mut self, id: &Slug) -> Option<KindKey> {
        let key = self.kinds.get(id).cloned();
        if key.is_none() {
            let what = format!("no participation kind has the id {id}");
            self.unresolved(what, None, ViolationCode::UndeclaredKind);
        }
        key
    }

    /// Resolves one node. A reference that does not resolve is reported and left out (a
    /// parent that does not resolve leaves the node a root), so the rest of the graph can
    /// still be checked (A15).
    fn node(&mut self, node: &Node<FileRefs>) -> Node<KeyRefs> {
        self.at = Some(match &node.parent {
            None => Path::root(node.id.clone()),
            Some(parent) => parent
                .child(node.id.clone())
                .unwrap_or_else(|_| parent.clone()),
        });
        node.map_references(self)
    }
}

/// The lookups one node's translation asks of the file's paths, ids, and collected keys.
impl MapReferences<FileRefs, KeyRefs> for Resolver {
    fn node(&mut self, node: &Path, field: Option<NodeField>) -> Option<NodeKey> {
        self.path(node, field)
    }

    fn role(&mut self, role: &Slug, field: Option<NodeField>) -> Option<RoleKey> {
        Resolver::role(self, role, field)
    }

    fn kind(&mut self, kind: &Slug) -> Option<KindKey> {
        Resolver::kind(self, kind)
    }

    fn own_key(&mut self, _: &Option<NodeKey>) -> NodeKey {
        match self.node_keys.next() {
            Some(key) => key,
            None => unreachable!("a key was collected for every node"),
        }
    }

    fn resource_key(&mut self, _: &Option<AttachmentKey>) -> AttachmentKey {
        match self.resource_keys.next() {
            Some(key) => key,
            None => unreachable!("a key was collected for every resource"),
        }
    }

    /// Keys can be longer than the paths and ids they replace, so a draft within the body
    /// limit as written can pass it resolved.
    fn draft_too_long(&mut self, _: &Option<AttachmentKey>, bytes: usize) {
        let what = format!("the message draft resolves to {bytes} bytes, past body_bytes_max");
        self.unresolved(what, None, ViolationCode::LimitExceeded);
        if let Some(found) = self.violations.last_mut() {
            found.limit = Some(cairn_schema::Limit::BodyBytes);
        }
    }

    /// Only two kinds given one key resolve alike, which the key check reports too.
    fn kinds_collapsed(&mut self, kind: &Slug) {
        let what = format!("the participation kind {kind} resolves to a kind another one has");
        self.unresolved(what, None, ViolationCode::DuplicateKey);
    }
}
