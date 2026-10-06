//! The handlers: each reads its request, makes one service call (or, for agent tokens, one
//! auth call) as the caller the auth layer named, and answers its wire shape. None decides
//! anything the service does not: the API shapes the service's vocabulary for HTTP and
//! never bypasses it (ARCHITECTURE, Service layer and composition).
pub mod reads;
pub mod users;

use axum::Json;
use axum::extract::{Extension, State};
use cairn_schema::{Actor, Domain, JourneyId, PatchTarget, Rejection, RouteId};
use cairn_service::{DomainPatch, WriteError};
use cairn_store::Store;

use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use crate::wire::{Capabilities, PatchAnswer, PatchRequest, ProblemCode};

/// A handler's answer: its JSON body, or what went wrong.
pub type Answer<T> = Result<Json<T>, ApiError>;

/// `GET /capabilities`.
pub async fn capabilities<S: Store + 'static>(State(api): State<Api<S>>) -> Json<Capabilities> {
    Json(Capabilities::from(api.service.capabilities()))
}

/// `POST /journeys/{id}/patches`.
pub async fn patch_journey<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    let id: JourneyId = segment(&id, "journey")?;
    submit(&api, actor, &Domain::Journey(id), request).await
}

/// `POST /routes/{id}/patches`.
pub async fn patch_route<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    let id: RouteId = segment(&id, "route")?;
    submit(&api, actor, &Domain::Route(id), request).await
}

/// `POST /deployment/patches`.
pub async fn patch_deployment<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    submit(&api, actor, &Domain::Deployment, request).await
}

/// A17: one domain patch as `actor`, whose target must be `domain`. A proposal is edited
/// through its own endpoints (4.9), never as a domain.
async fn submit<S: Store + 'static>(
    api: &Api<S>,
    actor: Actor,
    domain: &Domain,
    request: PatchRequest,
) -> Answer<PatchAnswer> {
    let PatchRequest { patch, note } = request;
    let proposal = matches!(patch.target, PatchTarget::Proposal { .. });
    if proposal || patch.target.domain() != *domain {
        let message = format!("the patch targets {:?}, not {domain}", patch.target);
        return Err(ApiError::problem(ProblemCode::TargetMismatch, message));
    }
    let patch_id = patch.id.clone();
    let submitted = DomainPatch::new(patch, note)
        .map_err(|error| ApiError::problem(ProblemCode::TargetMismatch, error.to_string()))?;
    // On its own task, so the request duration limit can answer the caller without cutting
    // a commit off between its store write and its announcement (H6); a resubmission by
    // patch id finds the result (H5).
    let (service, call) = (api.service.clone(), api.call(actor));
    let answer = tokio::spawn(async move { service.patch(&call, &submitted).await })
        .await
        .map_err(|error| {
            tracing::error!(%error, "a patch task failed");
            ApiError::problem(ProblemCode::Internal, "the patch task failed")
        })?;
    let outcome = match &answer {
        Ok(cairn_service::Written::Applied { .. }) => "applied",
        Ok(cairn_service::Written::AlreadyApplied { .. }) => "already_applied",
        Err(WriteError::Rejected(Rejection::Invalid { .. })) => "invalid",
        Err(WriteError::Rejected(Rejection::Stale { .. })) => "stale",
        Err(WriteError::Rejected(Rejection::PatchIdReused { .. })) => "patch_id_reused",
        Err(WriteError::Failed(_)) => "failed",
    };
    tracing::info!(patch_id = %patch_id, %domain, outcome, "patch");
    observe::patch_outcome(outcome);
    match answer {
        Ok(written) => Ok(Json(written.into())),
        Err(WriteError::Rejected(rejection)) => Err(ApiError::Rejected(rejection)),
        Err(WriteError::Failed(error)) => Err(error.into()),
    }
}

/// Any path no endpoint serves.
pub async fn no_such_endpoint() -> ApiError {
    ApiError::problem(ProblemCode::NoSuchEndpoint, "no endpoint at this path")
}

/// An endpoint asked with a method it does not take.
pub async fn method_not_allowed() -> ApiError {
    ApiError::problem(
        ProblemCode::MethodNotAllowed,
        "this endpoint takes another method",
    )
}
