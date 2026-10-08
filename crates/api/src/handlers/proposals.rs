//! Proposals (I6, C14; ARCHITECTURE, HTTP API: Resources): created against a domain with a
//! client-generated id at `POST /api/{domain}/proposals`, then addressed by that id alone under
//! `/api/proposals/{id}`, whose destination the stored proposal names. Every write runs on its own
//! task, is logged with its patch id and outcome, and counts toward the patch outcomes.

use axum::Json;
use axum::extract::{Extension, State};
use axum::routing::{get, patch, post};
use cairn_schema::{Actor, Domain, JourneyId, PatchId, Proposal, ProposalId, RouteId};
use cairn_service::{ProposalWritten, ProposeError, WriteError};
use cairn_store::Store;

use super::{Answer, committed, refusal_outcome, written_outcome};
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use crate::wire::{
    PatchAnswer, ProposalAnswer, ProposalApply, ProposalCreate, ProposalEdit, ProposalReview,
    ProposalStep,
};

/// The proposals' endpoints and handlers.
pub fn served<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    vec![
        (&at::PROPOSE_JOURNEY, post(propose_journey::<S>)),
        (&at::PROPOSE_ROUTE, post(propose_route::<S>)),
        (&at::PROPOSE_DEPLOYMENT, post(propose_deployment::<S>)),
        (&at::PROPOSAL, get(proposal::<S>)),
        (&at::EDIT_PROPOSAL, patch(edit::<S>)),
        (&at::PREVIEW_PROPOSAL, post(preview::<S>)),
        (&at::APPLY_PROPOSAL, post(apply::<S>)),
        (&at::DISCARD_PROPOSAL, post(discard::<S>)),
        (&at::REFRESH_PROPOSAL, post(refresh::<S>)),
    ]
}

/// `POST /api/journeys/{id}/proposals`.
pub async fn propose_journey<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalCreate>,
) -> Answer<ProposalAnswer> {
    let id: JourneyId = segment(&id, "journey")?;
    create(&api, actor, Domain::Journey(id), request).await
}

/// `POST /api/routes/{id}/proposals`.
pub async fn propose_route<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalCreate>,
) -> Answer<ProposalAnswer> {
    let id: RouteId = segment(&id, "route")?;
    create(&api, actor, Domain::Route(id), request).await
}

/// `POST /api/deployment/proposals`.
pub async fn propose_deployment<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    JsonBody(request): JsonBody<ProposalCreate>,
) -> Answer<ProposalAnswer> {
    create(&api, actor, Domain::Deployment, request).await
}

/// I6: creates a proposal for `destination`; a create resubmitted by proposal id answers it.
async fn create<S: Store + 'static>(
    api: &Api<S>,
    actor: Actor,
    destination: Domain,
    request: ProposalCreate,
) -> Answer<ProposalAnswer> {
    let ProposalCreate {
        patch_id,
        id,
        draft,
    } = request;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let written = committed(async move {
        service
            .create_proposal(&call, patch_id, &destination, &id, draft)
            .await
    })
    .await?;
    proposal_written(&logged_patch, &logged_id, "create", written)
}

/// `GET /api/proposals/{id}` (I6).
pub async fn proposal<S: Store + 'static>(
    State(api): State<Api<S>>,
    Path(id): Path<String>,
) -> Answer<Proposal> {
    let id = proposal_id(&id)?;
    Ok(Json(held(&api, &id).await?))
}

/// `PATCH /api/proposals/{id}` (I6, H5).
pub async fn edit<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalEdit>,
) -> Answer<ProposalAnswer> {
    let id = proposal_id(&id)?;
    let ProposalEdit {
        patch_id,
        base_revision,
        draft,
    } = request;
    let destination = destination_of(&api, &id, &patch_id, "edit").await?;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let written = committed(async move {
        service
            .edit_proposal(&call, patch_id, &destination, &id, base_revision, draft)
            .await
    })
    .await?;
    proposal_written(&logged_patch, &logged_id, "edit", written)
}

