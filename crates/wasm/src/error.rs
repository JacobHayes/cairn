//! What a browser-host call answers instead of its value. Every export takes and returns JSON
//! text; a failure is thrown to JavaScript as the JSON of a [`HostError`], which the
//! hand-written loader in `web/wasm` parses back into a typed error.

use cairn_schema::{EngineVersion, Rejection, Violations};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Why a call answered no value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostError {
    /// ARCHITECTURE, Web UI: version skew. The document was built by another engine version,
    /// so this engine does not derive, preview, or write with it; the page reloads instead.
    VersionSkew {
        /// The document's engine version.
        document: EngineVersion,
        /// This engine's.
        engine: EngineVersion,
    },
    /// An input is not the JSON the call reads.
    Unreadable {
        /// Which input.
        input: String,
        /// Where and why it failed.
        message: String,
    },
    /// A graph breaks an invariant (A15): a document's journey, or an imported route file.
    Invalid {
        /// Every violation.
        violations: Violations,
    },
    /// A write or a local apply was rejected: stale (H5), invalid, or a reused patch id.
    Rejected {
        /// The rejection, as the API answers it.
        rejection: Rejection,
    },
    /// A call names something its document or the root does not hold.
    Missing {
        /// What is missing.
        message: String,
    },
    /// The in-browser root's service or store failed.
    Failed {
        /// What failed.
        message: String,
    },
}

impl HostError {
    /// An input that did not read as `input`.
    pub(crate) fn unreadable(input: &str, message: impl std::fmt::Display) -> Self {
        HostError::Unreadable {
            input: input.to_owned(),
            message: message.to_string(),
        }
    }
}

impl From<HostError> for String {
    /// The error's JSON, thrown to JavaScript.
    fn from(error: HostError) -> Self {
        json(&error)
    }
}

/// Reads `text` as the JSON of `T`, naming `input` and the failing path when it is not.
/// No size check: a domain document at the limits is a response, larger than any request.
pub(crate) fn read<T: DeserializeOwned>(input: &str, text: &str) -> Result<T, HostError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value: T = serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        HostError::unreadable(
            input,
            format_args!("at {}: {}", error.path(), error.inner()),
        )
    })?;
    deserializer
        .end()
        .map_err(|error| HostError::unreadable(input, error))?;
    Ok(value)
}

/// The compact JSON of `value`: the wire form the API answers, byte for byte.
///
/// # Panics
///
/// When serialization fails, which the schema's types never do.
pub(crate) fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|error| panic!("a schema value serializes: {error}"))
}
