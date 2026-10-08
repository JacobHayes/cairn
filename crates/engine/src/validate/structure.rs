//! The tree, ids, keys, and limits (PRD Invariants: graph, first, second, and last bullets;
//! PRACTICES, Explicit limits). Cost at the limits: one pass over nodes, their edges, and
//! their resources, plus one serialization of the graph to measure it (at most 16 MiB).

use std::collections::BTreeMap;

use cairn_schema::limits::EDGE_COUNT_PER_NODE_MAX;
use cairn_schema::{AttachmentKey, KindKey, Limit, NodeKey, Subject, Violation, ViolationCode};

use super::{GraphCheck, at_node, missing_node_code, violation};
use crate::graph::TreeIssue;

/// Runs the structural checks.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    tree_issues(check, out);
    retired_keys(check, out);
    role_and_kind_ids(check, out);
    edge_counts(check, out);
    resource_keys(check, out);
    resource_counts(check, out);
    graph_size(check, out);
}

fn tree_issues(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let tree = check.tree;
    for issue in tree.issues() {
        let found = match issue {
            TreeIssue::MissingParent { node, parent } => {
                let mut found = at_node(
                    tree,
                    node,
                    missing_node_code(check.document, parent),
                    format!("the parent {parent} does not exist"),
                );
                found.at.field = Some(cairn_schema::NodeField::Parent);
                found.related.push(Subject::Node(parent.clone()));
                found
            }
            TreeIssue::LeafParent { node, parent } => {
                let mut found = at_node(
                    tree,
                    parent,
                    ViolationCode::LeafWithChildren,
                    "a decision or milestone has no children (A2)",
                );
                found.related.push(Subject::Node(node.clone()));
                found
            }
            TreeIssue::Cycle { members } => {
                let mut found = violation(
                    ViolationCode::ContainmentCycle,
                    "these nodes contain each other; containment is a tree",
                );
                found.at.subject = members.first().cloned().map(Subject::Node);
                found.related = members.iter().cloned().map(Subject::Node).collect();
                found
            }
            TreeIssue::TooDeep { node } => {
                let mut found = at_node(
                    tree,
                    node,
                    ViolationCode::LimitExceeded,
                    "the node is nested deeper than containment_depth_max",
                );
                found.limit = Some(Limit::ContainmentDepth);
                found
            }
            TreeIssue::DuplicateId { first, second } => {
                let mut found = at_node(
                    tree,
                    second,
                    ViolationCode::DuplicateSiblingId,
                    "two siblings share this id, so they share a path",
                );
                found.related.push(Subject::Node(first.clone()));
                found
            }
        };
        out.push(found);
    }
}

/// PRD Invariants: a graph remembers every key it retired, so no key comes back.
fn retired_keys(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let retired = &document.retired_keys;
    for key in document.nodes.as_map().keys() {
        if retired.nodes.contains(key) {
            out.push(at_node(
                check.tree,
                key,
                ViolationCode::RetiredKeyReused,
                format!("the key {key} was removed from this graph and cannot come back"),
            ));
        }
    }
    for key in document.roles.as_map().keys() {
        if retired.roles.contains(key) {
            let mut found = violation(
                ViolationCode::RetiredKeyReused,
                format!("the role key {key} was removed from this graph and cannot come back"),
            );
            found.at.subject = Some(Subject::Role(key.clone()));
            out.push(found);
        }
    }
    for key in document.participation_kinds.as_map().keys() {
        if retired.kinds.contains(key) {
            let mut found = violation(
                ViolationCode::RetiredKeyReused,
                format!("the kind key {key} was removed from this graph and cannot come back"),
            );
            found.at.subject = Some(Subject::Kind(key.clone()));
            out.push(found);
        }
    }
}

