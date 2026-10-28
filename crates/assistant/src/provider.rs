//! The model provider (ARCHITECTURE, Assistant; PRD I5): a trait in Cairn's own types,
//! vendor-neutral. A provider is sent a conversation with tool definitions and answers text
//! and tool calls; each implementation translates to one wire protocol
//! ([`crate::protocol`]), and a scripted one drives the tests ([`crate::scripted`]).

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What one model call is sent.
#[derive(Clone, Debug, PartialEq)]
pub struct Exchange {
    /// The instructions, the same on every request so a provider can cache them: the write
    /// policy, the shipped guide, and the workflows. The turn's context leads its user message.
    pub system: String,
    /// The conversation so far, oldest first; it ends with the user's message or tool results.
    pub messages: Vec<Message>,
    /// The tools the model may call.
    pub tools: Vec<ToolSpec>,
}

/// A tool as a model sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSpec {
    /// Its name.
    pub name: String,
    /// What it does.
    pub description: String,
    /// The JSON Schema of its arguments: an object.
    pub parameters: Map<String, Value>,
}

/// One message of a conversation as a provider is sent it.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// The user's words.
    User {
        /// What they said.
        text: String,
    },
    /// What the model answered: text, tool calls, or both.
    Assistant(Reply),
    /// The results of the tool calls of the message before, one per call, in its order.
    ToolResults(Vec<ToolResult>),
}

/// A model's answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    /// What it said, if anything.
    pub text: Option<String>,
    /// The tools it called, in order; none ends the turn.
    pub calls: Vec<ToolCall>,
    /// The answer as the protocol gave it, for that protocol to send back unchanged when the
    /// turn continues (reasoning a provider asks to see again rides here); `None` when the
    /// message was built from Cairn's types alone. Never stored.
    pub raw: Option<Value>,
    /// The tokens the call used, as the provider counted them.
    pub usage: Usage,
    /// Why the model stopped.
    pub stop: Stop,
}

/// The tokens one provider call used. `input` counts what was neither read from nor written
/// to the provider's prompt cache, so the four add up to the call's whole input on every
/// protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens neither read from nor written to the cache.
    pub input: u64,
    /// Output tokens, reasoning included.
    pub output: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write: u64,
}

/// Why a reply ended. Anything but [`Stop::Done`] means the reply is not what the model meant
/// to send, and none of its tool calls run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stop {
    /// The model finished: it answered, or called tools and waits for their results.
    #[default]
    Done,
    /// The reply hit the output token cap and is cut off.
    OutputLimit,
    /// The model declined to answer.
    Refused,
    /// The provider paused the turn, which asks to be continued.
    Paused,
}

/// One tool call.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    /// The provider's id for the call, which its result names.
    pub id: String,
    /// The tool.
    pub name: String,
    /// Its arguments: a JSON object, or whatever the model wrote, which the tool refuses.
    pub arguments: Value,
}

/// The result of one tool call.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolResult {
    /// The call it answers.
    pub call_id: String,
    /// What the tool answered, as JSON text.
    pub content: String,
    /// Whether the call failed: the content says why.
    pub is_error: bool,
}

/// Why a provider call did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// The call ran past its limit, or the provider could not be reached in time.
    TimedOut,
    /// The provider answered with an error, or with something that is not its protocol.
    Failed {
        /// What it said, without the credential.
        message: String,
    },
    /// The provider answered with an HTTP error status.
    Rejected {
        /// The status code.
        status: u16,
        /// What it said, without the credential.
        message: String,
    },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::TimedOut => {
                formatter.write_str("the model provider did not answer in time")
            }
            ProviderError::Failed { message } => {
                write!(formatter, "the model provider failed: {message}")
            }
            ProviderError::Rejected { status, message } => {
                write!(
                    formatter,
                    "the model provider failed (status {status}): {message}"
                )
            }
        }
    }
}

impl std::error::Error for ProviderError {}

/// A model provider (I5: deployment-configurable, not tied to one vendor). The turn loop
/// holds the call to [`PROVIDER_CALL_DURATION_MAX`](crate::limits::PROVIDER_CALL_DURATION_MAX),
/// so an implementation need not time itself.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Sends `exchange` and answers the model's reply.
    async fn send(&self, exchange: &Exchange) -> Result<Reply, ProviderError>;
}

/// The wire protocols a deployment can configure (ARCHITECTURE, Assistant).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// The Anthropic Messages API, with an API key.
    AnthropicMessages,
    /// The `OpenAI` Responses API, with an API key.
    OpenaiResponses,
    /// A generic OpenAI-compatible chat-completions endpoint (`OpenRouter`, self-hosted
    /// models), with a bearer key when it asks for one.
    ChatCompletions,
}
