//! Transition guards that need no derived state (D4, A16, G2, G4, G5, B10): a completed deliverable
//! that requires an artifact has an artifact link, a completed deliverable or action that
//! requires a note has a note of its own, one that requires its checklist has every item
//! ticked, and a completed placeholder has children or is
//! atomic, each on the graph the patch produces, so a guard is never satisfied only for
//! a moment. A guard bypass applied in the same patch accepts the failures of the guards it
//! names, and those specific failures are recorded on it; a bypass covers a guarded
//! transition (complete, a first answer) on its node in the same patch. Relevance and
//! `deps_done` join this stage with derive (2.4).

use std::collections::BTreeSet;

use cairn_schema::{GuardFailure, ViolationCode};

use super::Check;
use crate::derive::stale::{annotated_nodes, static_failures};
use crate::graph::Tree;
use crate::validate::at_node;

/// Runs the guard checks.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    let Some(journey) = session.journey() else {
        return;
    };
    let document = &journey.graph;
    let tree = Tree::build(document);
    let annotated = annotated_nodes(document);
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
        for failure in static_failures(document, &tree, node, &annotated) {
            if covered.contains(&failure.guard()) {
                check
                    .bypassed
                    .entry(key.clone())
                    .or_default()
                    .insert(failure);
                continue;
            }
            let message = match &failure {
                GuardFailure::Unticked(item) => {
                    let title = node
                        .payload
                        .checklist()
                        .and_then(|checklist| checklist.title(item))
                        .map_or_else(|| item.to_string(), ToString::to_string);
                    format!(
                        "completing this node needs its checklist item \"{title}\" ticked; tick it in the same patch, or bypass it with a reason to complete anyway"
                    )
                }
                _ => format!(
                    "completing this node fails {:?}; bypass it with a reason to complete anyway",
                    failure.guard()
                ),
            };
            let mut found = at_node(&tree, key, ViolationCode::GuardFailed, message);
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
            "a guard bypass covers a guarded transition of its node in the same patch",
        );
        found.at.mutation = Some(*ordinal);
        check.violations.push(found);
    }
}
