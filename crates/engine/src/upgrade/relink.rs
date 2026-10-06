//! Re-link (B9): after a saved route is published, the journey it came from links to that
//! version. Its nodes are matched by key; each difference between a matched node and the
//! version's is kept as a local edit unless the reviewer takes the route's value. The old
//! lineage holds until apply, where the re-link mutation sets the new lineage, the matched
//! nodes' provenance, and exactly the markers their differences call for, in one event.
//!
//! Cost: one pass over the journey's nodes, each compared with its version node field by
//! field (22 fields, its edges, participations, and resources): O(n x (22 + 64 + 33)).

use std::collections::BTreeSet;

use cairn_schema::{
    Conflict, ConflictResolution, Edge, JourneyId, KeyRefs, Lineage, LocalEdit, Mutation, Node,
    NodeFieldValue, ProposalDraft, ReviewItem,
};

use super::{DraftError, draft};
use crate::edit::differences;
use crate::records::Records;

/// B9: the proposal that links a journey to `lineage`, a version its saved route published:
/// the re-link mutation, and one conflict per difference on a matched node, kept by default.
///
/// # Errors
///
/// When the journey or the version is not loaded, or the draft passes a proposal's limits.
pub fn relink(
    records: &Records,
    journey: &JourneyId,
    lineage: &Lineage,
) -> Result<ProposalDraft, DraftError> {
    let held = records
        .journeys
        .get(journey)
        .ok_or_else(|| DraftError::JourneyMissing(journey.clone()))?;
    let version = records
        .versions
        .get(lineage)
        .ok_or_else(|| DraftError::VersionMissing(lineage.clone()))?;
    let mut items = Vec::new();
    for node in held.graph.nodes.values() {
        if let Some(routed) = version.graph.nodes.get(&node.key) {
            let answered = held.graph.state.answers.contains_key(&node.key);
            items.extend(
                node_conflicts(node, routed, answered)
                    .into_iter()
                    .map(|conflict| ReviewItem::Conflict {
                        conflict,
                        resolution: Some(ConflictResolution::KeepJourney),
                    }),
            );
        }
    }
    let title = format!("Link to version {} of {}", lineage.version, lineage.route);
    draft(
        &title,
        held.revision,
        vec![Mutation::Relink {
            lineage: lineage.clone(),
        }],
        items,
    )
}

/// B9: every difference between a journey node and its route node, as conflicts; a node
/// whose kind or answer type differs is one conflict, whole.
pub(crate) fn node_conflicts(
    node: &Node<KeyRefs>,
    routed: &Node<KeyRefs>,
    answered: bool,
) -> Vec<Conflict> {
    let edits: BTreeSet<LocalEdit> = differences(node, routed);
    if edits.contains(&LocalEdit::Shape) {
        return vec![Conflict::Shape {
            journey: Box::new(node.clone()),
            route: Box::new(routed.clone()),
            answered,
            dangling: false,
        }];
    }
    edits
        .into_iter()
        .filter_map(|edit| match edit {
            LocalEdit::Field(field) => {
                match (
                    NodeFieldValue::read(field, node),
                    NodeFieldValue::read(field, routed),
                ) {
                    (Some(journey), Some(route)) => Some(Conflict::Field {
                        node: node.key.clone(),
                        journey,
                        route,
                        dangling: false,
                    }),
                    _ => None,
                }
            }
            LocalEdit::Requires(requirement) => Some(Conflict::Edge {
                journey: node.requires.as_set().contains(&requirement),
                route: routed.requires.as_set().contains(&requirement),
                edge: Edge {
                    node: node.key.clone(),
                    requires: requirement,
                },
            }),
            LocalEdit::Participation(kind) => Some(Conflict::Participation {
                node: node.key.clone(),
                journey: node.participations.as_map().get(&kind).cloned(),
                route: routed.participations.as_map().get(&kind).cloned(),
                kind,
            }),
            LocalEdit::Resource(key) => Some(Conflict::Resource {
                node: node.key.clone(),
                journey: node
                    .resources
                    .iter()
                    .find(|found| found.key == key)
                    .cloned(),
                route: routed
                    .resources
                    .iter()
                    .find(|found| found.key == key)
                    .cloned(),
                resource: key,
                dangling: false,
            }),
            LocalEdit::Shape => None,
        })
        .collect()
}
