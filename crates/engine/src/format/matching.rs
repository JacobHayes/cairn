//! A13: matching a route file's keyless objects against the version it extends, and the keys
//! a mint must avoid.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AttachmentKey, FileRefs, KindKey, Node, NodeKey, Path, RoleKey, RouteFile, Slug,
};

use crate::graph::{Document, Tree};

/// Keys the base holds or retired: a key minted on import is never one of them, so a node
/// the file leaves out never comes back under another's key (Invariants).
#[derive(Default)]
pub(super) struct Reserved {
    /// The keys the base retired: a file that supplies one is rejected.
    pub retired: cairn_schema::RetiredKeys,
    pub nodes: BTreeSet<NodeKey>,
    pub roles: BTreeSet<RoleKey>,
    pub kinds: BTreeSet<KindKey>,
    pub resources: BTreeSet<AttachmentKey>,
}

impl Reserved {
    pub(super) fn of(base: &Document) -> Self {
        let retired = &base.retired_keys;
        Reserved {
            retired: retired.clone(),
            nodes: base
                .nodes
                .as_map()
                .keys()
                .chain(&retired.nodes)
                .cloned()
                .collect(),
            roles: base
                .roles
                .as_map()
                .keys()
                .chain(&retired.roles)
                .cloned()
                .collect(),
            kinds: base
                .participation_kinds
                .as_map()
                .keys()
                .chain(&retired.kinds)
                .cloned()
                .collect(),
            resources: base
                .nodes
                .values()
                .flat_map(|node| node.resources.iter().map(|resource| resource.key.clone()))
                .collect(),
        }
    }
}

/// The file with each keyless node given the key of the base node at its path, each keyless
/// role or kind the key of the base one with its id, and each keyless resource the key of
/// its node's base resource with the same title, unless the file gives that key to something
/// else.
pub(super) fn with_base_keys(file: &RouteFile, base: &Document) -> RouteFile {
    let mut file = file.clone();
    match_nodes(file.nodes.as_mut_slice(), base);
    match_resources(file.nodes.as_mut_slice(), base);
    let roles = base.roles.values().map(|role| (&role.id, &role.key));
    match_ids(file.roles.as_mut_slice(), &roles.collect(), |role| {
        (&role.id, &mut role.key)
    });
    let kinds = base
        .participation_kinds
        .values()
        .map(|kind| (&kind.id, &kind.key));
    match_ids(
        file.participation_kinds.as_mut_slice(),
        &kinds.collect(),
        |kind| (&kind.id, &mut kind.key),
    );
    file
}

fn match_nodes(nodes: &mut [Node<FileRefs>], base: &Document) {
    let tree = Tree::build(base);
    let mut taken: BTreeSet<NodeKey> = nodes.iter().filter_map(|node| node.key.clone()).collect();
    for node in nodes.iter_mut().filter(|node| node.key.is_none()) {
        let path = match &node.parent {
            None => Some(Path::root(node.id.clone())),
            Some(parent) => parent.child(node.id.clone()).ok(),
        };
        let found = path.as_ref().and_then(|path| tree.key_at(path));
        if let Some(key) = found
            && taken.insert(key.clone())
        {
            node.key = Some(key.clone());
        }
    }
}

fn match_resources(nodes: &mut [Node<FileRefs>], base: &Document) {
    let mut taken: BTreeSet<AttachmentKey> = nodes
        .iter()
        .flat_map(|node| {
            node.resources
                .iter()
                .filter_map(|resource| resource.key.clone())
        })
        .collect();
    for node in nodes.iter_mut() {
        let Some(based) = node.key.as_ref().and_then(|key| base.nodes.get(key)) else {
            continue;
        };
        for resource in node
            .resources
            .iter_mut()
            .filter(|resource| resource.key.is_none())
        {
            let found = based.resources.iter().find(|candidate| {
                candidate.title == resource.title && !taken.contains(&candidate.key)
            });
            if let Some(found) = found {
                taken.insert(found.key.clone());
                resource.key = Some(found.key.clone());
            }
        }
    }
}

/// Gives each keyless item the key of the base item with its id, unless taken.
fn match_ids<T, K: Ord + Clone>(
    items: &mut [T],
    base: &BTreeMap<&Slug, &K>,
    slot: impl Fn(&mut T) -> (&Slug, &mut Option<K>),
) {
    let mut taken: BTreeSet<K> = items
        .iter_mut()
        .filter_map(|item| slot(item).1.clone())
        .collect();
    for item in items.iter_mut() {
        let (id, key) = slot(item);
        if key.is_none()
            && let Some(found) = base.get(id)
            && taken.insert((*found).clone())
        {
            *key = Some((*found).clone());
        }
    }
}
