//! The assistant's endpoints (I5; ARCHITECTURE, Service layer and composition: an optional
//! subsystem mounts itself): served only when the host's root assembled an assistant, each
//! one turn of the caller's conversation about a journey or a route's draft. A turn waits on
//! the model provider, so these sit outside the API's request duration and are held to the
//! assistant's own turn limits instead. Reading a conversation back is an ordinary read,
//! inside the request duration.

use axum::Json;
use axum::extract::{Extension, State};
use axum::routing::{get, post};
use cairn_assistant::{Assistant, Target, TurnError};
use cairn_schema::{Actor, JourneyId, RouteId};
use cairn_store::Store;

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::wire::{AssistantRequest, Conversation, ProblemCode, TurnReply};

/// The conversation reads and their handlers, held to the request duration.
pub fn reads<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    vec![
        (
            &at::ASSISTANT_JOURNEY_CONVERSATION,
            get(journey_conversation::<S>),
        ),
        (
            &at::ASSISTANT_ROUTE_DRAFT_CONVERSATION,
            get(route_draft_conversation::<S>),
        ),
    ]
}

/// The turn endpoints and their handlers, outside the request duration.
pub fn served<S: Store + 'static>() -> crate::Served<S> {
    use crate::endpoints as at;
    vec![
        (&at::ASSISTANT_JOURNEY, post(journey::<S>)),
        (&at::ASSISTANT_ROUTE_DRAFT, post(route_draft::<S>)),
    ]
}

/// `POST /api/journeys/{id}/assistant`.
pub async fn journey<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<AssistantRequest>,
) -> Answer<TurnReply> {
    let id: JourneyId = segment(&id, "journey")?;
    converse(&api, &actor, Target::Journey(id), request).await
}

/// `POST /api/routes/{id}/draft/assistant`.
pub async fn route_draft<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
    JsonBody(request): JsonBody<AssistantRequest>,
) -> Answer<TurnReply> {
    let id: RouteId = segment(&id, "route")?;
    converse(&api, &actor, Target::RouteDraft(id), request).await
}

/// `GET /api/journeys/{id}/assistant`.
pub async fn journey_conversation<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Answer<Conversation> {
    let id: JourneyId = segment(&id, "journey")?;
    kept(&api, &actor, &Target::Journey(id)).await
}

/// `GET /api/routes/{id}/draft/assistant`.
pub async fn route_draft_conversation<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(id): Path<String>,
) -> Answer<Conversation> {
    let id: RouteId = segment(&id, "route")?;
    kept(&api, &actor, &Target::RouteDraft(id)).await
}

/// The conversation `actor`'s user holds about `target`, as kept; none started yet is empty.
async fn kept<S: Store + 'static>(
    api: &Api<S>,
    actor: &Actor,
    target: &Target,
) -> Answer<Conversation> {
    let assistant = mounted(api)?;
    if actor.agent.is_some() {
        return Err(ApiError::problem(
            ProblemCode::UserOnly,
            TurnError::AgentCaller.to_string(),
        ));
    }
    let record = assistant
        .conversation(actor, target)
        .await
        .map_err(|error| ApiError::problem(ProblemCode::Internal, error.to_string()))?;
    Ok(Json(Conversation {
        conversation: cairn_assistant::conversation::conversation_id(target, &actor.user),
        messages: record
            .map(|record| record.messages.into_iter().map(Into::into).collect())
            .unwrap_or_default(),
    }))
}

/// The assistant, which these endpoints are mounted only beside.
fn mounted<S: Store + 'static>(api: &Api<S>) -> Result<&Assistant<S>, ApiError> {
    api.assistant.as_ref().ok_or_else(|| {
        // Mounted only beside an assistant: a request here is a programmer error.
        ApiError::problem(ProblemCode::NoSuchEndpoint, "no assistant")
    })
}

/// One turn as `actor`.
async fn converse<S: Store + 'static>(
    api: &Api<S>,
    actor: &Actor,
    target: Target,
    request: AssistantRequest,
) -> Answer<TurnReply> {
    let assistant = mounted(api)?;
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
