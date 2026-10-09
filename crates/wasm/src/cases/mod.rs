//! The server's side of the agreement (brief 4.5, Acceptance): each fixture's journey walked
//! through the service natively, as the server runs it, and what it answered recorded as the
//! exact JSON the API sends, beside the call the browser host makes for the same value. The
//! native agreement tests run every call through this crate natively; the browser tests run
//! them in the derive worker's wasm; both compare bytes.
//!
//! A [`Group`] is one domain document and the calls made over it. [`server_groups`] walks the
//! fixtures: every projection, two proposal previews, a local apply against the server's
//! commit, the journey after it, the route files, and the vendor evaluation after an entity
//! merge (so a derive cached across a deployment change is caught). [`limits_group`] adds a
//! document at the limits (containment at the depth limit, `node_count_max` nodes), whose
//! expected values are the native engine's: there the point is the wasm build at the depth
//! limit, and the benchmark. [`budget_group`] adds generated journeys of 500 and 2,000 nodes,
//! whose level and trace the browser test times against the size budgets.

mod limits;
mod walk;

pub use limits::{budget_group, budget_label, limits_document, limits_group};
pub use walk::{NOW, server_groups};

use cairn_schema::{DomainDocument, Patch};
use serde::{Deserialize, Serialize};

use crate::error::{HostError, json};
use crate::{
    ApplyRequest, Derivation, ExportRequest, ImportRequest, PreviewRequest, Projection,
    apply_locally, exported, imported, preview_locally, read_document,
};

/// One call the browser host makes, as the derive worker's protocol names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaseCall {
    /// The whole derive.
    Derive,
    /// One projection.
    Project {
        /// The projection.
        request: Projection,
    },
    /// A proposal previewed against the document's journey.
    Preview {
        /// The proposal and clock.
        request: Box<PreviewRequest>,
    },
    /// A draft patch applied to the document's journey.
    Apply {
        /// The patch and clock.
        request: Box<ApplyRequest>,
    },
    /// A route version or draft exported.
    Export {
        /// The route and version.
        request: Box<ExportRequest>,
    },
    /// A route file imported.
    Import {
        /// The file and base.
        request: Box<ImportRequest>,
    },
    /// A patch's touched set.
    Touched {
        /// The patch.
        patch: Box<Patch>,
    },
}

/// One call and the server's answer to the same request, as the API's JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// What it checks.
    pub name: String,
    /// The browser host's call.
    pub call: CaseCall,
    /// The server's answer, byte for byte.
    pub expected: String,
}

/// One domain document and the calls made over it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// What the document is.
    pub label: String,
    /// The server's domain document, byte for byte; none for calls that read none (route
    /// files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    /// The calls.
    pub cases: Vec<Case>,
}

/// Runs `call` over `document` through this crate, as the derive worker does in wasm.
///
/// # Errors
///
/// The [`HostError`] the call answers; a server-agreeing call answers none.
pub fn run(document: Option<&str>, call: &CaseCall) -> Result<String, HostError> {
    let document = || -> Result<DomainDocument, HostError> {
        read_document(document.ok_or_else(|| HostError::Missing {
            message: "the call reads a document and the group has none".to_owned(),
        })?)
    };
    match call {
        CaseCall::Derive => {
            let derivation = Derivation::of(document()?)?;
            Ok(derivation.derived())
        }
        CaseCall::Project { request } => Derivation::of(document()?)?.projected(request),
        CaseCall::Preview { request } => Ok(json(&preview_locally(&document()?, request)?)),
        CaseCall::Apply { request } => Ok(json(&apply_locally(&document()?, request)?)),
        CaseCall::Export { request } => Ok(json(&exported(request)?)),
        CaseCall::Import { request } => Ok(json(&imported(request)?)),
        CaseCall::Touched { patch } => Ok(json(&patch.touched())),
    }
}
