//! Local writes over a domain document (ARCHITECTURE, Web UI: previews; Write path): a draft
//! patch applied to the browser's in-memory journey with the engine's `apply`, a draft patch
//! applied to a route the browser holds (its draft has no state, so there is nothing to
//! derive), and a proposal previewed (C14).
//! Nothing here commits: committed state always comes from the server. Each mirrors what the
//! server's service does with the same records, so its answer is the server's byte for byte.
//!
//! Cost: an apply is one engine apply and, for its consequences, two derives; a route apply is
//! one engine apply; a preview is the engine's (one apply, two derives).

use std::collections::BTreeSet;

use cairn_engine::format::placeholders;
use cairn_engine::{ApplyInputs, Graph as EngineGraph, Records, consequences, derive};
use cairn_schema::condition::ConditionError;
use cairn_schema::{
    Actor, Clause, Condition, Consequences, Date, Deployment, Domain, DomainDocument, Graph,
    KeyRefs, Limit, Lineage, Markdown, NodeFormValues, Patch, PatchTarget, Proposal, RemovalTarget,
    Route, RouteId, RouteVersion, Timestamp, ValueCheck, node_form_problems, value_problems,
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
    /// The route versions an `insert_route` in it reads (B13), as
    /// `GET /api/routes/{id}/versions/{version}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<RouteVersion>,
    /// The routes those versions belong to, as `GET /api/routes/{id}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
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
    /// (the deployment as the patch leaves it, when it creates identities).
    pub document: DomainDocument,
    /// D7: what the patch newly causes in the journey, as the server's patch answer reports
    /// it (empty when nothing).
    pub consequences: Consequences,
}

/// A proposal to preview against the document's journey.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    /// The proposal, as `GET /api/{destination}/proposals/{proposal}` answers it; its destination is the journey.
    pub proposal: Proposal,
    /// The route versions its `insert_route` mutations read, as `GET /api/routes/{id}/versions/{version}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<RouteVersion>,
    /// The routes an insertion in it reads (B13), as `GET /api/routes/{id}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
    /// When the preview is asked for.
    pub at: Timestamp,
    /// Who asks (H2).
    pub actor: Actor,
}

/// A proposal previewed locally: the preview, and the journey as it would be, derivable in
/// the tab (none when the candidate breaks a rule).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewAnswer {
    /// The preview, as the server's `ProposalReview` carries it.
    pub preview: cairn_schema::ProposalPreview,
    /// The journey document after, produced by the engine's candidate and never assembled by
    /// the page (C14).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<cairn_schema::DestinationAfter>,
}

/// A draft patch to apply locally to a route (A11, A12: authoring a route's draft by hand),
/// with what it reads and the clock and actor its events would carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteApplyRequest {
    /// The route as `GET /api/routes/{id}` answers it, with its draft; none for a patch that
    /// creates the route (base revision 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// The published versions the patch reads (the latest, for a patch opening a draft from it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<RouteVersion>,
    /// The routes an insertion in the patch reads (B13), as `GET /api/routes/{id}` answers them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
    /// The deployment context (E6): identities a participation names.
    pub deployment: Deployment,
    /// The patch; it targets the route.
    pub patch: Patch,
    /// The deployment's today.
    pub today: Date,
    /// When it would commit.
    pub at: Timestamp,
    /// Who would submit it (H2).
    pub actor: Actor,
}

/// A17 locally on a route: `request`'s patch applied to the route it holds (or to none, for
/// a patch that creates it), answered with the route as the patch leaves it. A route has no
/// state, so a route patch causes no consequences (D7).
///
/// # Errors
///
/// [`HostError::Rejected`] with the engine's rejection; [`HostError::Missing`] when the patch
/// targets another domain than the route held.
pub fn apply_route_locally(request: &RouteApplyRequest) -> Result<Route, HostError> {
    let PatchTarget::Route(id) = &request.patch.target else {
        return Err(HostError::Missing {
            message: "a route apply takes a patch to a route".to_owned(),
        });
    };
    let held = request.route.as_ref().map(|route| &route.header.id);
    if held.is_some_and(|held| held != id) {
        return Err(HostError::Missing {
            message: format!(
                "the request holds route {}, not {id}",
                held.map_or("", RouteId::as_str)
            ),
        });
    }
    let mut records = Records {
        deployment: request.deployment.clone(),
        ..Records::default()
    };
    if let Some(route) = &request.route {
        records.routes.insert(id.clone(), route.clone());
    }
    hold_versions(&mut records, &request.versions, &request.routes);
    let inputs = ApplyInputs {
        today: request.today,
        at: request.at,
        actor: request.actor.clone(),
        note: None,
    };
    let applied = cairn_engine::apply(&records, &request.patch, &inputs)
        .map_err(|rejection| HostError::Rejected { rejection })?;
    match applied.records().routes.get(id) {
        Some(route) => Ok(route.clone()),
        None => unreachable!("an accepted route patch leaves its route"),
    }
}

