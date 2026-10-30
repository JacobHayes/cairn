//! Bulk and import (ARCHITECTURE, HTTP API: Bulk and import): route files exported and
//! imported, an import being one route patch (A13).

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use axum::routing::{get, post};
use cairn_schema::{Actor, RouteFile, RouteId};

use super::{Answer, committed, written_outcome};
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::observe;
use crate::query::{self, Params};
use cairn_service::wire::{PatchAnswer, ProblemCode, RouteImport};

/// The bulk endpoints and their handlers.
pub fn served() -> crate::Served {
    use crate::endpoints as at;
    vec![
        (&at::IMPORT_ROUTE, post(import)),
        (&at::EXPORT_ROUTE, get(export)),
    ]
}

/// `POST /api/routes/{id}/import` (A13): one route patch opening a new route or a new draft.
pub async fn import(
    State(api): State<Api>,
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
    let answer = committed(async move { service.import_route(&call, patch_id, &file, note).await })
        .await?
        .map_err(ApiError::from);
    let outcome = written_outcome(&answer);
    tracing::info!(patch_id = %logged_patch, %route, outcome, "route import");
    observe::patch_outcome(outcome);
    Ok(Json(answer?.into()))
}

/// `GET /api/routes/{id}/export` (A13): a version, or the draft when none is asked for.
pub async fn export(
    State(api): State<Api>,
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
