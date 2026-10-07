//! The assistant's endpoints (I5; ARCHITECTURE, Service layer and composition: an optional
//! subsystem mounts itself): served only when the host's root assembled an assistant, each
//! one turn of the caller's conversation about a journey or a route's draft. A turn waits on
//! the model provider, so these sit outside the API's request duration and are held to the
//! assistant's own turn limits instead.

use axum::Json;
use axum::extract::{Extension, State};
use axum::routing::post;
use cairn_assistant::{Assistant, Target, TurnError};
use cairn_schema::{Actor, JourneyId, RouteId};
use cairn_store::Store;

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::wire::{AssistantRequest, ProblemCode, TurnReply};

/// The assistant's endpoints and handlers.
pub fn served<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    vec![
        (&at::ASSISTANT_JOURNEY, post(journey::<S>)),
        (&at::ASSISTANT_ROUTE_DRAFT, post(route_draft::<S>)),
    ]
}

/// `POST /journeys/{id}/assistant`.
pub async fn journey<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<AssistantRequest>,
) -> Answer<TurnReply> {
    let id: JourneyId = segment(&id, "journey")?;
    converse(&api, &actor, Target::Journey(id), request).await
}

/// `POST /routes/{id}/draft/assistant`.
pub async fn route_draft<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<AssistantRequest>,
) -> Answer<TurnReply> {
    let id: RouteId = segment(&id, "route")?;
    converse(&api, &actor, Target::RouteDraft(id), request).await
}

/// One turn as `actor`.
async fn converse<S: Store + 'static>(
    api: &Api<S>,
    actor: &Actor,
    target: Target,
    request: AssistantRequest,
) -> Answer<TurnReply> {
    let Some(assistant): Option<&Assistant<S>> = api.assistant.as_ref() else {
        // Mounted only beside an assistant: a request here is a programmer error.
        return Err(ApiError::problem(
            ProblemCode::NoSuchEndpoint,
            "no assistant",
        ));
    };
    let reply = assistant.turn(actor, target, request.message).await;
    reply.map(Json).map_err(|error| {
        let code = match &error {
            TurnError::Overloaded | TurnError::ConversationBusy => ProblemCode::Overloaded,
            TurnError::AgentCaller => ProblemCode::UserOnly,
            TurnError::TargetMissing(_) => ProblemCode::NotFound,
            TurnError::TimedOut => ProblemCode::TimedOut,
            TurnError::Failed(_) => ProblemCode::Internal,
        };
        ApiError::problem(code, error.to_string())
    })
}
