//! The caller (H2, H3) and their agent tokens, which auth mints, lists, and revokes.

use axum::Json;
use axum::extract::{Extension, RawQuery, State};
use axum::http::StatusCode;
use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
use cairn_schema::{Actor, AgentId, UserId};

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::query::{self, Params};
use crate::wire::{AgentToken, MintedToken, TokenRequest, UserName, Viewer};

/// `GET /api/users/me` (H2, H3).
pub async fn viewer(State(api): State<Api>, Extension(actor): Extension<Actor>) -> Answer<Viewer> {
    let agent = actor.agent.clone();
    let viewer = api.service.viewer(&api.call(actor)).await?;
    let merge_offer = viewer.merge_offer().cloned();
    Ok(Json(Viewer {
        user: viewer.user,
        agent,
        identities: viewer.identities,
        merge_offer,
        logins: viewer.logins.into_iter().map(Into::into).collect(),
    }))
}

/// `GET /api/users` (H3): the names of the users asked for, at most a page of them.
pub async fn user_names(RawQuery(raw): RawQuery, State(api): State<Api>) -> Answer<Vec<UserName>> {
    let params = Params::parse(raw.as_deref(), query::USER_NAME_PARAMS)?;
    let users: Vec<UserId> = params.all("id")?;
    if users.len() > usize::try_from(PAGE_ITEM_COUNT_MAX).unwrap_or(usize::MAX) {
        return Err(ApiError::bad_request(format!(
            "at most {PAGE_ITEM_COUNT_MAX} users can be named at once"
        )));
    }
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
