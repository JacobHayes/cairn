//! The derived reads (ARCHITECTURE, HTTP API: Resources; I1, I3, C8): every projection of a
//! journey, derived by the service at the caller's today with their identities as the viewer,
//! answered with the revisions it came from; node detail with its explanation lists capped
//! and pageable; and history by page.

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use axum::routing::get;
use cairn_schema::{
    Actor, DecisionView, Derived, ExplainedField, ExplanationPage, JourneyId, Level, ListPage,
    Next, NodeKey, RemovalRead, Snapshot, StatusSummary, Timeline, Trace,
};

use super::Answer;
use crate::Api;
use crate::extract::{Path, segment};
use crate::query::{self, Params};
use cairn_service::wire::{History, Mine, NodeDetail, Projected};

/// The derived reads' endpoints and handlers.
pub fn served() -> crate::Served {
    use crate::endpoints as at;
    vec![
        (&at::SNAPSHOT, get(snapshot)),
        (&at::LEVEL, get(level)),
        (&at::TRACE, get(trace)),
        (&at::DERIVED, get(derived)),
        (&at::DECISIONS, get(decisions)),
        (&at::TIMELINE, get(timeline)),
        (&at::SUMMARY, get(summary)),
        (&at::NEXT, get(next)),
        (&at::NODES, get(nodes)),
        (&at::MINE, get(mine)),
        (&at::NODE, get(node_detail)),
        (&at::NODE_REMOVAL, get(node_removal)),
        (&at::EXPLANATIONS, get(explanations)),
        (&at::HISTORY, get(history)),
    ]
}

/// A projection's answer.
type Projection<T> = Answer<Projected<T>>;

fn journey(id: &str) -> Result<JourneyId, crate::error::ApiError> {
    segment(id, "journey")
}

fn node(key: &str) -> Result<NodeKey, crate::error::ApiError> {
    segment(key, "node")
}

/// `GET /api/journeys/{id}/snapshot` (I3).
pub async fn snapshot(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Projection<Snapshot> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::SNAPSHOT_PARAMS)?;
    let scope = query::snapshot(&params)?;
    let projected = api.service.snapshot(&api.call(actor), &id, &scope).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/level` (C2).
pub async fn level(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Projection<Level> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::LEVEL_PARAMS)?;
    let query = query::level(&params)?;
    let call = api.call(actor);
    let projected = api.service.level(&call, &id, &query).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/trace/{key}` (C7).
pub async fn trace(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path((id, key)): Path<(String, String)>,
) -> Projection<Trace> {
    let (id, key) = (journey(&id)?, node(&key)?);
    let projected = api.service.trace(&api.call(actor), &id, &key).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/derived` (D3): the whole derive, which the browser computes itself from
/// the domain document; for a client without the wasm engine.
pub async fn derived(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Projection<Derived> {
    let id = journey(&id)?;
    let projected = api.service.derived(&api.call(actor), &id).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/decisions` (C12).
pub async fn decisions(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Projection<DecisionView> {
    let id = journey(&id)?;
    let projected = api.service.decision_view(&api.call(actor), &id).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/timeline` (C13).
pub async fn timeline(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Projection<Timeline> {
    let id = journey(&id)?;
    let projected = api.service.timeline(&api.call(actor), &id).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/summary` (C18).
pub async fn summary(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Projection<StatusSummary> {
    let id = journey(&id)?;
    let projected = api.service.status_summary(&api.call(actor), &id).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/next` (C10).
pub async fn next(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Projection<Next> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::NEXT_PARAMS)?;
    let asked = query::next(&params)?;
    let projected = api.service.next(&api.call(actor), &id, &asked).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/nodes` (C9).
pub async fn nodes(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Projection<ListPage> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::LIST_PARAMS)?;
    let asked = query::list(&params)?;
    let projected = api.service.list(&api.call(actor), &id, &asked).await?;
    params.page_at(projected.revision)?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/mine` (E4).
pub async fn mine(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Projection<Mine> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::MINE_PARAMS)?;
    let kinds = query::mine(&params)?;
    let projected = api.service.mine(&api.call(actor), &id, &kinds).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/nodes/{key}` (C8): explanation lists capped with their totals.
pub async fn node_detail(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path((id, key)): Path<(String, String)>,
) -> Projection<NodeDetail> {
    let (id, key) = (journey(&id)?, node(&key)?);
    let projected = api.service.node_detail(&api.call(actor), &id, &key).await?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/nodes/{key}/removal` (A18): the node's removal as the journey
/// stands, with the references elsewhere that the same patch must change.
pub async fn node_removal(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path((id, key)): Path<(String, String)>,
    RawQuery(raw): RawQuery,
) -> Projection<RemovalRead> {
    let (id, key) = (journey(&id)?, node(&key)?);
    let params = Params::parse(raw.as_deref(), query::EXPLANATION_PARAMS)?;
    let cursor = params.cursor()?;
    let projected = api
        .service
        .node_removal(&api.call(actor), &id, &key, cursor)
        .await?;
    params.page_at(projected.revision)?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/nodes/{key}/explanations/{field}`: a page of one explanation list,
/// largest first; the first page is what node detail carries.
pub async fn explanations(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path((id, key, field)): Path<(String, String, String)>,
    RawQuery(raw): RawQuery,
) -> Projection<ExplanationPage> {
    let (id, key) = (journey(&id)?, node(&key)?);
    let field: ExplainedField = segment(&field, "explained field")?;
    let params = Params::parse(raw.as_deref(), query::EXPLANATION_PARAMS)?;
    let cursor = params.cursor()?;
    let call = api.call(actor);
    let projected = api
        .service
        .explanations(&call, &id, &key, field, cursor)
        .await?;
    params.page_at(projected.revision)?;
    Ok(Json(Projected::from_service(projected)))
}

/// `GET /api/journeys/{id}/history` (J4).
pub async fn history(
    State(api): State<Api>,
    Path(id): Path<String>,
    RawQuery(raw): RawQuery,
) -> Answer<History> {
    let id = journey(&id)?;
    let params = Params::parse(raw.as_deref(), query::HISTORY_PARAMS)?;
    let (node, after) = query::history(&params)?;
    let history = api.service.history(&id, node.as_ref(), after).await?;
    Ok(Json(history.into()))
}
