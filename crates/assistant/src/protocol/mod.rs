//! The wire protocols (ARCHITECTURE, Assistant): each translates an [`Exchange`] into one
//! provider's request body and its answer back into a [`Reply`], and [`HttpProvider`] sends
//! it as one JSON POST. A deployment configures one protocol, endpoint, model, and
//! credential; the credential is configuration, held in memory and sent only in the
//! request's headers, never stored or logged.

pub mod anthropic;
pub mod chat;
pub mod openai;

use std::fmt;
use std::hash::{BuildHasher, RandomState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use url::Url;

use crate::limits::{
    PROVIDER_RESPONSE_BYTES_MAX, PROVIDER_RETRY_COUNT_MAX, PROVIDER_RETRY_WAIT_MAX,
};
use crate::provider::{Exchange, Protocol, Provider, ProviderError, Reply};

/// The most tokens one reply may hold, which the Anthropic protocol requires a request to name
/// and the others are given too: a long proposal fits, and at typical output rates a reply this
/// long still lands inside the provider call limit.
pub const REPLY_TOKEN_COUNT_MAX: u32 = 8_192;

/// The wait before the first retry, doubled for each after it, then jittered.
const RETRY_BACKOFF_BASE: Duration = Duration::from_secs(1);

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

    /// The key, for a request header.
    fn expose(&self) -> &str {
        &self.0
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
    /// The wire protocol.
    pub protocol: Protocol,
    /// The API's base URL, which the protocol's path is appended to: for example
    /// `https://api.anthropic.com/v1`, `https://api.openai.com/v1`, or an OpenAI-compatible
    /// server's `/v1`.
    pub endpoint: Url,
    /// The model to ask, as the provider names it.
    pub model: String,
    /// The API key; an OpenAI-compatible server may need none.
    pub credential: Option<Credential>,
}

/// A provider over HTTPS speaking one of the [`Protocol`]s.
pub struct HttpProvider {
    config: ProviderConfig,
    client: reqwest::Client,
    /// Whether the Responses requests ask for encrypted reasoning; cleared once the model
    /// refuses being asked, as a model that does not reason does.
    encrypted_reasoning: AtomicBool,
}

impl HttpProvider {
    /// The provider `config` describes.
    #[must_use]
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
            encrypted_reasoning: AtomicBool::new(true),
        }
    }

    /// The URL a call is posted to.
    fn url(&self) -> String {
        let path = match self.config.protocol {
            Protocol::AnthropicMessages => anthropic::PATH,
            Protocol::OpenaiResponses => openai::PATH,
            Protocol::ChatCompletions => chat::PATH,
        };
        format!(
            "{}/{path}",
            self.config.endpoint.as_str().trim_end_matches('/')
        )
    }

    /// One call: the request built, posted, and its answer read. A refused call answers the
    /// provider's `Retry-After` beside the error.
    async fn call(&self, exchange: &Exchange) -> Result<Reply, Refused> {
        let model = &self.config.model;
        let key = self.config.credential.as_ref().map(Credential::expose);
        let (body, headers) = match self.config.protocol {
            Protocol::AnthropicMessages => {
                (anthropic::request(model, exchange), anthropic::headers(key))
            }
            Protocol::OpenaiResponses => {
                let encrypted = self.encrypted_reasoning.load(Ordering::Relaxed);
                (openai::request(model, exchange, encrypted), bearer(key))
            }
            Protocol::ChatCompletions => (chat::request(model, exchange), bearer(key)),
        };
        let mut request = self
            .client
            .post(self.url())
            .header("content-type", "application/json")
            .body(body.to_string());
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send().await.map_err(transport)?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|wait| wait.to_str().ok())
            .and_then(|wait| retry_after(wait, jiff::Timestamp::now()));
        let bytes = read_bounded(response, PROVIDER_RESPONSE_BYTES_MAX).await?;
        let answer: Result<Value, _> = serde_json::from_slice(&bytes);
        if !status.is_success() {
            let message = match &answer {
                Ok(answer) => error_message(answer),
                Err(_) => String::from_utf8_lossy(&bytes).chars().take(512).collect(),
            };
            return Err(Refused {
                error: ProviderError::Rejected {
                    status: status.as_u16(),
                    message: redact(message, key),
                },
                retry_after,
            });
        }
        let answer = answer.map_err(|error| ProviderError::Failed {
            message: format!("status {status}: the answer is not JSON: {error}"),
        })?;
        Ok(match self.config.protocol {
            Protocol::AnthropicMessages => anthropic::reply(&answer),
            Protocol::OpenaiResponses => openai::reply(&answer),
            Protocol::ChatCompletions => chat::reply(&answer),
        }?)
    }
}

/// A call that failed, and how long the provider asked to wait before another.
struct Refused {
    error: ProviderError,
    retry_after: Option<Duration>,
}

