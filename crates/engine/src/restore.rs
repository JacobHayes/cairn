//! Restoring a tombstoned route node into a journey (B4): a local copy of the node and its
//! removed subtree under fresh keys, parents first. The copy is the route's node run through
//! [`Node::map_references`] with a translator that renames the restored nodes to their new
//! keys, keeps what the journey holds and drops what it does not, the same walker removal
//! uses, so a new reference field cannot compile without being carried here.
//!
//! Cost: one pass over the restored nodes plus one containment index of each graph.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use cairn_schema::{
    AttachmentKey, Graph, KeyAllocator, KeyRefs, KindKey, MapReferences, Mutation, Node, NodeField,
    NodeKey, ResourceContent, RoleKey, Slug, mint,
};

use crate::cascade::{Pruned, pruned_draft};
use crate::graph::Tree;

/// Fresh key bodies: the caller's random seed, then a count.
struct Seeded<'s> {
    seed: &'s str,
    count: u64,
}

impl KeyAllocator for Seeded<'_> {
    fn next_body(&mut self, _: &'static str) -> String {
        self.count += 1;
        format!("{}{}", self.seed, self.count)
    }
}

/// Renames the restored nodes, keeps what the journey holds, and drops the rest.
struct Restoring<'g, 's> {
    journey: &'g Graph,
    keys: BTreeMap<NodeKey, NodeKey>,
    fresh: Seeded<'s>,
    /// A message draft that overflowed once its placeholders took the new keys.
    overflowed: bool,
}

impl Restoring<'_, '_> {
    fn knows(&self, node: &NodeKey) -> bool {
        self.keys.contains_key(node) || self.journey.nodes.get(node).is_some()
    }
}

impl MapReferences<KeyRefs, KeyRefs> for Restoring<'_, '_> {
    fn node(&mut self, node: &NodeKey, _: Option<NodeField>) -> Option<NodeKey> {
        self.keys
            .get(node)
            .cloned()
            .or_else(|| self.journey.nodes.get(node).is_some().then(|| node.clone()))
    }

    fn role(&mut self, role: &RoleKey, _: Option<NodeField>) -> Option<RoleKey> {
        self.journey.roles.get(role).is_some().then(|| role.clone())
    }

    fn kind(&mut self, kind: &KindKey) -> Option<KindKey> {
        (*kind == KindKey::owner() || self.journey.participation_kinds.get(kind).is_some())
            .then(|| kind.clone())
    }

    fn own_key(&mut self, key: &NodeKey) -> NodeKey {
        self.keys.get(key).cloned().unwrap_or_else(|| key.clone())
    }

    fn resource_key(&mut self, _: &AttachmentKey) -> AttachmentKey {
        mint(&mut self.fresh)
    }

    fn draft_too_long(&mut self, _: &AttachmentKey, _: usize) {
        self.overflowed = true;
    }

    fn kinds_collapsed(&mut self, kind: &KindKey) {
        unreachable!("restoring maps each kind to itself: {kind}")
    }
}

/// The first of `base`, `base-2`, `base-3`, ... not in `taken`.
fn unique_id(base: &Slug, taken: &BTreeSet<Slug>) -> Slug {
    let mut candidate = base.clone();
    let mut suffix = 1_u64;
    while taken.contains(&candidate) {
        suffix += 1;
        // A base at the id limit has no room for a suffix: the patch is refused as a clash.
        match Slug::from_str(&format!("{}-{suffix}", base.as_str())) {
            Ok(next) => candidate = next,
            Err(_) => break,
        }
    }
    candidate
}

/// B4: the mutations that restore tombstoned `top` of `version` into `journey`: its node
/// and its tombstoned descendants as local copies under keys built from `seed`, parents
/// first. A parent the journey does not hold leaves a copy at the top level; a reference to
/// something the journey does not hold is dropped, a message draft keeping its other text.
/// None when `top` is not in `version`, or a message draft would pass its size limit under
/// the new keys (restoring would lose it).
#[must_use]
pub fn restore_nodes(
    journey: &Graph,
    version: &Graph,
    top: &NodeKey,
    seed: &str,
) -> Option<Vec<Mutation>> {
    version.nodes.get(top)?;
    let route = Tree::build(version);
    let held = Tree::build(journey);
    let nodes: Vec<NodeKey> = std::iter::once(top.clone())
        .chain(
            route
                .descendants(top)
                .into_iter()
                .filter(|key| journey.state.tombstones.contains(key)),
        )
        .collect();
    let mut restoring = Restoring {
        journey,
        keys: BTreeMap::new(),
        fresh: Seeded { seed, count: 0 },
        overflowed: false,
    };
    for key in &nodes {
        let renamed = mint(&mut restoring.fresh);
        restoring.keys.insert(key.clone(), renamed);
    }
    let mut taken: BTreeMap<Option<NodeKey>, BTreeSet<Slug>> = BTreeMap::new();
    let mut out = Vec::new();
    for key in &nodes {
        let node = version.nodes.get(key)?;
        let copy = prune(node, &restoring);
        let mut restored = copy.map_references(&mut restoring);
        let siblings = taken.entry(restored.parent.clone()).or_insert_with(|| {
            let children = restored
                .parent
                .as_ref()
                .map_or_else(|| held.roots(), |parent| held.children(parent));
            children
                .iter()
                .filter_map(|child| journey.nodes.get(child).map(|node| node.id.clone()))
                .collect()
        });
        restored.id = unique_id(&node.id, siblings);
        siblings.insert(restored.id.clone());
        out.push(Mutation::AddNode { node: restored });
    }
    (!restoring.overflowed).then_some(out)
}

/// The node without the condition clauses and draft placeholders that name a decision or
/// role the journey will not hold, which the walker would otherwise drop whole.
fn prune(node: &Node<KeyRefs>, restoring: &Restoring) -> Node<KeyRefs> {
    let mut copy = node.clone();
    copy.relevant_when = node
        .relevant_when
        .as_ref()
        .and_then(|condition| condition.without_decisions(|decision| !restoring.knows(decision)));
    let mut kept = Vec::new();
    for resource in std::mem::take(&mut copy.resources) {
        let ResourceContent::MessageDraft(template) = &resource.content else {
            kept.push(resource);
            continue;
        };
        let gone = |segment: &cairn_schema::Segment<KeyRefs>| match segment {
            cairn_schema::Segment::Answer(decision) => !restoring.knows(decision),
            cairn_schema::Segment::RoleName(role) => restoring.journey.roles.get(role).is_none(),
            cairn_schema::Segment::Text(_) | cairn_schema::Segment::Journey(_) => false,
        };
        match pruned_draft(template, gone) {
            Pruned::Unchanged => kept.push(resource),
            Pruned::To(template) => {
                let mut next = resource.clone();
                next.content = ResourceContent::MessageDraft(template);
                kept.push(next);
            }
            Pruned::Emptied => {}
        }
    }
    copy.resources = kept;
    copy
}
