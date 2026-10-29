//! The model provider (ARCHITECTURE, Assistant; PRD I5): one OpenAI-compatible
//! chat-completions endpoint, spoken by hand. A deployment configures an endpoint, a model,
//! and a credential; every gateway and vendor Cairn needs speaks this protocol. The
//! credential is configuration, held in memory and sent only in the request's headers, never
//! stored or logged.

use std::fmt;

use serde_json::{Value, json};
use url::Url;

use crate::limits::PROVIDER_RESPONSE_BYTES_MAX;

/// The most tokens one reply may hold: a long proposal fits, and at typical output rates a
/// reply this long still lands inside the provider call limit.
pub const REPLY_TOKEN_COUNT_MAX: u32 = 8_192;

/// A deployment-wide API key (ARCHITECTURE, Assistant): configuration, never stored in the
/// database, and never shown: its `Debug` prints no part of it.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential(String);

impl Credential {
    /// The key as configured.
    #[must_use]
    pub fn new(key: String) -> Self {
        Self(key)
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Credential(..)")
    }
}

/// The assistant's provider configuration: the one knob it has (PRACTICES, No configuration
/// by default names it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderConfig {
    /// The API's base URL, which `chat/completions` is appended to: for example
    /// `https://openrouter.ai/api/v1`, `https://api.openai.com/v1`, or a self-hosted
    /// server's `/v1`.
    pub endpoint: Url,
    /// The model to ask, as the provider names it.
    pub model: String,
    /// The API key; a self-hosted server may need none.
    pub credential: Option<Credential>,
}

/// Why a provider call did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// The call ran past its limit, or the provider could not be reached in time.
    TimedOut,
    /// The provider answered with an error, or with something that is not chat completions.
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

/// One tool call a reply asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    /// The provider's id for the call, which its result names.
    pub id: String,
    /// The tool.
    pub name: String,
    /// Its arguments: a JSON object, or whatever the model wrote, which the tool refuses.
    pub arguments: Value,
}

/// A model's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    /// What it said, if anything.
    pub text: Option<String>,
    /// The tools it called, in order; none ends the turn.
    pub calls: Vec<ToolCall>,
    /// Whether the reply is cut off or declined: none of its calls run.
    pub finished: bool,
    /// The tokens the call used, as the provider counted them.
    pub usage: Usage,
}

/// The tokens one call used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens, cached ones included.
    pub input: u64,
    /// Input tokens read from the provider's prompt cache.
    pub cached: u64,
    /// Output tokens, reasoning included.
    pub output: u64,
}

impl Completion {
    /// The message that goes back to the provider on the next call: the reply as the
    /// protocol's own message, rebuilt rather than echoed, since servers differ in what else
    /// they put on one.
    #[must_use]
    pub fn message(&self) -> Value {
        let mut message = json!({ "role": "assistant", "content": self.text });
        if !self.calls.is_empty() {
            message["tool_calls"] = self
                .calls
                .iter()
                .map(|call| {
                    json!({
                        "id": call.id,
                        "type": "function",
                        "function": { "name": call.name, "arguments": call.arguments.to_string() },
                    })
                })
                .collect();
        }
        message
    }
}

/// The HTTPS client of one configured provider.
pub struct Chat {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl Chat {
    /// The client `config` describes.
    #[must_use]
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Posts `messages` (the system text first) with `tools`, and reads the model's answer.
    /// A rate limit or a server error is not retried: the gateways in front of the models
    /// retry, and a timeout may have billed its tokens. The turn holds the call to its
    /// limits.
    ///
    /// # Errors
    ///
    /// [`ProviderError`] when the call fails, is refused, or answers something else.
    pub async fn complete(
        &self,
        messages: &[Value],
        tools: &[Value],
    ) -> Result<Completion, ProviderError> {
        let key = self.config.credential.as_ref().map(|key| key.0.as_str());
        let body = json!({
            "model": self.config.model,
            "messages": messages,
            "tools": tools,
            "max_tokens": REPLY_TOKEN_COUNT_MAX,
        });
        let url = format!(
            "{}/chat/completions",
            self.config.endpoint.as_str().trim_end_matches('/')
        );
        let mut request = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_string());
        if let Some(key) = key {
            request = request.bearer_auth(key);
        }
        let mut response = request.send().await.map_err(transport)?;
        let status = response.status();
        let bytes = read_bounded(&mut response, PROVIDER_RESPONSE_BYTES_MAX).await?;
        let answer: Result<Value, _> = serde_json::from_slice(&bytes);
        if !status.is_success() {
            let message = match &answer {
                Ok(answer) => error_message(answer),
                Err(_) => String::from_utf8_lossy(&bytes).chars().take(512).collect(),
            };
            return Err(ProviderError::Rejected {
                status: status.as_u16(),
                message: redact(message, key),
            });
        }
        let answer = answer.map_err(|error| failed(&format!("the answer is not JSON: {error}")))?;
        completion(&answer)
    }
}