impl From<ProviderError> for Refused {
    fn from(error: ProviderError) -> Self {
        Self {
            error,
            retry_after: None,
        }
    }
}

#[async_trait]
impl Provider for HttpProvider {
    /// Retries a rate limit or a server error up to [`PROVIDER_RETRY_COUNT_MAX`] times; the
    /// turn holds the whole call, waits included, to its limits. A timeout is never retried:
    /// the provider may have billed the tokens.
    async fn send(&self, exchange: &Exchange) -> Result<Reply, ProviderError> {
        let mut retries = 0;
        loop {
            let refused = match self.call(exchange).await {
                Ok(reply) => return Ok(reply),
                Err(refused) => refused,
            };
            let ProviderError::Rejected { status, message } = &refused.error else {
                return Err(refused.error);
            };
            // A model that does not reason refuses being asked for its reasoning: ask no more,
            // and send this call again at once.
            if self.config.protocol == Protocol::OpenaiResponses
                && *status == 400
                && message.contains("ncrypted content")
                && self.encrypted_reasoning.swap(false, Ordering::Relaxed)
            {
                tracing::info!(%message, "assistant model does not reason; asking no more for encrypted reasoning");
                continue;
            }
            let jitter = jitter();
            let Some(wait) = retry_wait(*status, retries, refused.retry_after, jitter) else {
                return Err(refused.error);
            };
            tracing::warn!(
                status,
                message,
                retries,
                ?wait,
                "assistant provider call retried"
            );
            tokio::time::sleep(wait).await;
            retries += 1;
        }
    }
}

/// The wait a `Retry-After` header asks for at `now`: a number of seconds, or an HTTP date
/// (zero when it is past); `None` when it is neither.
#[must_use]
pub fn retry_after(value: &str, now: jiff::Timestamp) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = jiff::fmt::rfc2822::DateTimeParser::new()
        .parse_timestamp(value)
        .ok()?;
    let wait = at.duration_since(now);
    Some(Duration::try_from(wait).unwrap_or_default())
}

/// How long to wait before retrying a call answered with `status`, after `retries` retries
/// already, when the provider asked for `retry_after`; `None` when it is not worth retrying:
/// not a rate limit or a server error (529 is one), out of retries, or asked to wait longer
/// than a turn should. The wait doubles each retry, scaled by `jitter` (0 to 1000 thousandths)
/// down to half, and is never shorter than what the provider asked.
fn retry_wait(
    status: u16,
    retries: u32,
    retry_after: Option<Duration>,
    jitter: u32,
) -> Option<Duration> {
    let retryable = status == 429 || (500..=599).contains(&status);
    if !retryable || retries >= PROVIDER_RETRY_COUNT_MAX {
        return None;
    }
    if retry_after.is_some_and(|wait| wait > PROVIDER_RETRY_WAIT_MAX) {
        return None;
    }
    let backoff = RETRY_BACKOFF_BASE * 2_u32.pow(retries);
    let backoff = backoff * (500 + jitter.min(1000) / 2) / 1000;
    Some(backoff.max(retry_after.unwrap_or_default()))
}

/// A number from 0 up to 1000, from the standard library's randomly seeded hasher.
fn jitter() -> u32 {
    u32::try_from(RandomState::new().hash_one(0_u8) % 1001).unwrap_or(0)
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
    mut response: reqwest::Response,
    bytes_max: u32,
) -> Result<Vec<u8>, ProviderError> {
    let limit = usize::try_from(bytes_max).unwrap_or(usize::MAX);
    let too_large = || ProviderError::Failed {
        message: format!(
            "the answer is over {bytes_max} bytes (assistant_provider_response_bytes)"
        ),
    };
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

/// The bearer authorization header, when there is a key.
fn bearer(key: Option<&str>) -> Vec<(&'static str, String)> {
    key.map(|key| ("authorization", format!("Bearer {key}")))
        .into_iter()
        .collect()
}

/// A transport failure, which names the URL but never a header.
fn transport(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        return ProviderError::TimedOut;
    }
    ProviderError::Failed {
        message: error.without_url().to_string(),
    }
}

/// The message of a provider's error body: every protocol here puts one at `error.message`
/// (or a bare `error` string); otherwise the body, cut short.
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

/// A protocol's answer that is not what the protocol says it sends.
pub(crate) fn malformed(what: &str) -> ProviderError {
    ProviderError::Failed {
        message: format!("the answer is not the protocol's: {what}"),
    }
}

/// A call's arguments, which these protocols send as JSON text: parsed, or kept as the
/// string when the model wrote something else, so the tool refuses it by name.
pub(crate) fn arguments_from_text(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
}

/// A call's arguments as the JSON text these protocols send.
pub(crate) fn arguments_as_text(arguments: &Value) -> String {
    arguments.to_string()
}
