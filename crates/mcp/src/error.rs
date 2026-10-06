//! What a tool answers when it cannot do what it was asked (PRACTICES, Errors, panics, and
//! rejections): a value an agent reads and acts on. A rejected patch carries the engine's
//! rejection unchanged, with every violation by path (A15).

use std::fmt;

use cairn_schema::Rejection;
use cairn_service::{ProposeError, ReadError, ServiceError, WriteError};
use serde::Serialize;

/// Why a tool call did not succeed. Every variant but [`ToolError::UnknownTool`] and
/// [`ToolError::Failed`] is the agent's to fix: it is answered as the tool's result, so the
/// agent sees it and tries again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ToolError {
    /// No tool has this name.
    UnknownTool {
        /// The name asked for.
        name: String,
    },
    /// The arguments do not match the tool's schema.
    Arguments {
        /// Where, as `patch.mutations[0].op`; empty for the arguments as a whole.
        path: String,
        /// What was wrong there.
        message: String,
    },
    /// The write was refused: stale (with the revisions that moved and what their events
    /// touched), a reused patch id, or invalid with every violation (A15, H5).
    Rejected {
        /// The rejection, as the engine and store gave it.
        rejection: Rejection,
    },
    /// What the call names does not exist.
    NotFound {
        /// What is missing.
        message: String,
    },
    /// The request cannot be carried out as asked (a proposal that cannot be drafted, a
    /// patch to a proposal sent as a domain patch).
    Refused {
        /// Why.
        message: String,
    },
    /// The server failed (the store, or a bug): nothing was written, and trying again later
    /// may succeed.
    Failed {
        /// What failed.
        message: String,
    },
}

impl ToolError {
    /// A missing thing, described.
    pub(crate) fn not_found(what: impl fmt::Display) -> Self {
        ToolError::NotFound {
            message: format!("no {what}"),
        }
    }

    /// A request refused, with why.
    pub(crate) fn refused(why: impl fmt::Display) -> Self {
        ToolError::Refused {
            message: why.to_string(),
        }
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::UnknownTool { name } => write!(formatter, "no tool is named {name}"),
            ToolError::Arguments { path, message } if path.is_empty() => {
                write!(formatter, "the arguments: {message}")
            }
            ToolError::Arguments { path, message } => {
                write!(formatter, "the arguments at {path}: {message}")
            }
            ToolError::Rejected { rejection } => rejected(rejection, formatter),
            ToolError::NotFound { message }
            | ToolError::Refused { message }
            | ToolError::Failed { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ToolError {}

/// A rejection in a line: each violation, or the revisions that moved.
fn rejected(rejection: &Rejection, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match rejection {
        Rejection::Invalid { violations } => {
            let violations = violations.as_slice();
            write!(
                formatter,
                "rejected with {} violation(s):",
                violations.len()
            )?;
            for violation in violations {
                write!(formatter, " {violation};")?;
            }
            Ok(())
        }
        Rejection::Stale { conflicts, .. } => {
            formatter.write_str("stale: read again and redo the write against")?;
            for conflict in conflicts {
                let current = conflict.current;
                write!(formatter, " {:?} at revision {current};", conflict.of)?;
            }
            Ok(())
        }
        Rejection::PatchIdReused { patch_id } => write!(
            formatter,
            "patch id {patch_id} was used before for another write: choose a new one"
        ),
    }
}

impl From<ServiceError> for ToolError {
    fn from(error: ServiceError) -> Self {
        ToolError::Failed {
            message: error.to_string(),
        }
    }
}

impl From<WriteError> for ToolError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Rejected(rejection) => ToolError::Rejected { rejection },
            WriteError::Failed(error) => error.into(),
        }
    }
}

impl From<ReadError> for ToolError {
    fn from(error: ReadError) -> Self {
        match error {
            ReadError::JourneyMissing(id) => ToolError::not_found(format_args!("journey {id}")),
            ReadError::ProposalMissing(id) => ToolError::not_found(format_args!("proposal {id}")),
            ReadError::Projection(error) => ToolError::NotFound {
                message: error.to_string(),
            },
            ReadError::Failed(error) => error.into(),
        }
    }
}

impl From<ProposeError> for ToolError {
    fn from(error: ProposeError) -> Self {
        match error {
            ProposeError::Draft(error) => ToolError::refused(error),
            ProposeError::ProposalMissing(id) => {
                ToolError::not_found(format_args!("proposal {id}"))
            }
            ProposeError::Write(error) => error.into(),
        }
    }
}
