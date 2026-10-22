//! The handlers: each reads its request, makes one service call (or, for agent tokens, one
//! auth call) as the caller the auth layer named, and answers its wire shape. None decides
//! anything the service does not: the API shapes the service's vocabulary for HTTP and
//! never bypasses it (ARCHITECTURE, Service layer and composition).
pub mod assistant;
pub mod bulk;
pub mod projections;
pub mod proposals;
pub mod reads;
pub mod users;

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use cairn_schema::{Actor, Domain, JourneyId, PatchTarget, Rejection, RouteId};
use cairn_service::{DomainPatch, Written};

use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use cairn_service::wire::{
    Capabilities, Health, HealthStatus, PatchAnswer, PatchRequest, ProblemCode,
};

/// A handler's answer: its JSON body, or what went wrong.
pub type Answer<T> = Result<Json<T>, ApiError>;

/// `GET /healthz`, outside the auth layer: 200 while the store answers, 503 once it has
/// failed closed. Cheap: it reads the store's state, never its storage.
pub async fn health(State(api): State<Api>) -> Response {
    match api.service.health() {
        Ok(()) => (
            StatusCode::OK,
            Json(Health {
                status: HealthStatus::Ok,
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "health check: the store has failed closed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: HealthStatus::StoreFailedClosed,
                }),
            )
                .into_response()
        }
    }
}

/// `GET /api/capabilities`.
pub async fn capabilities(State(api): State<Api>) -> Json<Capabilities> {
    Json(Capabilities::from(api.service.capabilities()))
}

/// `POST /api/journeys/{id}/patches`.
pub async fn patch_journey(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    let id: JourneyId = segment(&id, "journey")?;
    submit(&api, actor, &Domain::Journey(id), request).await
}

/// `POST /api/routes/{id}/patches`.
pub async fn patch_route(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    let id: RouteId = segment(&id, "route")?;
    submit(&api, actor, &Domain::Route(id), request).await
}

/// `POST /api/deployment/patches`.
pub async fn patch_deployment(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    JsonBody(request): JsonBody<PatchRequest>,
) -> Answer<PatchAnswer> {
    submit(&api, actor, &Domain::Deployment, request).await
}

/// A17: one domain patch as `actor`, whose target must be `domain`. A proposal is edited
/// through its own endpoints (4.9), never as a domain.
async fn submit(
    api: &Api,
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
    let (service, call) = (api.service.clone(), api.call(actor));
    let answer = committed(async move { service.patch(&call, &submitted).await })
        .await?
        .map_err(ApiError::from);
    let outcome = written_outcome(&answer);
    tracing::info!(patch_id = %patch_id, %domain, outcome, "patch");
    observe::patch_outcome(outcome);
    Ok(Json(answer?.into()))
}

/// Runs a write on its own task, so the request duration limit can answer the caller without
/// cutting a commit off between its store write and its announcement (H6); a resubmission by
/// patch id finds the result (H5).
///
/// # Errors
///
/// An internal problem when the task fails.
pub(crate) async fn committed<T: Send + 'static>(
    write: impl Future<Output = T> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::spawn(write).await.map_err(|error| {
        tracing::error!(%error, "a write task failed");
        ApiError::problem(ProblemCode::Internal, "the write task failed")
    })
}

/// A domain write's outcome, as the patch count and logs name it. The write's error is
/// converted to the API's once, by the handler, which answers with that same result: the
/// conversion logs and counts an engine panic, so a second one would count it twice.
pub(crate) fn written_outcome(answer: &Result<Written, ApiError>) -> &'static str {
    match answer {
        Ok(Written::Applied { over: None, .. }) => "applied",
        Ok(Written::Applied { over: Some(_), .. }) => "applied_over",
        Ok(Written::AlreadyApplied { .. }) => "already_applied",
        Err(error) => refusal_outcome(error),
    }
}

/// A refused write's outcome from its answer, as the patch count and logs name it: a
/// rejection by its kind, `not_found`, `not_drafted` for a request Cairn cannot draft, or
/// `failed`.
pub(crate) fn refusal_outcome(error: &ApiError) -> &'static str {
    match error {
        ApiError::Rejected(Rejection::Invalid { .. }) => "invalid",
        ApiError::Rejected(Rejection::Stale { .. }) => "stale",
        ApiError::Rejected(Rejection::PatchIdReused { .. }) => "patch_id_reused",
        ApiError::Problem {
            code: ProblemCode::NotFound,
            ..
        } => "not_found",
        ApiError::Problem {
            code: ProblemCode::CannotDraft,
            ..
        } => "not_drafted",
        ApiError::Problem { .. } => "failed",
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
