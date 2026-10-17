//! Proposals (I6, C14; ARCHITECTURE, HTTP API: Resources): created against a domain with a
//! client-generated id at `POST /api/{domain}/proposals`, then addressed by that id alone under
//! `/api/proposals/{id}`, whose destination the stored proposal names. Every write runs on its own
//! task, is logged with its patch id and outcome, and counts toward the patch outcomes.

use axum::Json;
use axum::extract::{Extension, RawPathParams, RawQuery, State};
use axum::routing::{get, patch, post};
use cairn_schema::{Actor, Domain, JourneyId, PatchId, Proposal, ProposalId, RouteId};
use cairn_service::{ProposalWritten, ProposeError, WriteError};
use cairn_store::{ProposalScope, Store};

use super::{Answer, committed, refusal_outcome, written_outcome};
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use crate::query::{self, Params};
use crate::wire::{
    PatchAnswer, ProposalAnswer, ProposalApply, ProposalCreate, ProposalEdit, ProposalList,
    ProposalReview, ProposalStep,
};

/// Which kind of destination a proposal endpoint is under: the router tells each handler
/// by an extension, so one handler serves a proposal's endpoint under all three.
#[derive(Clone, Copy, Debug)]
pub enum Under {
    /// `/api/journeys/{id}`.
    Journey,
    /// `/api/routes/{id}`.
    Route,
    /// `/api/deployment`.
    Deployment,
}

/// The proposals' endpoints and handlers.
pub fn served<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    let mut served: crate::Served<S> = vec![
        (&at::PROPOSE_JOURNEY, post(propose_journey::<S>)),
        (&at::PROPOSE_ROUTE, post(propose_route::<S>)),
        (&at::PROPOSE_DEPLOYMENT, post(propose_deployment::<S>)),
        (&at::PROPOSALS, get(listing::<S>)),
    ];
    for (under, set) in [
        (Under::Journey, &at::JOURNEY_PROPOSALS),
        (Under::Route, &at::ROUTE_PROPOSALS),
        (Under::Deployment, &at::DEPLOYMENT_PROPOSALS),
    ] {
        served.extend([
            (&set.list, get(list::<S>).layer(Extension(under))),
            (&set.get, get(proposal::<S>).layer(Extension(under))),
            (&set.edit, patch(edit::<S>).layer(Extension(under))),
            (&set.preview, post(preview::<S>).layer(Extension(under))),
            (&set.apply, post(apply::<S>).layer(Extension(under))),
            (&set.discard, post(discard::<S>).layer(Extension(under))),
            (&set.refresh, post(refresh::<S>).layer(Extension(under))),
        ]);
    }
    served
}

/// The destination the path names and, when it names one, the proposal under it.
fn addressed(
    under: Under,
    params: &RawPathParams,
) -> Result<(Domain, Option<ProposalId>), ApiError> {
    let mut id = None;
    let mut proposal = None;
    for (name, value) in params {
        match name {
            "id" => id = Some(value),
            "proposal" => proposal = Some(proposal_id(value)?),
            _ => unreachable!("a proposal endpoint's path has only an id and a proposal"),
        }
    }
    let destination = match (under, id) {
        (Under::Journey, Some(id)) => Domain::Journey(segment::<JourneyId>(id, "journey")?),
        (Under::Route, Some(id)) => Domain::Route(segment::<RouteId>(id, "route")?),
        (Under::Deployment, None) => Domain::Deployment,
        (Under::Journey | Under::Route | Under::Deployment, _) => {
            unreachable!("a proposal endpoint's path names its destination")
        }
    };
    Ok((destination, proposal))
}

/// The destination and proposal of a path that names a proposal.
fn addressed_proposal(
    under: Under,
    params: &RawPathParams,
) -> Result<(Domain, ProposalId), ApiError> {
    match addressed(under, params)? {
        (destination, Some(proposal)) => Ok((destination, proposal)),
        (_, None) => unreachable!("a proposal endpoint's path names its proposal"),
    }
}

/// `GET /api/{destination}/proposals` (C14): the destination's open proposals, newest first.
pub async fn list<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
    RawQuery(raw): RawQuery,
) -> Answer<ProposalList> {
    let (destination, _) = addressed(under, &params)?;
    let (after, size) = query::proposals(&Params::parse(raw.as_deref(), query::PROPOSAL_PARAMS)?)?;
    let page = api
        .service
        .proposals(ProposalScope::Destination(destination), after, size)
        .await?;
    Ok(Json(page.into()))
}

