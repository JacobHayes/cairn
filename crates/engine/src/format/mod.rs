//! The route file format (A13, A14; ARCHITECTURE, File format): import against the version
//! a file extends, deterministic export, and the mutations that write an imported graph into
//! a route's draft.

mod export;
mod import;
mod matching;

pub use export::{RouteHeading, export};
pub use import::{from_file, import};

use cairn_schema::Mutation;

use crate::graph::{Document, Tree};

/// A13, B8: the mutations that build a graph's structure in an empty route draft (one opened by an import or
/// a saved journey): its roles and kinds by id, its default owner, and its nodes by path,
/// each with its edges, participations, and resources. At most 2,000 nodes, 64 roles and
/// kinds, and one default owner: within the mutation limit.
#[must_use]
pub fn content(document: &Document) -> Vec<Mutation> {
    let tree = Tree::build(document);
    let mut roles: Vec<_> = document.roles.values().collect();
    roles.sort_by(|left, right| left.id.cmp(&right.id));
    let mut kinds: Vec<_> = document.participation_kinds.values().collect();
    kinds.sort_by(|left, right| left.id.cmp(&right.id));
    let mut nodes: Vec<_> = document.nodes.values().collect();
    nodes.sort_by_key(|node| tree.path(&node.key));
    let mut mutations: Vec<Mutation> = roles
        .into_iter()
        .map(|role| Mutation::AddRole { role: role.clone() })
        .collect();
    mutations.extend(
        kinds
            .into_iter()
            .map(|kind| Mutation::AddParticipationKind { kind: kind.clone() }),
    );
    mutations.extend(
        document
            .default_owner
            .clone()
            .map(|role| Mutation::SetDefaultOwner { role: Some(role) }),
    );
    mutations.extend(
        nodes
            .into_iter()
            .map(|node| Mutation::AddNode { node: node.clone() }),
    );
    mutations
}