/// The completion in an answer: the first choice's message.
fn completion(answer: &Value) -> Result<Completion, ProviderError> {
    let choice = answer.pointer("/choices/0");
    let message = choice
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| failed("no choice with a message"))?;
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    let mut calls = Vec::new();
    for call in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let field = |name: &str| {
            call.pointer(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let (Some(id), Some(name), Some(arguments)) = (
            field("/id"),
            field("/function/name"),
            field("/function/arguments"),
        ) else {
            return Err(failed("a tool call without its id, name, or arguments"));
        };
        // The arguments are JSON text; text that is not JSON is kept whole, and the tool
        // refuses it by name.
        let arguments = serde_json::from_str(&arguments).unwrap_or(Value::String(arguments));
        calls.push(ToolCall {
            id,
            name,
            arguments,
        });
    }
    let reason = choice
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str);
    let count = |name: &str| {
        answer
            .pointer(name)
            .and_then(Value::as_u64)
            .unwrap_or_default()
    };
    Ok(Completion {
        text,
        calls,
        finished: !matches!(reason, Some("length" | "content_filter")),
        usage: Usage {
            input: count("/usage/prompt_tokens"),
            cached: count("/usage/prompt_tokens_details/cached_tokens"),
            output: count("/usage/completion_tokens"),
        },
    })
}

fn failed(what: &str) -> ProviderError {
    ProviderError::Failed {
        message: what.to_owned(),
    }
}

/// `message` with the credential, if it echoes one, replaced.
fn redact(message: String, key: Option<&str>) -> String {
    match key {
        Some(key) if !key.is_empty() => message.replace(key, "[redacted]"),
        _ => message,
    }
}

/// The body of `response`, refused once it is past `bytes_max` bytes, read no further.
async fn read_bounded(
    response: &mut reqwest::Response,
    bytes_max: u32,
) -> Result<Vec<u8>, ProviderError> {
    let limit = usize::try_from(bytes_max).unwrap_or(usize::MAX);
    let too_large = || failed(&format!("the answer is over {bytes_max} bytes"));
    let announced = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok());
    if announced.is_some_and(|length| length > limit) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if body.len() + chunk.len() > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A transport failure, which never names the URL or a header.
fn transport(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        return ProviderError::TimedOut;
    }
    failed(&error.without_url().to_string())
}

/// The message of a provider's error body: at `error.message` (or a bare `error` string);
/// otherwise the body, cut short.
fn error_message(answer: &Value) -> String {
    let error = answer.get("error");
    let message = error
        .and_then(|error| error.get("message"))
        .or(error)
        .and_then(Value::as_str);
    match message {
        Some(message) => message.to_owned(),
        None => answer.to_string().chars().take(512).collect(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A credential an error body echoes never reaches the turn.
    #[test]
    fn a_credential_the_provider_echoes_is_redacted() {
        let said = redact(
            "bad key sk-secret-1 for sk-secret-1".to_owned(),
            Some("sk-secret-1"),
        );
        assert!(!said.contains("sk-secret-1"), "{said}");
        assert_eq!(redact("no key".to_owned(), None), "no key");
    }

    /// A reply's text, tool calls (arguments read as JSON), usage and whether it finished.
    #[test]
    fn a_completion_is_read_from_the_first_choice() {
        let answer = |reason: &str| {
            json!({
                "choices": [{ "finish_reason": reason, "message": { "content": "Looking.",
                    "tool_calls": [{ "id": "c1", "type": "function",
                        "function": { "name": "get_snapshot", "arguments": "{\"journey\":\"j_x\"}" } }] } }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 3,
                    "prompt_tokens_details": { "cached_tokens": 7 } },
            })
        };
        let done = completion(&answer("tool_calls")).unwrap();
        assert!(done.finished);
        assert_eq!(done.text.as_deref(), Some("Looking."));
        assert_eq!(done.calls[0].arguments, json!({ "journey": "j_x" }));
        assert_eq!(
            (done.usage.input, done.usage.cached, done.usage.output),
            (10, 7, 3)
        );
        assert!(!completion(&answer("length")).unwrap().finished);
        assert!(!completion(&answer("content_filter")).unwrap().finished);
        assert!(completion(&json!({ "choices": [] })).is_err());
    }
}
