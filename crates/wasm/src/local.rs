//! Local writes over a domain document (ARCHITECTURE, Web UI: previews; Write path): a draft
//! patch applied to the browser's in-memory journey with the engine's `apply`, a proposal
//! previewed (C14), and a patch's touched set for the H5 safe retry. Nothing here commits:
//! committed state always comes from the server. Each mirrors what the server's service does
//! with the same records, so its answer is the server's byte for byte.
//!
//! Cost: an apply is one engine apply and, for its consequences, two derives; a preview is
//! the engine's (one apply, two derives); a touched set is linear in the patch.

use std::collections::BTreeSet;

use cairn_engine::{ApplyInputs, Graph, Records, consequences, derive};
use cairn_schema::{
    Actor, Consequences, Domain, DomainDocument, Markdown, Patch, PatchTarget, Proposal,
    RouteVersion, Timestamp, TouchedSet,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::derivation::read_document;
use crate::error::{HostError, json, read};

/// A draft patch to apply locally, with the clock and actor its events would carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    /// The patch; it targets the document's journey.
    pub patch: Patch,
    /// A note for each of its events (J1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Markdown>,
    /// When it would commit.
    pub at: Timestamp,
    /// Who would submit it (H2).
    pub actor: Actor,
}

/// A patch applied locally: the document as it would leave it and what it would newly cause.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedLocally {
    /// The document after: the journey as the patch leaves it, with the same derive inputs
    /// (the deployment as the patch leaves it, when it creates entities).
    pub document: DomainDocument,
    /// D7: what the patch newly causes in the journey, as the server's patch answer reports
    /// it (empty when nothing).
    pub consequences: Consequences,
}

/// A proposal to preview against the document's journey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    /// The proposal, as `GET /proposals/{id}` answers it; its destination is the journey.
    pub proposal: Proposal,
    /// The route versions its mutations read (an upgrade's target and the journey's own),
    /// as `GET /routes/{id}/versions/{version}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<RouteVersion>,
    /// When the preview is asked for.
    pub at: Timestamp,
    /// Who asks (H2).
    pub actor: Actor,
}

/// The records a patch to the document's journey reads: the journey and the deployment.
fn records_of(document: &DomainDocument) -> Records {
    let mut records = Records {
        deployment: document.inputs.deployment.clone(),
        ..Records::default()
    };
    records
        .journeys
        .insert(document.journey.header.id.clone(), document.journey.clone());
    records
}

/// The journey of `document` targeted, or the mismatch.
fn own_journey(document: &DomainDocument, target: &Domain) -> Result<(), HostError> {
    if *target == Domain::Journey(document.journey.header.id.clone()) {
        Ok(())
    } else {
        Err(HostError::Missing {
            message: format!(
                "the document holds journey {}, not {target:?}",
                document.journey.header.id
            ),
        })
    }
}

/// D7, as the server's service works it out for a patch answer: the journey before and after,
/// each derived at the document's today with no viewer, over its own deployment.
fn caused(before: &DomainDocument, after: &DomainDocument) -> Consequences {
    let side = |document: &DomainDocument| {
        let deployment = &document.inputs.deployment;
        let graph = match Graph::new(document.journey.graph.clone(), deployment) {
            Ok(graph) => graph,
            Err(violations) => panic!("an accepted journey is valid: {violations:?}"),
        };
        let mut inputs = document.inputs.clone();
        inputs.viewer = BTreeSet::new();
        let derived = derive(&graph, Some(document.journey.header.created_on), &inputs);
        (graph, derived)
    };
    let (before_graph, before_derived) = side(before);
    let (after_graph, after_derived) = side(after);
    consequences(&before_graph, &before_derived, &after_graph, &after_derived)
}