/// The published versions a patch reads, and the routes they belong to when it inserts from
/// them (B13: the engine checks a route's retirement on its header).
fn hold_versions(records: &mut Records, versions: &[RouteVersion], routes: &[Route]) {
    for version in versions {
        let lineage = Lineage {
            route: version.route.clone(),
            version: version.version,
        };
        records.versions.insert(lineage, version.clone());
    }
    for route in routes {
        records
            .routes
            .insert(route.header.id.clone(), route.clone());
    }
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
        let graph = match EngineGraph::new(document.journey.graph.clone(), deployment) {
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
    // server's to apply.
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
    let mut held = records_of(document);
    hold_versions(&mut held, &request.versions, &request.routes);
    let applied = cairn_engine::apply(&held, &request.patch, &inputs)
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
/// as the server's preview derives it (no viewer, the document's today), with the journey as
/// it would be, over the viewer's own derive inputs.
///
/// # Errors
///
/// [`HostError::Missing`] when the proposal's destination is not the document's journey.
pub fn preview_locally(
    document: &DomainDocument,
    request: &PreviewRequest,
) -> Result<PreviewAnswer, HostError> {
    let proposal = &request.proposal;
    own_journey(document, &proposal.destination)?;
    let mut records = records_of(document);
    records
        .proposals
        .insert(proposal.id.clone(), proposal.clone());
    hold_versions(&mut records, &request.versions, &request.routes);
    let inputs = ApplyInputs {
        today: document.inputs.today,
        at: request.at,
        actor: request.actor.clone(),
        note: None,
    };
    let mut derive_inputs = document.inputs.clone();
    derive_inputs.viewer = BTreeSet::new();
    let candidate =
        cairn_engine::candidate(&records, &proposal.destination, &proposal.draft, &inputs);
    let preview =
        cairn_engine::preview_of(&records, &proposal.destination, &candidate, &derive_inputs);
    // The views derive the journey after as its viewer sees it, like the journey itself.
    let after = candidate.after.as_ref().ok().map(|after| {
        cairn_engine::destination_after(after, &proposal.destination, &document.inputs)
    });
    Ok(PreviewAnswer { preview, after })
}

/// Applies a draft patch to `document`'s journey without committing it: `request` is the JSON
/// of an [`ApplyRequest`]; the answer is the JSON of an [`AppliedLocally`].
///
/// # Errors
///
/// The JSON of a [`HostError`]: unreadable input, the rejection, or a patch to another domain.
#[wasm_bindgen]
pub fn apply(document: &str, request: &str) -> Result<String, String> {
    let document = read_document(document)?;
    let request: ApplyRequest = read("apply request", request)?;
    Ok(json(&apply_locally(&document, &request)?))
}

/// Applies a draft patch to a route without committing it: `request` is the JSON of a
/// [`RouteApplyRequest`]; the answer is the JSON of the route as the patch leaves it.
///
/// # Errors
///
/// The JSON of a [`HostError`]: unreadable input, the rejection, or a patch to another domain.
#[wasm_bindgen(js_name = applyRoute)]
pub fn apply_route(request: &str) -> Result<String, String> {
    let request: RouteApplyRequest = read("route apply request", request)?;
    Ok(json(&apply_route_locally(&request)?))
}

/// Previews a proposal against `document`'s journey: `request` is the JSON of a
/// [`PreviewRequest`]; the answer is the JSON of a `ProposalPreview`.
///
/// # Errors
///
/// The JSON of a [`HostError`]: unreadable input, or a proposal to another domain.
#[wasm_bindgen]
pub fn preview(document: &str, request: &str) -> Result<String, String> {
    let document = read_document(document)?;
    let request: PreviewRequest = read("preview request", request)?;
    Ok(json(&preview_locally(&document, &request)?))
}

/// What is wrong with a condition's size or shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionProblem {
    /// Deeper than the depth limit or holding more comparisons than the clause limit.
    TooLarge {
        /// The limit passed.
        limit: Limit,
        /// The depth or count that passed it.
        count: u64,
    },
    /// An `all` or `any` with nothing in it.
    EmptyCombination,
    /// The clause does not read as one a condition holds (a list of values past its limit,
    /// say), in the words the reader gave.
    Unreadable {
        /// What the reader said.
        reason: String,
    },
}

/// A5: what keeps `clause` from being a condition, in the one place the rule lives
/// ([`Condition::new`]); `None` when it can be one.
#[must_use]
pub fn condition_problem_of(clause: Clause<KeyRefs>) -> Option<ConditionProblem> {
    match Condition::new(clause) {
        Ok(_) => None,
        Err(ConditionError::TooLarge(exceeded)) => Some(ConditionProblem::TooLarge {
            limit: exceeded.limit,
            count: exceeded.count,
        }),
        Err(ConditionError::EmptyCombination) => Some(ConditionProblem::EmptyCombination),
    }
}

/// A5: the JSON of what keeps the clause (the JSON of a `Clause`) from being a condition: a
/// `{"too_large": {"limit", "count"}}`, `"empty_combination"`, `{"unreadable": {"reason"}}`, or
/// `null` when it can be one.
#[wasm_bindgen(js_name = conditionProblem)]
#[must_use]
pub fn condition_problem(clause: &str) -> String {
    match serde_json::from_str::<Clause<KeyRefs>>(clause) {
        Ok(clause) => json(&condition_problem_of(clause)),
        Err(error) => json(&Some(ConditionProblem::Unreadable {
            reason: error.to_string(),
        })),
    }
}

/// A1a, A4, A9, G5: the JSON of the problems with a node form's values (the JSON of
/// [`NodeFormValues`]), one per field that cannot be sent, each `{"field", "message"}` with the
/// message in words for the person filling in the form; `[]` when all can be sent. The rules
/// are the types' own, the ones the patch reader builds a node from.
///
/// # Errors
///
/// The JSON of a [`HostError`] when the values do not read.
#[wasm_bindgen(js_name = nodeFormProblems)]
pub fn node_form_problems_text(values: &str) -> Result<String, String> {
    let values: NodeFormValues = read("node form values", values)?;
    Ok(json(&node_form_problems(&values)))
}

/// A1a, A6, A7, E6: the JSON of the problems with the loose values of a form (the JSON of
/// a list of [`ValueCheck`]), one `{"field", "message"}` per value that cannot be sent; `[]`
/// when all can be. The rules are the types' own, the ones the patch reader builds the same
/// values from.
///
/// # Errors
///
/// The JSON of a [`HostError`] when the checks do not read.
#[wasm_bindgen(js_name = valueProblems)]
pub fn value_problems_text(checks: &str) -> Result<String, String> {
    let checks: Vec<ValueCheck> = read("value checks", checks)?;
    Ok(json(&value_problems(&checks)))
}

/// A18: the JSON of a `RemovalPlan`: what removing `target` (a `RemovalTarget`) from `graph`
/// (a `Graph`, with its state for a journey) takes and leaves dangling, with the mutations
/// that rewrite what dangles; `null` when a node asked for is not in the graph.
///
/// # Errors
///
/// The JSON of a [`HostError`] when either input does not read.
#[wasm_bindgen(js_name = removalPlan)]
pub fn removal_plan(graph: &str, target: &str) -> Result<String, String> {
    let graph: Graph = read("graph", graph)?;
    let target: RemovalTarget = read("removal target", target)?;
    Ok(json(&cairn_engine::plan_removal(&graph, &target)))
}

/// A10: a stored message draft as an author reads it (roles by id, decisions by path), as
/// the JSON of a string.
///
/// # Errors
///
/// The JSON of a [`HostError`] when the graph does not read.
#[wasm_bindgen(js_name = draftWritten)]
pub fn draft_written(graph: &str, stored: &str) -> Result<String, String> {
    let graph: Graph = read("graph", graph)?;
    Ok(json(&placeholders::written(&graph, stored)))
}

/// A10: a draft an author wrote, stored with keys: `{"stored": text}`, or `{"problem":
/// {"unreadable": words}}` or `{"problem": {"unknown": [placeholders]}}`.
///
/// # Errors
///
/// The JSON of a [`HostError`] when the graph does not read.
#[wasm_bindgen(js_name = draftStored)]
pub fn draft_stored(graph: &str, written: &str) -> Result<String, String> {
    let graph: Graph = read("graph", graph)?;
    Ok(match placeholders::stored(&graph, written) {
        Ok(stored) => json(&serde_json::json!({ "stored": stored })),
        Err(problem) => json(&serde_json::json!({ "problem": problem })),
    })
}
