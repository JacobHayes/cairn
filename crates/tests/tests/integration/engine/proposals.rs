//! Proposal documents (C14, I6; B8): review items resolve into ordinary mutations, and only
//! an item that still needs a choice blocks apply.
#![cfg(test)]

use crate::engine::support;

use cairn_engine::{resolve, resolve_partial};
use cairn_schema::{Mutation, ParticipationSource, UnresolvedReason};

/// A save-as-route draft: a group with a child and a third node requiring the child, the
/// group's owner an explicit identity.
fn saved(items: &str) -> String {
    format!(
        "title: Save\ndestination_revision: 0\nmutations:\n\
         - op: create_route\n  name: Saved\n\
         - op: open_draft\n  source: {{save_as_route: {{journey: j_vendor_eval}}}}\n\
         - op: add_node\n  node: {{key: n_group, id: group, kind: group, title: Group, participations: {{k_owner: [who_k3d7qm2xw5tn6pfa, who_other]}}}}\n\
         - op: add_node\n  node: {{key: n_child, id: child, parent: n_group, kind: action, title: Child}}\n\
         - op: add_node\n  node: {{key: n_after, id: after, kind: action, title: After, requires: [n_child]}}\n\
         items:\n{items}"
    )
}

fn added(mutations: &[Mutation]) -> Vec<&cairn_schema::Node<cairn_schema::KeyRefs>> {
    mutations
        .iter()
        .filter_map(|mutation| match mutation {
            Mutation::AddNode { node } => Some(node),
            _ => None,
        })
        .collect()
}

/// B8: an excluded node goes with its subtree and the edges into it; explicit identities
/// mapped to one role become that role, and a new role is added.
#[test]
fn exclusions_and_mappings_rewrite_the_saved_structure() {
    let items = "\
- item: participation\n  identity: who_k3d7qm2xw5tn6pfa\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {new_role: {key: r_lead, id: lead}}\n\
- item: participation\n  identity: who_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: drop\n\
- item: exclusion\n  node: n_group\n  excluded: false\n\
- item: exclusion\n  node: n_child\n  excluded: true\n";
    let mutations = resolve(&support::draft(&saved(items))).unwrap();
    let nodes = added(&mutations);
    let keys: Vec<&str> = nodes.iter().map(|node| node.key.as_str()).collect();
    assert_eq!(keys, ["n_group", "n_after"]);
    assert!(
        nodes[1].requires.is_empty(),
        "the edge into the excluded child goes"
    );
    let owner = nodes[0].participations.as_map().values().next().unwrap();
    assert_eq!(*owner, ParticipationSource::Role("r_lead".parse().unwrap()));
    assert!(mutations.iter().any(
        |mutation| matches!(mutation, Mutation::AddRole { role } if role.key.as_str() == "r_lead")
    ));
}

/// B8: identities on one participation mapped to different roles block both items; one mapped
/// to the default owner of a graph with none blocks.
#[test]
fn mixed_mappings_and_a_missing_default_owner_block() {
    let mixed = "\
- item: participation\n  identity: who_k3d7qm2xw5tn6pfa\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {role: r_one}\n\
- item: participation\n  identity: who_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: {role: r_two}\n";
    let blocked = resolve_partial(&support::draft(&saved(mixed))).unwrap_or_default_unresolved();
    assert_eq!(
        blocked,
        [
            (0, UnresolvedReason::MixedMapping),
            (1, UnresolvedReason::MixedMapping)
        ]
    );
    let owner = "\
- item: participation\n  identity: who_k3d7qm2xw5tn6pfa\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: default_owner\n\
- item: participation\n  identity: who_other\n  uses: [{node: n_group, kind: k_owner}]\n  mapping: drop\n";
    let blocked = resolve_partial(&support::draft(&saved(owner))).unwrap_or_default_unresolved();
    assert_eq!(blocked, [(0, UnresolvedReason::NoDefaultOwner)]);
}

trait Blocked {
    fn unwrap_or_default_unresolved(self) -> Vec<(u32, UnresolvedReason)>;
}

impl Blocked for cairn_engine::proposal::Resolved {
    fn unwrap_or_default_unresolved(self) -> Vec<(u32, UnresolvedReason)> {
        self.unresolved
            .into_iter()
            .map(|item| (item.item, item.reason))
            .collect()
    }
}
