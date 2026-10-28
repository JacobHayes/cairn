//! Transition guards that need no derived state (D4, A16, G2, G4, G5, B10): the evidence a
//! deliverable's or action's completion needs. A completed deliverable that requires an
//! artifact has an artifact link, a completed deliverable or action that requires a note has a
//! note of its own, one that requires its checklist has every item ticked, and a completed
//! placeholder has children or is atomic, each on the graph the patch produces, so a guard is
//! never satisfied only for a moment. A completion that goes ahead anyway accepts the
//! failures present, and those specific failures are recorded on its bypass. Order is never
//! a guard.

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
        let going_ahead = session.bypassed.contains_key(key);
        for failure in static_failures(document, &tree, node, &annotated) {
            if going_ahead {
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
                        "completing this node needs its checklist item \"{title}\" ticked; tick it in the same patch, or complete it anyway (transition complete_anyway)"
                    )
                }
                _ => format!(
                    "completing this node fails the {} guard; complete it anyway (transition complete_anyway) to go ahead without it",
                    cairn_schema::wire_name(&failure.guard())
                ),
            };
            let mut found = at_node(&tree, key, ViolationCode::GuardFailed, message);
            found.at.mutation = Some(*ordinal);
            found.failures.insert(failure);
            check.violations.push(found);
        }
    }
}
