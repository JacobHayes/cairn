//! The request and response bodies only the API has: the assistant's and the agent tokens'.
//! Every body both hosts answer with lives in the service's `wire` module.

mod assistant;
mod tokens;

pub use assistant::{
    AssistantRequest, Conversation, ConversationAuthor, ConversationEntry, TurnReply,
};
pub use tokens::{AgentToken, MintedToken, TokenRequest};
