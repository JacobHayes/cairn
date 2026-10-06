//! The assistant's bodies (I5): what the user says, and the turn's answer, which is the
//! assistant crate's own type so the wire and the turn cannot drift.

use cairn_schema::Markdown;
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
