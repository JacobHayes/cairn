//! The `OpenAI` Responses API: `POST {endpoint}/responses` with a bearer key. The system
//! prompt is `instructions`; the conversation is a list of input items, a tool call a
//! `function_call` item with its arguments as JSON text, and its result a
//! `function_call_output` item. Nothing is stored at the provider (`store: false`), so the
//! reasoning items of a reply are asked for encrypted and sent back unchanged within a turn;
//! a model that does not reason refuses being asked, which [`super::HttpProvider`] learns and
//! stops doing.

use serde_json::{Value, json};

use super::{REPLY_TOKEN_COUNT_MAX, arguments_as_text, arguments_from_text, malformed};
use crate::provider::{Exchange, Message, ProviderError, Reply, Stop, ToolCall, Usage};

/// The path under the endpoint.
pub const PATH: &str = "responses";

/// The key that groups this assistant's requests, which share their leading text.
const PROMPT_CACHE_KEY: &str = "cairn-assistant";

/// The request body for `exchange`, asking for the reply's reasoning encrypted when
/// `encrypted_reasoning`.
#[must_use]
pub fn request(model: &str, exchange: &Exchange, encrypted_reasoning: bool) -> Value {
    let tools: Vec<Value> = exchange
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
                "strict": false,
            })
        })
        .collect();
    let mut input = Vec::new();
    for message in &exchange.messages {
        items(message, &mut input);
    }
    let mut body = json!({
        "model": model,
        "instructions": exchange.system,
        "input": input,
        "tools": tools,
        "store": false,
        // Prefixes cache automatically; the key routes requests that share one to the same
        // cache.
        "prompt_cache_key": PROMPT_CACHE_KEY,
        "max_output_tokens": REPLY_TOKEN_COUNT_MAX,
    });
    if encrypted_reasoning {
        body["include"] = json!(["reasoning.encrypted_content"]);
    }
    body
}

/// The input items of one message.
fn items(message: &Message, into: &mut Vec<Value>) {
    match message {
        Message::User { text } => into.push(json!({
            "type": "message",
            "role": "user",
            "content": [{ "type": "input_text", "text": text }],
        })),
        Message::Assistant(reply) => {
            if let Some(Value::Array(output)) = &reply.raw {
                into.extend(output.iter().cloned());
                return;
            }
            into.extend(reply.text.iter().map(|text| {
                json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": text }],
                })
            }));
            into.extend(reply.calls.iter().map(|call| {
                json!({
                    "type": "function_call",
                    "call_id": call.id,
                    "name": call.name,
                    "arguments": arguments_as_text(&call.arguments),
                })
            }));
        }
        Message::ToolResults(results) => into.extend(results.iter().map(|result| {
            json!({
                "type": "function_call_output",
                "call_id": result.call_id,
                "output": result.content,
            })
        })),
    }
}

/// The reply in an answer: the text of its `message` items, its `function_call` items as
/// calls.
///
/// # Errors
///
/// [`ProviderError::Failed`] when the response failed or has no output list, or an item is
/// malformed.
pub fn reply(answer: &Value) -> Result<Reply, ProviderError> {
    if answer.get("status").and_then(Value::as_str) == Some("failed") {
        return Err(ProviderError::Failed {
            message: super::error_message(answer),
        });
    }
    let Some(output) = answer.get("output").and_then(Value::as_array) else {
        return Err(malformed("no output list"));
    };
    let mut texts = Vec::new();
    let mut calls = Vec::new();
    let mut refused = false;
    for item in output {
        let field = |name: &str| item.get(name).and_then(Value::as_str);
        match field("type") {
            Some("message") => {
                let content = item.get("content").and_then(Value::as_array);
                let parts = content
                    .into_iter()
                    .flatten()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("output_text"));
                refused |= content
                    .into_iter()
                    .flatten()
                    .any(|part| part.get("type").and_then(Value::as_str) == Some("refusal"));
                for part in parts {
                    let text = part.get("text").and_then(Value::as_str);
                    texts.push(text.ok_or_else(|| malformed("output text without text"))?);
                }
            }
            Some("function_call") => {
                let (Some(id), Some(name), Some(arguments)) =
                    (field("call_id"), field("name"), field("arguments"))
                else {
                    return Err(malformed(
                        "a function call without its id, name, or arguments",
                    ));
                };
                calls.push(ToolCall {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    arguments: arguments_from_text(arguments),
                });
            }
            // Reasoning and items Cairn does not read ride back in `raw`.
            _ => {}
        }
    }
    let text = (!texts.is_empty()).then(|| texts.concat());
    Ok(Reply {
        text,
        calls,
        raw: Some(Value::Array(output.clone())),
        usage: usage(answer),
        stop: stop(answer, refused),
    })
}

/// The tokens an answer reports. Its `input_tokens` counts the cached ones, which are
/// taken out; this API has no cache write to report.
fn usage(answer: &Value) -> Usage {
    let usage = answer.get("usage");
    let count = |of: Option<&Value>, name: &str| {
        of.and_then(|of| of.get(name))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let cached = count(
        usage.and_then(|usage| usage.get("input_tokens_details")),
        "cached_tokens",
    );
    Usage {
        input: count(usage, "input_tokens").saturating_sub(cached),
        output: count(usage, "output_tokens"),
        cache_read: cached,
        cache_write: 0,
    }
}

/// Why an answer stopped: an incomplete response says why, and a refusal part is a refusal.
fn stop(answer: &Value, refused: bool) -> Stop {
    if answer.get("status").and_then(Value::as_str) == Some("incomplete") {
        let reason = answer
            .get("incomplete_details")
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str);
        return match reason {
            Some("content_filter") => Stop::Refused,
            _ => Stop::OutputLimit,
        };
    }
    if refused { Stop::Refused } else { Stop::Done }
}
