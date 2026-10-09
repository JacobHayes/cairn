//! Notices (A20): advisory findings about a route graph. A notice is recomputed from the
//! graph each time it is asked for, never stored, and never rejects anything (A15).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{NodeKey, Path};

/// What a notice is about. One exists today; each later advisory finding adds a code.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum NoticeCode {
    /// A node with no chain to or from the graph's `final` milestone, so neither its priority
    /// nor its dates feel that milestone.
    Unanchored,
}

/// One advisory finding about a node of a route graph (A20).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    /// What it is about.
    pub code: NoticeCode,
    /// The node it names.
    pub node: NodeKey,
    /// The node's path, to open it by.
    pub path: Path,
    /// A sentence for people, generated for display.
    pub message: String,
}
