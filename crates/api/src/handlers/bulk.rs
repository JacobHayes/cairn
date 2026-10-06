//! Bulk and import (ARCHITECTURE, HTTP API: Bulk and import): upgrade, save as route, and
//! re-link, each drafted by the engine and stored as a proposal that is reviewed and applied
//! through the proposal endpoints (B7, B8, B9, I6); and route files exported and imported, an
//! import being one route patch (A13).

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use axum::routing::{get, post};
use cairn_schema::{Actor, JourneyId, RouteFile, RouteId};
use cairn_store::Store;

use super::proposals::drafted_written;
use super::{Answer, committed, written_outcome};
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use crate::query::{self, Params};
use crate::wire::{
    PatchAnswer, ProblemCode, ProposalAnswer, RelinkRequest, RouteImport, SaveAsRouteRequest,
    UpgradeRequest,
};

/// The bulk endpoints and their handlers.
pub fn served<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    vec![
        (&at::UPGRADE, post(upgrade::<S>)),
        (&at::SAVE_AS_ROUTE, post(save_as_route::<S>)),
        (&at::RELINK, post(relink::<S>)),
        (&at::IMPORT_ROUTE, post(import::<S>)),
        (&at::EXPORT_ROUTE, get(export::<S>)),
    ]
}

/// `POST /journeys/{id}/upgrade` (B7).
pub async fn upgrade<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<UpgradeRequest>,
) -> Answer<ProposalAnswer> {
    let journey: JourneyId = segment(&id, "journey")?;
    let UpgradeRequest {
        patch_id,
        proposal,
        to,
    } = request;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (proposal.clone(), patch_id.clone());
    let drafted = committed(async move {
        service
            .propose_upgrade(&call, patch_id, &proposal, &journey, to)
            .await
    })
    .await?;
    drafted_written(&logged_patch, &logged_id, "upgrade", drafted)
}

/// `POST /journeys/{id}/save-as-route` (B8).
pub async fn save_as_route<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<SaveAsRouteRequest>,
) -> Answer<ProposalAnswer> {
    let journey: JourneyId = segment(&id, "journey")?;
    let SaveAsRouteRequest {
        patch_id,
        proposal,
        route,
        name,
    } = request;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (proposal.clone(), patch_id.clone());
    let drafted = committed(async move {
        service
            .propose_save_as_route(&call, patch_id, &proposal, &journey, &route, &name)
            .await
    })
    .await?;
    drafted_written(&logged_patch, &logged_id, "save_as_route", drafted)
}

/// `POST /journeys/{id}/relink` (B9).
pub async fn relink<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<RelinkRequest>,
) -> Answer<ProposalAnswer> {
    let journey: JourneyId = segment(&id, "journey")?;
    let RelinkRequest {
        patch_id,
        proposal,
        lineage,
    } = request;
    let (service, call) = (api.service.clone(), api.call(actor));
    let (logged_id, logged_patch) = (proposal.clone(), patch_id.clone());
    let drafted = committed(async move {
        service
            .propose_relink(&call, patch_id, &proposal, &journey, &lineage)
            .await
    })
    .await?;
    drafted_written(&logged_patch, &logged_id, "relink", drafted)
}

/// `POST /routes/{id}/import` (A13): one route patch opening a new route or a new draft.
pub async fn import<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<RouteImport>,
) -> Answer<PatchAnswer> {
    let route: RouteId = segment(&id, "route")?;
    let RouteImport {
        patch_id,
        file,
        note,
    } = request;
    if file.route != route {
        let message = format!("the file is of route {}, not {route}", file.route);
        return Err(ApiError::problem(ProblemCode::TargetMismatch, message));
    }
    let (service, call) = (api.service.clone(), api.call(actor));
    let logged_patch = patch_id.clone();
    let answer =
        committed(async move { service.import_route(&call, patch_id, &file, note).await }).await?;
    let outcome = written_outcome(&answer);
    tracing::info!(patch_id = %logged_patch, %route, outcome, "route import");
    observe::patch_outcome(outcome);
    Ok(Json(answer?.into()))
}

/// `GET /routes/{id}/export` (A13): a version, or the draft when none is asked for.
pub async fn export<S: Store + 'static>(
    State(api): State<Api<S>>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Answer<RouteFile> {
    let route: RouteId = segment(&id, "route")?;
    let params = Params::parse(raw.as_deref(), query::EXPORT_PARAMS)?;
    let version = params.one("version")?;
    let file = api.service.export_route(&route, version).await?;
    file.map(Json).ok_or_else(|| match version {
        Some(version) => ApiError::not_found(format_args!("version {version} of route {route}")),
        None => ApiError::not_found(format_args!("draft of route {route}")),
    })
}