/// A17 locally: `request`'s patch applied to `document`'s journey at the document's today.
///
/// # Errors
///
/// [`HostError::Rejected`] with the engine's rejection; [`HostError::Missing`] when the patch
/// targets another domain.
pub fn apply_locally(
    document: &DomainDocument,
    request: &ApplyRequest,
) -> Result<AppliedLocally, HostError> {
    // Only the journey's own patches: a proposal's lifecycle and other domains are the
    // server's (or the in-browser root's) to apply.
    let target = match &request.patch.target {
        PatchTarget::Journey(id) => Domain::Journey(id.clone()),
        PatchTarget::Route(id) => Domain::Route(id.clone()),
        PatchTarget::Deployment | PatchTarget::Proposal { .. } => Domain::Deployment,
    };
    own_journey(document, &target)?;
    let inputs = ApplyInputs {
        today: document.inputs.today,
        at: request.at,
        actor: request.actor.clone(),
        note: request.note.clone(),
    };
    let applied = cairn_engine::apply(&records_of(document), &request.patch, &inputs)
        .map_err(|rejection| HostError::Rejected { rejection })?;
    let records = applied.records();
    let journey = &document.journey.header.id;
    let mut after = document.clone();
    after.journey = match records.journeys.get(journey) {
        Some(found) => found.clone(),
        None => unreachable!("an accepted journey patch leaves its journey"),
    };
    after.inputs.deployment = records.deployment.clone();
    let consequences = caused(document, &after);
    Ok(AppliedLocally {
        document: after,
        consequences,
    })
}

/// C14 locally: what applying `request`'s proposal to the document's journey would do now,
/// as the server's preview derives it (no viewer, the document's today).
///
/// # Errors
///
/// [`HostError::Missing`] when the proposal's destination is not the document's journey.
pub fn preview_locally(
    document: &DomainDocument,
    request: &PreviewRequest,
) -> Result<cairn_schema::ProposalPreview, HostError> {
    let proposal = &request.proposal;
    own_journey(document, &proposal.destination)?;
    let mut records = records_of(document);
    records
        .proposals
        .insert(proposal.id.clone(), proposal.clone());
    for version in &request.versions {
        records.versions.insert(
            cairn_schema::Lineage {
                route: version.route.clone(),
                version: version.version,
            },
            version.clone(),
        );
    }
    let inputs = ApplyInputs {
        today: document.inputs.today,
        at: request.at,
        actor: request.actor.clone(),
        note: None,
    };
    let mut derive_inputs = document.inputs.clone();
    derive_inputs.viewer = BTreeSet::new();
    Ok(cairn_engine::preview(
        &records,
        &proposal.destination,
        &proposal.draft,
        &inputs,
        &derive_inputs,
    ))
}

/// Applies a draft patch to `document`'s journey without committing it: `request` is the JSON
/// of an [`ApplyRequest`]; the answer is the JSON of an [`AppliedLocally`].
///
/// # Errors
///
/// The JSON of a [`HostError`]: version skew, unreadable input, the rejection, or a patch to
/// another domain.
#[wasm_bindgen]
pub fn apply(document: &str, request: &str) -> Result<String, String> {
    let document = read_document(document)?;
    let request: ApplyRequest = read("apply request", request)?;
    Ok(json(&apply_locally(&document, &request)?))
}

/// Previews a proposal against `document`'s journey: `request` is the JSON of a
/// [`PreviewRequest`]; the answer is the JSON of a `ProposalPreview`.
///
/// # Errors
///
/// The JSON of a [`HostError`]: version skew, unreadable input, or a proposal to another
/// domain.
#[wasm_bindgen]
pub fn preview(document: &str, request: &str) -> Result<String, String> {
    let document = read_document(document)?;
    let request: PreviewRequest = read("preview request", request)?;
    Ok(json(&preview_locally(&document, &request)?))
}

/// H5: what `patch` (its JSON) touches, as the JSON of a `TouchedSet`.
///
/// # Errors
///
/// The JSON of a [`HostError`] when the patch does not read.
#[wasm_bindgen]
pub fn touched(patch: &str) -> Result<String, String> {
    let patch: Patch = read("patch", patch)?;
    Ok(json(&patch.touched()))
}

/// H5: whether what intervened (the JSON of a stale rejection's `TouchedSet`) overlaps what
/// `patch` touches, so an automatic resubmission is not safe.
///
/// # Errors
///
/// The JSON of a [`HostError`] when either input does not read.
#[wasm_bindgen(js_name = touchedOverlaps)]
pub fn touched_overlaps(patch: &str, intervening: &str) -> Result<bool, String> {
    let patch: Patch = read("patch", patch)?;
    let intervening: TouchedSet = read("intervening", intervening)?;
    Ok(intervening.overlaps(&patch.touched()))
}
