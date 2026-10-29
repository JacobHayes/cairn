//! The assistant's bodies (I5): what the user says, the turn's answer, which is the
//! assistant crate's own type so the wire and the turn cannot drift, and the conversation as
//! kept, for a panel opened again.

use cairn_schema::{ConversationId, Markdown};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use cairn_assistant::{ConversationAuthor, ConversationEntry, TurnReply};

/// `POST /api/journeys/{id}/assistant` and `POST /api/routes/{id}/draft/assistant`: one message of
/// the caller's conversation about that journey or draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantRequest {
    /// What the user says.
    pub message: Markdown,
}

// PRD: I5.
/// `GET /api/journeys/{id}/assistant` and `GET /api/routes/{id}/draft/assistant`: the caller's
/// conversation about that journey or draft (one per target per user), as kept: its
/// newest messages, oldest first. A conversation not started yet has none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
    /// The conversation's id, which every turn about this target answers too.
    pub conversation: ConversationId,
    /// Its messages, oldest first.
    #[serde(default)]
    pub messages: Vec<ConversationEntry>,
}
