//! The MCP endpoint's tools (ARCHITECTURE, MCP endpoint; PRD I2, I3, I7): a curated set for
//! an agent's loop rather than a mirror of the HTTP API. Each tool maps to one service
//! operation, shares the schema types for its arguments, bounds its output (a list that can
//! grow is paged), and answers a refused write with the engine's rejection unchanged (A15).
//!
//! [`ToolSet::call`] runs a tool in process as an actor: the assistant (4.4) uses it
//! directly, and [`router`] serves the same set over Streamable HTTP at [`MCP_PATH`], with
//! the shipped instructions (I4) as its instructions and prompts.

#![forbid(unsafe_code)]

mod error;
pub mod instructions;
pub mod limits;
mod server;
mod tools;
mod toolset;

pub use error::ToolError;
pub use server::{MCP_PATH, TOOL_CALL_DURATION, TOOL_CALLS, UNKNOWN_TOOL, router};
pub use toolset::{ToolDefinition, ToolSet};
