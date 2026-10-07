//! The wire protocols (ARCHITECTURE, Assistant): each translates an [`Exchange`] into one
//! provider's request body and its answer back into a [`Reply`], and [`HttpProvider`] sends
//! it as one JSON POST. A deployment configures one protocol, endpoint, model, and
//! credential; the credential is configuration, held in memory and sent only in the
//! request's headers, never stored or logged.

pub mod anthropic;
pub mod chat;
pub mod openai;

use std::fmt;

use serde_json::Value;
use url::Url;

use crate::limits::PROVIDER_RESPONSE_BYTES_MAX;
use crate::provider::{Answer, Exchange, Protocol, Provider, ProviderError, Reply};

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
}

impl HttpProvider {
    /// The provider `config` describes.
    #[must_use]
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
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

    /// One call: the request built, posted, and its answer read.
    async fn call(&self, exchange: &Exchange) -> Result<Reply, ProviderError> {
        let model = &self.config.model;
        let key = self.config.credential.as_ref().map(Credential::expose);
        let (body, headers) = match self.config.protocol {
            Protocol::AnthropicMessages => {
                (anthropic::request(model, exchange), anthropic::headers(key))
            }
            Protocol::OpenaiResponses => (openai::request(model, exchange), bearer(key)),
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
        let bytes = read_bounded(response, PROVIDER_RESPONSE_BYTES_MAX).await?;
        let answer: Value =
            serde_json::from_slice(&bytes).map_err(|error| ProviderError::Failed {
                message: format!("status {status}: the answer is not JSON: {error}"),
            })?;
        if !status.is_success() {
            return Err(ProviderError::Failed {
                message: format!("status {status}: {}", error_message(&answer)),
            });
        }
        match self.config.protocol {
            Protocol::AnthropicMessages => anthropic::reply(&answer),
            Protocol::OpenaiResponses => openai::reply(&answer),
            Protocol::ChatCompletions => chat::reply(&answer),
        }
    }
}

impl Provider for HttpProvider {
    fn send<'a>(&'a self, exchange: &'a Exchange) -> Answer<'a> {
        Box::pin(self.call(exchange))
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
