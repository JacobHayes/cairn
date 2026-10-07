//! The caller (H2, H3) and their agent tokens, which auth mints, lists, and revokes.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use cairn_schema::{Actor, AgentId};
use cairn_store::Store;

use super::Answer;
use crate::Api;
use crate::error::ApiError;
use crate::extract::{JsonBody, Path, segment};
use crate::wire::{AgentToken, MintedToken, TokenRequest, Viewer};

/// `GET /users/me` (H2, H3).
pub async fn viewer<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
) -> Answer<Viewer> {
    let agent = actor.agent.clone();
    let viewer = api.service.viewer(&api.call(actor)).await?;
    let merge_offer = viewer.merge_offer().cloned();
    Ok(Json(Viewer {
        user: viewer.user,
        agent,
        entities: viewer.entities,
        merge_offer,
        identities: viewer.identities.into_iter().map(Into::into).collect(),
    }))
}

/// `GET /users/me/tokens`.
pub async fn tokens<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
) -> Answer<Vec<AgentToken>> {
    let tokens = api.auth.accounts().tokens_of(&actor.user).await?;
    Ok(Json(tokens.into_iter().map(Into::into).collect()))
}

/// `POST /users/me/tokens` (H2): only a user mints one.
pub async fn mint_token<S: Store + 'static>(
    State(api): State<Api<S>>,
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

/// `DELETE /users/me/tokens/{agent}`: only a user revokes one, and only their own.
pub async fn revoke_token<S: Store + 'static>(
    State(api): State<Api<S>>,
    Extension(actor): Extension<Actor>,
    Path(agent): Path<String>,
) -> Result<StatusCode, ApiError> {
    let agent: AgentId = segment(&agent, "agent")?;
    api.auth.accounts().revoke_token(&actor, &agent).await?;
    Ok(StatusCode::NO_CONTENT)
}
