//! A20: the notices a route write reports. An import lists those of the draft it opens and a
//! publish those of the version it creates, whether the patch carries the mutation or a
//! proposal it applies does; any other patch lists none. Computed from the graph each time,
//! never stored, and never a reason to reject (A15).
//!
//! Cost: one validation of the graph and one engine pass over it (the engine module states
//! it), for a patch that imports or publishes only.

use cairn_engine::{Applied, Graph, Records, notices, resolve};
use cairn_schema::{Deployment, DraftSource, Lineage, Mutation, Notice, Patch, PatchTarget};

/// The notices of the graph `applied` imports or publishes for `patch`, read against the
/// `before` records it was applied to: the one its last such mutation leaves, an imported
/// draft or the latest published version.
///
/// # Panics
///
/// When an accepted route graph fails validation, which the engine already checked.
pub(crate) fn of(patch: &Patch, before: &Records, applied: &Applied) -> Vec<Notice> {
    let PatchTarget::Route(route) = &patch.target else {
        return Vec::new();
    };
    // A proposal's own mutations run when it is applied, in the patch's place.
    let mutations: Vec<Mutation> = patch
        .mutations
        .as_slice()
        .iter()
        .flat_map(|mutation| match mutation {
            Mutation::ApplyProposal { proposal, .. } => before
                .proposals
                .get(proposal)
                .and_then(|held| resolve(&held.draft).ok())
                .unwrap_or_default(),
            other => vec![other.clone()],
        })
        .collect();
    let last = mutations.iter().rev().find(|mutation| {
        matches!(
            mutation,
            Mutation::OpenDraft {
                source: DraftSource::Import
            } | Mutation::PublishDraft {}
        )
    });
    let records = applied.records();
    let Some(held) = records.routes.get(route) else {
        return Vec::new();
    };
    let graph = match last {
        Some(Mutation::OpenDraft { .. }) => held.draft.as_ref().map(|draft| &draft.graph),
        Some(_) => held
            .versions
            .iter()
            .next_back()
            .and_then(|version| {
                records.versions.get(&Lineage {
                    route: route.clone(),
                    version: *version,
                })
            })
            .map(|version| &version.graph),
        None => None,
    };
    let Some(document) = graph else {
        return Vec::new();
    };
    // A route has no answers, so no deployment changes what it means.
    match Graph::new(document.clone(), &Deployment::default()) {
        Ok(checked) => notices(&checked),
        Err(violations) => panic!("an accepted route graph is valid: {violations:?}"),
    }
}
