//! Previewing a proposal (C14, I6; ARCHITECTURE, Write path: proposal previews): what its
//! resolved mutations would do to its destination now, for the review screen's diff over a
//! journey, a route's draft, the deployment, or a destination the proposal creates at
//! revision 0. Items still unresolved are left out and listed; the candidate goes through the
//! same apply as the real thing, so every violation strict validation finds is listed; for a
//! journey, the frontier after and the consequences (D7) come from one derive on each side.
//!
//! Cost: one apply (one clone of the loaded records, the mutations, one validation) and, for a
//! journey, two derives and the consequences between them; for a route, one more validation
//! and the notices pass (A20).

use cairn_schema::{
    Deployment, DeriveInputs, Domain, Mutations, Patch, PatchTarget, ProposalDraft,
    ProposalPreview, Rejection,
};

use super::resolve_partial;
use crate::derive::{consequences, derive};
use crate::graph::Graph;
use crate::notices::notices;
use crate::pipeline::{ApplyInputs, apply};
use crate::records::Records;

/// C14: what `draft` would do to `destination` if applied now. `records` holds what the
/// proposal's apply would load; `derive_inputs` give today and the deployment for the
/// journey's two derives.
#[must_use]
pub fn preview(
    records: &Records,
    destination: &Domain,
    draft: &ProposalDraft,
    inputs: &ApplyInputs,
    derive_inputs: &DeriveInputs,
) -> ProposalPreview {
    let resolved = resolve_partial(draft);
    let mut preview = ProposalPreview {
        unresolved: resolved.unresolved,
        violations: Vec::new(),
        graph: None,
        frontier: Vec::new(),
        consequences: None,
        notices: Vec::new(),
    };
    let after = match Mutations::new(resolved.mutations) {
        // Nothing to apply: the destination as it stands.
        Err(_) => records.clone(),
        Ok(mutations) => {
            let patch = Patch {
                id: match "p_preview".parse() {
                    Ok(id) => id,
                    Err(error) => unreachable!("a fixed patch id parses: {error}"),
                },
                target: match destination {
                    Domain::Journey(id) => PatchTarget::Journey(id.clone()),
                    Domain::Route(id) => PatchTarget::Route(id.clone()),
                    Domain::Deployment => PatchTarget::Deployment,
                },
                base_revision: records.revision(destination),
                deployment_revision: None,
                mutations,
            };
            match apply(records, &patch, inputs) {
                Ok(applied) => applied.records().clone(),
                Err(Rejection::Invalid { violations }) => {
                    preview.violations = violations.as_slice().to_vec();
                    return preview;
                }
                Err(Rejection::Stale { .. } | Rejection::PatchIdReused { .. }) => {
                    unreachable!("a preview applies at the destination's current revision")
                }
            }
        }
    };
    match destination {
        Domain::Journey(id) => journey_after(&mut preview, records, &after, id, derive_inputs),
        Domain::Route(id) => {
            preview.graph = after
                .routes
                .get(id)
                .and_then(|route| route.draft.as_ref())
                .map(|draft| draft.graph.clone());
            // A20: a route has no answers, so no deployment changes what it means.
            preview.notices = preview.graph.as_ref().map_or_else(Vec::new, |document| {
                match Graph::new(document.clone(), &Deployment::default()) {
                    Ok(graph) => notices(&graph),
                    Err(_) => unreachable!("an applied route draft is valid"),
                }
            });
        }
        Domain::Deployment => {}
    }
    preview
}

/// D7: the journey after, its frontier, and what changed in derived state.
fn journey_after(
    preview: &mut ProposalPreview,
    records: &Records,
    after: &Records,
    id: &cairn_schema::JourneyId,
    derive_inputs: &DeriveInputs,
) {
    let Some(journey) = after.journeys.get(id) else {
        return;
    };
    let derive_after = DeriveInputs {
        deployment: after.deployment.clone(),
        ..derive_inputs.clone()
    };
    let Ok(graph) = Graph::new(journey.graph.clone(), &after.deployment) else {
        unreachable!("an applied journey is valid")
    };
    let derived = derive(&graph, Some(journey.header.created_on), &derive_after);
    preview.frontier = derived.ranking().frontier().to_vec();
    let before = records.journeys.get(id).and_then(|held| {
        let graph = Graph::new(held.graph.clone(), &records.deployment).ok()?;
        let derived = derive(&graph, Some(held.header.created_on), derive_inputs);
        Some((graph, derived))
    });
    if let Some((before_graph, before)) = before {
        preview.consequences = Some(consequences(&before_graph, &before, &graph, &derived));
    }
    preview.graph = Some(journey.graph.clone());
}
