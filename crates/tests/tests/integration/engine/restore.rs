//! Restoring a tombstoned node (B4): the copy is refused whole when a message draft would
//! pass its size limit once its placeholders take the fresh keys, rather than losing it.

#![cfg(test)]

use crate::engine::support;

use cairn_engine::restore_nodes;
use cairn_schema::{Limit, Mutation, NodeKey, NodeKind, Resource, ResourceContent, from_json};

/// A route version and a journey graph that tombstoned a decision of it, which holds a
/// message draft of `fill` bytes of text and a placeholder naming the decision itself.
fn tombstoned_decision(fill: usize) -> (cairn_schema::Graph, cairn_schema::Graph, NodeKey) {
    let version = support::route_graph("vendor-evaluation").into_document();
    let key = version
        .nodes
        .values()
        .find(|node| {
            node.kind() == NodeKind::Decision
                && version
                    .nodes
                    .values()
                    .all(|other| other.parent.as_ref() != Some(&node.key))
        })
        .unwrap()
        .key
        .clone();
    let mut version = version;
    let mut node = version.nodes.get(&key).unwrap().clone();
    let text = format!("{}{{{{answers.{key}}}}}", "x".repeat(fill));
    node.resources.push(Resource {
        key: from_json("\"a_restore_draft\"").unwrap(),
        title: None,
        content: ResourceContent::MessageDraft(text.parse().unwrap()),
    });
    version.nodes.put(node).unwrap();
    let mut journey = version.clone();
    journey.state.tombstones.insert(key.clone());
    (version, journey, key)
}

fn added(mutations: &[Mutation]) -> usize {
    mutations
        .iter()
        .filter(|each| matches!(each, Mutation::AddNode { .. }))
        .count()
}

#[test]
fn a_restore_that_would_lose_a_draft_to_the_size_limit_is_refused() {
    let room = Limit::BodyBytes.max() as usize - 64;
    let (version, journey, key) = tombstoned_decision(room);
    // Short fresh keys leave room: the draft is kept, in the restored copy.
    let short = restore_nodes(&journey, &version, &key, "s").unwrap();
    assert_eq!(added(&short), 1);
    // Long fresh keys push the same draft past the limit: nothing is returned.
    assert_eq!(
        restore_nodes(&journey, &version, &key, &"s".repeat(60)),
        None
    );
}
