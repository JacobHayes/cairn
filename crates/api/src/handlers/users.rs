//! The caller (H2, H3) and their agent tokens, which auth mints, lists, and revokes.

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use cairn_schema::{Actor, AgentId, UserId};
use cairn_store::TourMark;

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::query::{self, Params};
use crate::wire::{AgentToken, MintedToken, TokenRequest};
use cairn_service::wire::{Caller, ProblemCode, Tour, TourWrite, TourWriteState, UserName};

/// `GET /api/users/me` (H2, H3).
pub async fn caller(State(api): State<Api>, Extension(actor): Extension<Actor>) -> Answer<Caller> {
    let agent = actor.agent.clone();
    let caller = api.service.caller(&api.call(actor)).await?;
    Ok(Json(Caller::of(caller, agent)))
}

/// `GET /api/users` (H3): the names of the users asked for, at most a page of them.
pub async fn user_names(RawQuery(raw): RawQuery, State(api): State<Api>) -> Answer<Vec<UserName>> {
    let params = Params::parse(raw.as_deref(), query::USER_NAME_PARAMS)?;
    let users: Vec<UserId> = params.all("id")?;
    let names = api.service.user_names(&users).await?;
    Ok(Json(names.into_iter().map(Into::into).collect()))
}

/// `GET /api/users/me/tokens`.
pub async fn tokens(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
) -> Answer<Vec<AgentToken>> {
    let tokens = api.auth.accounts().tokens_of(&actor.user).await?;
    Ok(Json(tokens.into_iter().map(Into::into).collect()))
}

/// `POST /api/users/me/tokens` (H2): only a user mints one.
pub async fn mint_token(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    JsonBody(request): JsonBody<TokenRequest>,
) -> Result<(StatusCode, Json<MintedToken>), ApiError> {
    if !api.service.capabilities().mcp.tokens {
        return Err(ApiError::problem(
            ProblemCode::TokensNotAccepted,
            "this deployment does not accept agent tokens",
        ));
    }
    let minted = api.auth.accounts().mint_token(&actor, request.name).await?;
    let token = MintedToken {
        agent: minted.agent,
        token: minted.token.expose().to_owned(),
    };
    Ok((StatusCode::CREATED, Json(token)))
}

/// `DELETE /api/users/me/tokens/{agent}`: only a user revokes one, and only their own.
pub async fn revoke_token(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    Path(agent): Path<String>,
) -> Result<StatusCode, ApiError> {
    let agent: AgentId = segment(&agent, "agent")?;
    api.auth.accounts().revoke_token(&actor, &agent).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/users/me/tour` (the tour): the caller's place in it, and today.
pub async fn tour(State(api): State<Api>, Extension(actor): Extension<Actor>) -> Answer<Tour> {
    let view = api.service.tour(&api.call(actor)).await?;
    Ok(Json(view.into()))
}

/// `PUT /api/users/me/tour` (the tour): moves the caller's place only over the record the
/// write expects; 409 with the current record otherwise.
pub async fn put_tour(
    State(api): State<Api>,
    Extension(actor): Extension<Actor>,
    JsonBody(write): JsonBody<TourWrite>,
) -> Result<Response, ApiError> {
    let started = write.state == TourWriteState::Started;
    if started != write.practice_journey.is_some() {
        return Err(ApiError::bad_request(
            "a started tour names its practice journey, and a finished or dismissed one names none",
        ));
    }
    let expect_state = write.expect.state.state();
    if expect_state.is_some_and(|state| state == cairn_store::TourState::Started)
        != write.expect.practice_journey.is_some()
    {
        return Err(ApiError::bad_request(
            "the expected record names a practice journey exactly when it is started",
        ));
    }
    let expect = expect_state.map(|state| TourMark {
        state,
        practice_journey: write.expect.practice_journey,
    });
    let call = api.call(actor);
    let written = api
        .service
        .put_tour(&call, write.state.into(), write.practice_journey, expect)
        .await?;
    match written {
        Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(current) => {
            let today = api.service.tour(&call).await?.today;
            let view = cairn_service::TourView {
                record: current,
                today,
            };
            Ok((StatusCode::CONFLICT, Json(Tour::from(view))).into_response())
        }
    }
}
