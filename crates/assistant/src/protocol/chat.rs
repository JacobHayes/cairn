//! A generic OpenAI-compatible chat-completions endpoint (`OpenRouter`, self-hosted models):
//! `POST {endpoint}/chat/completions`, with a bearer key when one is configured. The system
//! prompt is the first message; a tool call is an entry of the assistant message's
//! `tool_calls` with its arguments as JSON text, and its result a `tool` message. Servers
//! differ in what else they put on a message, so a reply is sent back rebuilt from Cairn's
//! types rather than as it came.

use serde_json::{Value, json};

use super::{REPLY_TOKEN_COUNT_MAX, arguments_as_text, arguments_from_text, malformed};
use crate::provider::{Exchange, Message, ProviderError, Reply, Stop, ToolCall, Usage};

/// The path under the endpoint.
pub const PATH: &str = "chat/completions";

/// The request body for `exchange`.
#[must_use]
pub fn request(model: &str, exchange: &Exchange) -> Value {
    let tools: Vec<Value> = exchange
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                },
            })
        })
        .collect();
    let mut messages = vec![json!({ "role": "system", "content": exchange.system })];
    for message in &exchange.messages {
        entries(message, &mut messages);
    }
    json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        // The field every OpenAI-compatible server reads; OpenAI's own reasoning models want
        // `max_completion_tokens` instead, and a deployment on OpenAI uses the Responses protocol.
        "max_tokens": REPLY_TOKEN_COUNT_MAX,
    })
}

/// The chat messages of one message.
fn entries(message: &Message, into: &mut Vec<Value>) {
    match message {
        Message::User { text } => into.push(json!({ "role": "user", "content": text })),
        Message::Assistant(reply) => {
            let mut entry = json!({ "role": "assistant", "content": reply.text });
            if !reply.calls.is_empty() {
                let calls: Vec<Value> = reply
                    .calls
                    .iter()
                    .map(|call| {
                        json!({
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": arguments_as_text(&call.arguments),
                            },
                        })
                    })
                    .collect();
                entry["tool_calls"] = Value::Array(calls);
            }
            into.push(entry);
        }
        Message::ToolResults(results) => into.extend(results.iter().map(|result| {
            json!({
                "role": "tool",
                "tool_call_id": result.call_id,
                "content": result.content,
            })
        })),
    }
}

/// The reply in an answer: the first choice's message.
///
/// # Errors
///
/// [`ProviderError::Failed`] when there is no choice with a message, or a call is malformed.
pub fn reply(answer: &Value) -> Result<Reply, ProviderError> {
    let choice = answer
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());
    let message = choice
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| malformed("no choice with a message"))?;
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    let mut calls = Vec::new();
    let listed = message.get("tool_calls").and_then(Value::as_array);
    for call in listed.into_iter().flatten() {
        let function = call.get("function");
        let field = |of: Option<&Value>, name: &str| {
            of.and_then(|of| of.get(name))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let (Some(id), Some(name), Some(arguments)) = (
            field(Some(call), "id"),
            field(function, "name"),
            field(function, "arguments"),
        ) else {
            return Err(malformed("a tool call without its id, name, or arguments"));
        };
        calls.push(ToolCall {
            id,
            name,
            arguments: arguments_from_text(&arguments),
        });
    }
    Ok(Reply {
        text,
        calls,
        raw: None,
        usage: usage(answer),
        stop: stop(choice),
    })
}

/// The tokens an answer reports. Its `prompt_tokens` counts the cached ones and the ones
/// written to the cache, which are taken out; a server that reports cache writes
/// (`OpenRouter`) says so beside the reads.
fn usage(answer: &Value) -> Usage {
    let usage = answer.get("usage");
    let count = |of: Option<&Value>, name: &str| {
        of.and_then(|of| of.get(name))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let details = usage.and_then(|usage| usage.get("prompt_tokens_details"));
    let cached = count(details, "cached_tokens");
    let written = count(details, "cache_write_tokens");
    Usage {
        input: count(usage, "prompt_tokens").saturating_sub(cached + written),
        output: count(usage, "completion_tokens"),
        cache_read: cached,
        cache_write: written,
    }
}

/// Why the choice stopped, by its `finish_reason`.
fn stop(choice: Option<&Value>) -> Stop {
    match choice
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
    {
        Some("length") => Stop::OutputLimit,
        Some("content_filter") => Stop::Refused,
        _ => Stop::Done,
    }
}