/// `GET /api/proposals?scope=` (C14, C16): the open proposals no one destination holds.
pub async fn listing<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    RawQuery(raw): RawQuery,
) -> Answer<ProposalList> {
    let params = Params::parse(raw.as_deref(), query::PROPOSAL_SCOPE_PARAMS)?;
    let (after, size) = query::proposals(&params)?;
    let page = match params.one::<query::ProposalListing>("scope")? {
        Some(query::ProposalListing::Waiting) => {
            api.service
                .proposals_waiting(&api.call(actor), after, size)
                .await?
        }
        Some(query::ProposalListing::NewJourneys) => {
            api.service
                .proposals(ProposalScope::CreatingJourneys, after, size)
                .await?
        }
        Some(query::ProposalListing::NewRoutes) => {
            api.service
                .proposals(ProposalScope::CreatingRoutes, after, size)
                .await?
        }
        None => unreachable!("the scope is required"),
    };
    Ok(Json(page.into()))
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

/// `GET /api/{destination}/proposals/{proposal}` (I6).
pub async fn proposal<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
) -> Answer<Proposal> {
    let (destination, id) = addressed_proposal(under, &params)?;
    Ok(Json(held(&api, &destination, &id).await?))
}

/// `PATCH /api/{destination}/proposals/{proposal}` (I6, H5).
pub async fn edit<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
    JsonBody(request): JsonBody<ProposalEdit>,
) -> Answer<ProposalAnswer> {
    let (destination, id) = addressed_proposal(under, &params)?;
    let ProposalEdit {
        patch_id,
        base_revision,
        draft,
    } = request;
    held_at(&api, &destination, &id, &patch_id, "edit").await?;
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

/// `POST /api/{destination}/proposals/{proposal}/discard` (I6).
pub async fn discard<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
    JsonBody(request): JsonBody<ProposalStep>,
) -> Answer<ProposalAnswer> {
    let (destination, id) = addressed_proposal(under, &params)?;
    let ProposalStep {
        patch_id,
        base_revision,
    } = request;
    held_at(&api, &destination, &id, &patch_id, "discard").await?;
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

/// `POST /api/{destination}/proposals/{proposal}/refresh` (I6): drafted again on its destination as it stands, the
/// reviewer's choices carried over; it must be reviewed again before it applies.
pub async fn refresh<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
    JsonBody(request): JsonBody<ProposalStep>,
) -> Answer<ProposalAnswer> {
    let (destination, id) = addressed_proposal(under, &params)?;
    let ProposalStep {
        patch_id,
        base_revision,
    } = request;
    held_at(&api, &destination, &id, &patch_id, "refresh").await?;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (id.clone(), patch_id.clone());
    let drafted = committed(async move {
        service
            .refresh_proposal(&call, patch_id, &destination, &id, base_revision)
            .await
    })
    .await?;
    drafted_written(&logged_patch, &logged_id, "refresh", drafted)
}

/// `POST /api/{destination}/proposals/{proposal}/preview` (C14, D7, I6).
pub async fn preview<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
) -> Answer<ProposalReview> {
    let (destination, id) = addressed_proposal(under, &params)?;
    let review = api
        .service
        .preview_proposal(&api.call(actor), &destination, &id)
        .await?;
    Ok(Json(review.into()))
}

/// `POST /api/{destination}/proposals/{proposal}/apply` (I6, H2): the caller confirms it.
pub async fn apply<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Extension(under): Extension<Under>,
    params: RawPathParams,
    JsonBody(request): JsonBody<ProposalApply>,
) -> Answer<PatchAnswer> {
    let (destination, id) = addressed_proposal(under, &params)?;
    let ProposalApply {
        patch_id,
        reviewed_revision,
        note,
    } = request;
    held_at(&api, &destination, &id, &patch_id, "apply").await?;
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

/// The proposal with this id under `destination`, or not found: a proposal under another
/// destination is not found here.
async fn held<S: Store + 'static>(
    api: &Api<S>,
    destination: &Domain,
    id: &ProposalId,
) -> Result<Proposal, ApiError> {
    api.service
        .proposal_at(destination, id)
        .await?
        .ok_or_else(|| ApiError::not_found(format_args!("proposal {id} under {destination}")))
}

/// [`held`] for a write: a proposal that is not there is logged and counted like any other
/// refused write, then answered not found.
async fn held_at<S: Store + 'static>(
    api: &Api<S>,
    destination: &Domain,
    id: &ProposalId,
    patch_id: &PatchId,
    write: &'static str,
) -> Result<(), ApiError> {
    held(api, destination, id)
        .await
        .map(|_| ())
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