/// `POST /api/proposals/{id}/discard` (I6).
pub async fn discard<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalStep>,
) -> Answer<ProposalAnswer> {
    let id = proposal_id(&id)?;
    let ProposalStep {
        patch_id,
        base_revision,
    } = request;
    let destination = destination_of(&api, &id, &patch_id, "discard").await?;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let written = committed(async move {
        service
            .discard_proposal(&call, patch_id, &destination, &id, base_revision)
            .await
    })
    .await?;
    proposal_written(&logged_patch, &logged_id, "discard", written)
}

/// `POST /api/proposals/{id}/refresh` (I6): drafted again on its destination as it stands, the
/// reviewer's choices carried over; it must be reviewed again before it applies.
pub async fn refresh<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalStep>,
) -> Answer<ProposalAnswer> {
    let id = proposal_id(&id)?;
    let ProposalStep {
        patch_id,
        base_revision,
    } = request;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let drafted = committed(async move {
        service
            .refresh_proposal(&call, patch_id, &id, base_revision)
            .await
    })
    .await?;
    drafted_written(&logged_patch, &logged_id, "refresh", drafted)
}

/// `POST /api/proposals/{id}/preview` (C14, D7, I6).
pub async fn preview<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Answer<ProposalReview> {
    let id = proposal_id(&id)?;
    let review = api.service.preview_proposal(&api.call(actor), &id).await?;
    Ok(Json(review.into()))
}

/// `POST /api/proposals/{id}/apply` (I6, H2): the caller confirms it.
pub async fn apply<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<ProposalApply>,
) -> Answer<PatchAnswer> {
    let id = proposal_id(&id)?;
    let ProposalApply {
        patch_id,
        reviewed_revision,
        note,
    } = request;
    let destination = destination_of(&api, &id, &patch_id, "apply").await?;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let answer = committed(async move {
        service
            .apply_proposal(&call, patch_id, &destination, &id, reviewed_revision, note)
            .await
    })
    .await?;
    let outcome = written_outcome(&answer);
    tracing::info!(patch_id = %logged_patch, proposal = %logged_id, outcome, "proposal apply");
    observe::patch_outcome(outcome);
    Ok(Json(answer?.into()))
}

fn proposal_id(id: &str) -> Result<ProposalId, ApiError> {
    segment(id, "proposal")
}

/// The proposal with this id, or not found.
async fn held<S: Store + 'static>(api: &Api<S>, id: &ProposalId) -> Result<Proposal, ApiError> {
    api.service
        .proposal(id)
        .await?
        .ok_or_else(|| ApiError::not_found(format_args!("proposal {id}")))
}

/// The destination of the proposal a write names; a proposal that does not exist is logged
/// and counted like any other refused write, then answered not found.
async fn destination_of<S: Store + 'static>(
    api: &Api<S>,
    id: &ProposalId,
    patch_id: &PatchId,
    write: &'static str,
) -> Result<Domain, ApiError> {
    held(api, id)
        .await
        .map(|found| found.destination)
        .inspect_err(|error| {
            let outcome = refusal_outcome(error);
            tracing::info!(patch_id = %patch_id, proposal = %id, write, outcome, "proposal");
            observe::patch_outcome(outcome);
        })
}

/// Logs and counts a proposal write, and answers it.
pub(crate) fn proposal_written(
    patch_id: &PatchId,
    id: &ProposalId,
    write: &'static str,
    written: Result<ProposalWritten, WriteError>,
) -> Answer<ProposalAnswer> {
    drafted_written(patch_id, id, write, written.map_err(ProposeError::Write))
}

/// Logs and counts a proposal write that drafts, and answers it.
pub(crate) fn drafted_written(
    patch_id: &PatchId,
    id: &ProposalId,
    write: &'static str,
    written: Result<ProposalWritten, ProposeError>,
) -> Answer<ProposalAnswer> {
    let answer = written.map(ProposalAnswer::from).map_err(ApiError::from);
    let outcome = match &answer {
        Ok(answer) => answer.outcome(),
        Err(error) => refusal_outcome(error),
    };
    tracing::info!(patch_id = %patch_id, proposal = %id, write, outcome, "proposal");
    observe::patch_outcome(outcome);
    Ok(Json(answer?))
}