/// PRD Identity and references: role and kind ids are unique in the graph, and `owner` is
/// the built-in kind's id and key.
fn role_and_kind_ids(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let mut role_ids = BTreeMap::new();
    for role in document.roles.values() {
        if let Some(first) = role_ids.insert(role.id.clone(), role.key.clone()) {
            let mut found = violation(
                ViolationCode::DuplicateSiblingId,
                format!("two roles share the id {}", role.id),
            );
            found.at.subject = Some(Subject::Role(role.key.clone()));
            found.related.push(Subject::Role(first));
            out.push(found);
        }
    }
    let owner = KindKey::owner();
    let mut kind_ids = BTreeMap::from([(cairn_schema::id::owner_kind_id(), owner.clone())]);
    for kind in document.participation_kinds.values() {
        let first = kind_ids.insert(kind.id.clone(), kind.key.clone());
        if let Some(first) = first.or_else(|| (kind.key == owner).then(|| owner.clone())) {
            let mut found = violation(
                ViolationCode::DuplicateSiblingId,
                format!(
                    "the kind {} repeats an id or key in use, or the built-in owner",
                    kind.id
                ),
            );
            found.at.subject = Some(Subject::Kind(kind.key.clone()));
            found.related.push(Subject::Kind(first));
            out.push(found);
        }
    }
}

/// PRACTICES, Explicit limits: at most 64 explicit edges per node, in plus out.
fn edge_counts(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let mut counts: BTreeMap<&NodeKey, u32> = BTreeMap::new();
    for node in document.nodes.values() {
        for requirement in node.requires.iter() {
            *counts.entry(&node.key).or_default() += 1;
            *counts.entry(requirement).or_default() += 1;
        }
    }
    for (key, count) in counts {
        if count > EDGE_COUNT_PER_NODE_MAX && document.nodes.get(key).is_some() {
            let mut found = at_node(
                check.tree,
                key,
                ViolationCode::LimitExceeded,
                format!(
                    "{count} explicit edges in and out; the limit is {EDGE_COUNT_PER_NODE_MAX}"
                ),
            );
            found.limit = Some(Limit::EdgeCountPerNode);
            out.push(found);
        }
    }
}

/// Resources are keyed so upgrades diff them (B7): a key names one resource in the graph.
fn resource_keys(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let mut seen: BTreeMap<&AttachmentKey, &NodeKey> = BTreeMap::new();
    for node in check.document.nodes.values() {
        for resource in &node.resources {
            if let Some(first) = seen.insert(&resource.key, &node.key) {
                let mut found = at_node(
                    check.tree,
                    &node.key,
                    ViolationCode::DuplicateKey,
                    format!("the resource key {} is used twice", resource.key),
                );
                found.related.push(Subject::Node(first.clone()));
                out.push(found);
            }
        }
    }
}

/// PRACTICES, Explicit limits: at most `resource_count_per_node_max` resources per node,
/// however they arrived (added one at a time, imported, or merged in by an upgrade).
fn resource_counts(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    for node in check.document.nodes.values() {
        if let Err(exceeded) = Limit::ResourceCountPerNode.check(node.resources.len()) {
            let mut found = at_node(
                check.tree,
                &node.key,
                ViolationCode::LimitExceeded,
                format!(
                    "{} resources, past {}",
                    exceeded.count,
                    exceeded.limit.name()
                ),
            );
            found.limit = Some(exceeded.limit);
            out.push(found);
        }
    }
}

/// PRACTICES, Explicit limits: a graph, serialized with its state, is at most 16 MiB.
fn graph_size(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let bytes = match cairn_schema::to_json(check.document) {
        Ok(json) => json.len(),
        Err(error) => panic!("a graph always serializes: {error}"),
    };
    if Limit::GraphBytes.check(bytes).is_err() {
        let mut found = violation(
            ViolationCode::LimitExceeded,
            format!("the graph serializes to {bytes} bytes, past graph_bytes_max"),
        );
        found.limit = Some(Limit::GraphBytes);
        out.push(found);
    }
}
