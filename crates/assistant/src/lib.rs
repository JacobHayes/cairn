//! The in-app assistant (ARCHITECTURE, Assistant; PRD I5, I7, H2): optional, and the same
//! thing as an external agent with a different transport. A turn runs a server-side tool
//! loop over the MCP tool set in process ([`Assistant::turn`]), as the assistant acting for
//! its user, with the same tools and guide as any agent. Whether a change is a state write
//! or a proposal is the guide's call; the turn reports each write from the tool's own answer.
//!
//! The model provider is one OpenAI-compatible chat-completions endpoint ([`chat`]). Its
//! credential is configuration and is never stored.

#![forbid(unsafe_code)]

pub mod chat;
pub mod conversation;
pub mod limits;
mod receipts;
pub mod tool_results;
mod turn;

pub use conversation::{ConversationAuthor, ConversationEntry, Target};
pub use receipts::{Proposed, Write};
pub use turn::{
    ASSISTANT_AGENT, Assistant, Ended, PROVIDER_TOKENS_TOTAL, TURNS_TOTAL, TurnError, TurnReply,
    assistant_agent,
};

/// `bytes` as lowercase hex digits.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
