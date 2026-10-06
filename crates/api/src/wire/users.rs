//! The caller and their agent tokens (H2, H3).

use std::collections::BTreeSet;

use cairn_schema::{AgentId, EntityKey, Timestamp, Title, UserId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The caller (H2) and the entities their verified emails name (H3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Viewer {
    /// The user.
    pub user: UserId,
    /// The agent acting for them, when the request carries an agent token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    /// Their entities, which "mine" covers together.
    pub entities: BTreeSet<EntityKey>,
    /// H3: present when their emails name more than one entity, which are then offered for
    /// merging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_offer: Option<BTreeSet<EntityKey>>,
}

/// `POST /users/me/tokens`: what to call the new agent token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    /// Its name, as the user's list shows it.
    pub name: Title,
}

/// A token just minted: the one time its secret is seen (H2).
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
