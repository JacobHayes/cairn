//! The agent-token bodies: the one wire shapes that stay with the API, since they come from
//! the sign-in crate the service does not depend on.

use cairn_schema::{AgentId, Timestamp, Title};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `POST /api/users/me/tokens`: what to call the new agent token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    /// Its name, as the user's list shows it.
    pub name: Title,
}

// PRD: H2.
/// A token just minted: the one time its secret is seen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MintedToken {
    /// The agent the token acts as.
    pub agent: AgentId,
    /// The bearer token; Cairn keeps only its digest.
    pub token: String,
}

/// An agent token as its user's list shows it: never the secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentToken {
    /// The agent the token acts as.
    pub agent: AgentId,
    /// What the user called it.
    pub name: Title,
    /// When it was minted.
    pub created_at: Timestamp,
    /// When it was revoked, if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<Timestamp>,
}

impl From<cairn_auth::AgentToken> for AgentToken {
    fn from(token: cairn_auth::AgentToken) -> Self {
        let cairn_auth::AgentToken {
            agent,
            name,
            created_at,
            revoked_at,
        } = token;
        Self {
            agent,
            name,
            created_at,
            revoked_at,
        }
    }
}
