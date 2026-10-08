//! The assistant's bodies (I5): what the user says, the turn's answer, which is the
//! assistant crate's own type so the wire and the turn cannot drift, and the conversation as
//! kept, for a panel opened again.

use cairn_schema::{ConversationId, Markdown, Timestamp};
use cairn_store::MessageAuthor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use cairn_assistant::TurnReply;

/// `POST /journeys/{id}/assistant` and `POST /routes/{id}/draft/assistant`: one message of
/// the caller's conversation about that journey or draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssistantRequest {
    /// What the user says.
    pub message: Markdown,
}

/// `GET /journeys/{id}/assistant` and `GET /routes/{id}/draft/assistant`: the caller's
/// conversation about that journey or draft (I5: one per target per user), as kept: its
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

/// One kept message of a conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConversationEntry {
    /// Who wrote it.
    pub author: ConversationAuthor,
    /// When it was kept.
    pub at: Timestamp,
    /// What it says.
    pub content: Markdown,
}

/// Who wrote a conversation message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConversationAuthor {
    /// The user.
    User,
    /// The assistant.
    Assistant,
    /// Cairn, reporting a write the assistant made or how a turn ended.
    Cairn,
}

impl From<cairn_store::ConversationMessage> for ConversationEntry {
    fn from(message: cairn_store::ConversationMessage) -> Self {
        let cairn_store::ConversationMessage {
            author,
            at,
            content,
        } = message;
        let author = match author {
            MessageAuthor::User => ConversationAuthor::User,
            MessageAuthor::Assistant => ConversationAuthor::Assistant,
            MessageAuthor::Tool => ConversationAuthor::Cairn,
        };
        Self {
            author,
            at,
            content,
        }
    }
}
