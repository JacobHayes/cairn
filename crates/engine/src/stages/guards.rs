//! Transition guards that need no derived state (D4, A16, G2, B10): a completed deliverable
//! that requires an artifact has an artifact link, and a completed placeholder has children
//! or is atomic, both on the graph the patch produces, so a guard is never satisfied only for
//! a moment. A guard bypass applied in the same patch accepts the failures of the guards it
//! names, and those specific failures are recorded on it; a bypass covers a guarded
//! transition (complete, a first answer, reach) on its node in the same patch. Relevance and
//! `deps_done` join this stage with derive (2.4).

use std::collections::BTreeSet;

use cairn_schema::{GuardFailure, KeyRefs, Node, Payload, ViolationCode};

use super::Check;
use crate::graph::{Document, Tree};
use crate::validate::at_node;

/// The failures of the guards this stage owns, for a completed node.
fn failures(document: &Document, tree: &Tree, node: &Node<KeyRefs>) -> BTreeSet<GuardFailure> {
    let mut found = BTreeSet::new();
    let (placeholder, requires_artifact) = match &node.payload {
        Payload::Deliverable(deliverable) => {
            (deliverable.placeholder, deliverable.requires_artifact)
        }
        Payload::Action(action) => (action.placeholder, false),
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => (false, false),
    };
    let has_artifact = document.state.annotations.values().any(|annotation| {
        annotation.is_artifact() && annotation.body.node.as_ref() == Some(&node.key)
    });
    if requires_artifact && !has_artifact {
        found.insert(GuardFailure::MissingArtifact);
    }
    let atomic = document
        .state
        .nodes
        .get(&node.key)
        .is_some_and(|stored| stored.atomic);
    if placeholder && !atomic && !tree.is_container(&node.key) {
        found.insert(GuardFailure::NotBrokenDown);
    }
    // Only deliverables and actions have these guards (A16).
    assert!(
        found.is_empty() || matches!(node.payload, Payload::Deliverable(_) | Payload::Action(_))
    );
    assert!(found.len() <= 2);
    found
}

/// Runs the guard checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let Some(journey) = session.journey() else {
        return;
    };
    let document = &journey.graph;
    let tree = Tree::build(document);
    // Every completion the patch attempted, whatever the node's final state: a guard is
    // evaluated for the transition attempted, on the graph the patch produces (D4).
    for (key, ordinal) in &session.completed {
        let Some(node) = document.nodes.get(key) else {
            continue;
        };
        let covered: BTreeSet<_> = document
            .state
            .overrides
            .get(key)
            .and_then(|overrides| overrides.bypass.as_ref())
            .filter(|_| session.bypassed.contains_key(key))
            .map(|bypass| bypass.guards.clone())
            .unwrap_or_default();
        for failure in failures(document, &tree, node) {
            if covered.contains(&failure.guard()) {
                check
                    .bypassed
                    .entry(key.clone())
                    .or_default()
                    .insert(failure);
                continue;
            }
            let mut found = at_node(
                &tree,
                key,
                ViolationCode::GuardFailed,
                format!(
                    "completing this node fails {:?} (D4); bypass it with a reason to complete anyway",
                    failure.guard()
                ),
            );
            found.at.mutation = Some(*ordinal);
            found.bypassable = Some(failure.guard());
            found.failures.insert(failure);
            check.violations.push(found);
        }
    }
    for (key, ordinal) in &session.bypassed {
        if session.guarded.contains_key(key) {
            check.bypassed.entry(key.clone()).or_default();
            continue;
        }
        let mut found = at_node(
            &tree,
            key,
            ViolationCode::IllegalTransition,
            "a guard bypass covers a guarded transition of its node in the same patch (D4)",
        );
        found.at.mutation = Some(*ordinal);
        check.violations.push(found);
    }
}
