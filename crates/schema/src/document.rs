//! Reading and writing documents (ARCHITECTURE, File format: YAML on disk, JSON on the
//! wire, one schema). Writing is deterministic: field order is the type's, collections are
//! sorted where order carries no meaning, and the same value always writes the same bytes.
//! Reading checks the request-body cap before parsing (any document arrives as one request
//! or file) and reports where in the document a value failed. A graph's own cap applies to
//! the graph a patch produces, which validation checks.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::limits::Limit;

/// A document that failed to parse: where, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// The path to the value that failed, as `nodes[3].due_by`, or empty for the document
    /// itself.
    pub path: String,
    /// What was wrong there.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() || self.path == "." {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "at {}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

fn check_size(text: &str) -> Result<(), ParseError> {
    Limit::RequestBytes
        .check(text.len())
        .map_err(|exceeded| ParseError {
            path: String::new(),
            message: exceeded.to_string(),
        })
}

/// Parses a YAML document.
///
/// # Errors
///
/// When the document is past `request_bytes_max`, is not YAML, or does not fit `T`; the error
/// names the path to the failing value.
pub fn from_yaml<T: DeserializeOwned>(text: &str) -> Result<T, ParseError> {
    check_size(text)?;
    // YAML 1.2 booleans only, so `no` and `yes` are text (a choice id may be `no`), and a
    // repeated key is an error rather than a silent overwrite.
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
    options.with_snippet = false;
    // The parser's default budget caps a document at 250,000 YAML nodes, which documents
    // within Cairn's own limits can pass (a graph of many multi-valued participations). The
    // request cap already bounds the input, and every YAML node or event takes at least a
    // byte, so the budget is set from that cap; alias-expansion limits keep their defaults.
    let request_bytes = usize::try_from(crate::limits::REQUEST_BYTES_MAX).unwrap_or(usize::MAX);
    let mut budget = options.budget.clone().unwrap_or_default();
    budget.max_nodes = request_bytes;
    budget.max_events = request_bytes.saturating_mul(2);
    options.budget = Some(budget);
    // A failure inside the value stops the deserializer early, and the reader then reports
    // the unread rest of the input; the failure inside is the one that matters.
    let mut inner: Option<ParseError> = None;
    let parsed =
        serde_saphyr::with_deserializer_from_str_with_options(text, options, |deserializer| {
            match serde_path_to_error::deserialize::<_, T>(deserializer) {
                Ok(value) => Ok(Some(value)),
                Err(error) => {
                    inner = Some(ParseError {
                        path: error.path().to_string(),
                        message: error.inner().to_string(),
                    });
                    Ok(None)
                }
            }
        });
    if let Some(error) = inner {
        return Err(error);
    }
    match parsed {
        Ok(Some(value)) => Ok(value),
        Ok(None) => unreachable!("a failed parse records its error"),
        Err(error) => Err(ParseError {
            path: String::new(),
            message: error.to_string(),
        }),
    }
}

/// Parses a JSON document.
///
/// # Errors
///
/// When the document is past `request_bytes_max`, is not JSON, or does not fit `T`; the error
/// names the path to the failing value.
pub fn from_json<T: DeserializeOwned>(text: &str) -> Result<T, ParseError> {
    check_size(text)?;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value: T =
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| ParseError {
            path: error.path().to_string(),
            message: error.inner().to_string(),
        })?;
    deserializer.end().map_err(|error| ParseError {
        path: String::new(),
        message: error.to_string(),
    })?;
    Ok(value)
}

/// A document that failed to serialize. The schema's types always serialize, so this is a
/// bug when it happens; it is returned rather than panicking so a host can report it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteError(String);

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "document did not serialize: {}", self.0)
    }
}

impl std::error::Error for WriteError {}

/// Writes a YAML document: block style, two-space indents, a trailing newline.
///
/// # Errors
///
/// When serialization fails, which the schema's types never do.
pub fn to_yaml<T: Serialize>(value: &T) -> Result<String, WriteError> {
    serde_saphyr::to_string(value).map_err(|error| WriteError(error.to_string()))
}

/// Writes a JSON document: compact, no trailing newline (the wire form).
///
/// # Errors
///
/// When serialization fails, which the schema's types never do.
pub fn to_json<T: Serialize>(value: &T) -> Result<String, WriteError> {
    serde_json::to_string(value).map_err(|error| WriteError(error.to_string()))
}

/// Writes a JSON document indented two spaces with a trailing newline, for files.
///
/// # Errors
///
/// When serialization fails, which the schema's types never do.
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String, WriteError> {
    let mut text =
        serde_json::to_string_pretty(value).map_err(|error| WriteError(error.to_string()))?;
    text.push('\n');
    Ok(text)
}
