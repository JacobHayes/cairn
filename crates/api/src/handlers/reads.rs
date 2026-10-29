//! The store-backed reads (ARCHITECTURE, HTTP API: Resources): whole documents by id, the
//! domain document, the journey index, route detail, search, and events.

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use cairn_schema::{
    Actor, Deployment, DomainDocument, IdentityKey, Journey, JourneyId, Route, RouteId,
    RouteVersion, VersionNumber,
};

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{Path, segment};
use crate::query::{self, Params};
use cairn_service::wire::{
    EventPage, IdentityReferences, JourneyPage, RouteDetail, RoutePage, SearchPage,
};

/// `GET /api/journeys` (C16).
pub async fn journeys(State(api): State<Api>, RawQuery(raw): RawQuery) -> Answer<JourneyPage> {
    let params = Params::parse(raw.as_deref(), query::JOURNEY_PARAMS)?;
    let page = api.service.journeys(&query::journeys(&params)?).await?;
    Ok(Json(page.into()))
}

/// `GET /api/journeys/{id}`.
pub async fn journey(State(api): State<Api>, Path(id): Path<String>) -> Answer<Journey> {
    let id: JourneyId = segment(&id, "journey")?;
    let journey = api.service.journey(&id).await?;
    journey
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format_args!("journey {id}")))
}

/// `GET /api/journeys/{id}/document`: at the caller's today, with their identities as the viewer.
pub async fn document(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Answer<DomainDocument> {
    let id: JourneyId = segment(&id, "journey")?;
    let document = api.service.document(&api.call(actor), &id).await?;
    document
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format_args!("journey {id}")))
}

/// `GET /api/routes` (I2).
pub async fn routes(State(api): State<Api>, RawQuery(raw): RawQuery) -> Answer<RoutePage> {
    let params = Params::parse(raw.as_deref(), query::ROUTE_PARAMS)?;
    let (after, size) = query::routes(&params)?;
    let page = api.service.routes(after.as_ref(), size).await?;
    Ok(Json(page.into()))
}

/// `GET /api/routes/{id}`.
pub async fn route(State(api): State<Api>, Path(id): Path<String>) -> Answer<Route> {
    let id: RouteId = segment(&id, "route")?;
    let route = api.service.route(&id).await?;
    route
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format_args!("route {id}")))
}

/// `GET /api/routes/{id}/versions` (C17).
pub async fn route_versions(State(api): State<Api>, Path(id): Path<String>) -> Answer<RouteDetail> {
    let id: RouteId = segment(&id, "route")?;
    let detail = api.service.route_detail(&id).await?;
    detail
        .map(|detail| Json(detail.into()))
        .ok_or_else(|| ApiError::not_found(format_args!("route {id}")))
}

/// `GET /api/routes/{id}/versions/{version}`.
pub async fn route_version(
    State(api): State<Api>,
    Path((id, version)): Path<(String, String)>,
) -> Answer<RouteVersion> {
    let id: RouteId = segment(&id, "route")?;
    let version: VersionNumber = segment(&version, "version")?;
    let found = api.service.route_version(&id, version).await?;
    found
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format_args!("version {version} of route {id}")))
}

/// `GET /api/deployment`.
pub async fn deployment(State(api): State<Api>) -> Answer<Deployment> {
    Ok(Json(api.service.deployment().await?))
}

/// `GET /api/deployment/identities/{key}/references` (E6).
pub async fn identity_references(
    State(api): State<Api>,
    Path(key): Path<String>,
) -> Answer<IdentityReferences> {
    let key: IdentityKey = segment(&key, "identity")?;
    let references = api.service.identity_references(&key).await?;
    references
        .map(|found| Json(found.into()))
        .ok_or_else(|| ApiError::not_found(format_args!("identity {key}")))
}

/// `GET /api/search`.
pub async fn search(State(api): State<Api>, RawQuery(raw): RawQuery) -> Answer<SearchPage> {
    let params = Params::parse(raw.as_deref(), query::SEARCH_PARAMS)?;
    let page = api.service.search(&query::search(&params)?).await?;
    Ok(Json(page.into()))
}

/// `GET /api/events` (J5).
pub async fn events(State(api): State<Api>, RawQuery(raw): RawQuery) -> Answer<EventPage> {
    let params = Params::parse(raw.as_deref(), query::EVENT_PARAMS)?;
    let page = api.service.events(&query::events(&params)?).await?;
    Ok(Json(page.into()))
}
