//! The in-app assistant (ARCHITECTURE, Assistant; PRD I5, I7, H2): optional, and the same
//! thing as an external agent with a different transport. A turn runs a server-side tool
//! loop over the MCP tool set in process ([`Assistant::turn`]), as the assistant acting for
//! its user, and every tool call passes through one write wrapper ([`wrapper`]): structural
//! changes become one proposal for the user to review; state changes apply directly, like any
//! external agent's, and are reported with their consequences.
//!
//! The model provider is a trait in Cairn's own types ([`Provider`]) with three wire
//! protocols ([`protocol`]) and a scripted provider for tests ([`scripted`]). The
//! deployment's credential is configuration and is never stored.

#![forbid(unsafe_code)]

pub mod conversation;
pub mod limits;
pub mod protocol;
pub mod provider;
mod said;
pub mod scripted;
pub mod tool_results;
mod touched;
mod turn;
pub mod wrapper;

pub use conversation::Target;
pub use provider::{
    Exchange, Message, Protocol, Provider, ProviderError, Reply, Stop, ToolCall, ToolResult,
    ToolSpec, Usage,
};
pub use turn::{
    ASSISTANT_AGENT, Assistant, Ended, PROVIDER_TOKENS_TOTAL, TURNS_TOTAL, TurnError, TurnReply,
    assistant_agent,
};
pub use wrapper::{Action, AppliedReceipt, Because};

/// `bytes` as lowercase hex digits.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
