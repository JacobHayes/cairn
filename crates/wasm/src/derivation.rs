//! A domain document derived in the browser (ARCHITECTURE, Web UI: data flow): the engine
//! version checked first (version skew), the journey validated and derived once with the
//! document's own inputs, and every projection run on that one derivation, as the server's
//! service runs them on its memoized one. Inputs and outputs are the schema's JSON, so a value
//! here is the server's value byte for byte.
//!
//! Cost: reading the document is linear in its size; the derive is the engine's (ARCHITECTURE,
//! Read path); each projection costs what its engine module states.

use std::collections::BTreeSet;

use cairn_engine::{
    Derived, DerivedJourney, DraftContext, Graph, ProjectionError, derive, engine_version,
};
use cairn_schema::{
    AttachmentKey, Cursor, DomainDocument, EngineVersion, ExplainedField, KindKey, ListQuery,
    NextQuery, NodeKey, NodeKind, RenderedDraft, ResourceContent, SnapshotScope, Url,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{HostError, json, read};

/// The engine version a document carries, read before anything else in it, so a document
/// from a newer engine with fields this one does not know is refused as skew, not as
/// unreadable.
#[derive(Deserialize)]
struct Versioned {
    engine_version: EngineVersion,
}

/// ARCHITECTURE, Web UI: version skew. Reads a domain document, refusing one built by another
/// engine version before reading the rest of it.
///
/// # Errors
///
/// [`HostError::VersionSkew`] for another engine's document; [`HostError::Unreadable`] when it
/// is not a domain document.
pub fn read_document(text: &str) -> Result<DomainDocument, HostError> {
    let Versioned {
        engine_version: version,
    } = read("document", text)?;
    let engine = engine_version();
    if version != engine {
        return Err(HostError::VersionSkew {
            document: version,
            engine,
        });
    }
    read("document", text)
}

/// The engine version this module was built from: what a host compares a document's with
/// before deriving, previewing, or writing with it.
#[wasm_bindgen(js_name = engineVersion)]
#[must_use]
pub fn engine_version_text() -> String {
    engine_version().as_str().to_owned()
}

/// One projection of a derived journey (ARCHITECTURE, Engine > Projections), as the server's
/// projection endpoints take their queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "projection", rename_all = "snake_case", deny_unknown_fields)]
pub enum Projection {
    /// C2: one canvas level, the kinds shown, at the top or drilled into `container`.
    Level {
        /// The kinds shown.
        shown: BTreeSet<NodeKind>,
        /// The container drilled into; the top level when none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        container: Option<NodeKey>,
    },
    /// C7: upstream and downstream of a node.
    Trace {
        /// The node.
        key: NodeKey,
    },
    /// C12: the decision view.
    DecisionView,
    /// C13: the timeline.
    Timeline,
    /// C18: the status summary.
    StatusSummary,
    /// C10: the acting frontier, ranked and filtered.
    Next {
        /// The filters, the sort, and whose ranking.
        #[serde(default)]
        query: NextQuery,
    },
    /// C9: the nodes a query matches, paged.
    List {
        /// The query.
        #[serde(default)]
        query: ListQuery,
    },
    /// E4: the nodes the viewer participates in, by kind (every kind when empty).
    Mine {
        /// The participation kinds.
        #[serde(default)]
        kinds: BTreeSet<KindKey>,
    },
    /// I3: the bounded agent snapshot.
    Snapshot {
        /// The subtree, depth, and page.
        #[serde(default)]
        scope: SnapshotScope,
    },
    /// C8: one page of a node's explanation list.
    Explanations {
        /// The node.
        key: NodeKey,
        /// Which value's list.
        field: ExplainedField,
        /// Where the page starts.
        #[serde(default)]
        cursor: Cursor,
    },
}

/// A10, G3: a node's message draft to render with the journey's context. The server has no
/// such read: a draft is rendered where the document is derived, which for the UI is the
/// browser (ARCHITECTURE, Web UI: data flow).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftRequest {
    /// The node the resource is on.
    pub key: NodeKey,
    /// The resource, a message draft.
    pub resource: AttachmentKey,
    /// The link to the journey, which only the page knows (`{{journey.url}}`); none renders
    /// a marker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<Url>,
}

/// A domain document with its journey validated and derived (D3): what every projection reads.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Derivation {
    document: DomainDocument,
    graph: Graph,
    derived: Derived,
}

impl Derivation {
    /// D3: derives `document`'s journey with its own inputs.
    ///
    /// # Errors
    ///
    /// [`HostError::Invalid`] when the journey breaks an invariant, which a server-built
    /// document never does.
    pub fn of(document: DomainDocument) -> Result<Self, HostError> {
        let graph = Graph::new(document.journey.graph.clone(), &document.inputs.deployment)
            .map_err(|violations| HostError::Invalid { violations })?;
        let derived = derive(
            &graph,
            Some(document.journey.header.created_on),
            &document.inputs,
        );
        Ok(Self {
            document,
            graph,
            derived,
        })
    }

    /// The document derived.
    #[must_use]
    pub fn document(&self) -> &DomainDocument {
        &self.document
    }

    /// The projection `request`, as the server's service answers it.
    ///
    /// # Errors
    ///
    /// [`HostError::Missing`] when the request names a node the journey does not hold.
    pub fn projected(&self, request: &Projection) -> Result<String, HostError> {
        let journey = DerivedJourney::new(&self.graph, &self.derived);
        let viewer = &self.document.inputs.viewer;
        let missing = |error: ProjectionError| HostError::Missing {
            message: error.to_string(),
        };
        Ok(match request {
            Projection::Level { shown, container } => {
                json(&journey.level(shown, container.as_ref()).map_err(missing)?)
            }
            Projection::Trace { key } => json(&journey.trace(key).map_err(missing)?),
            Projection::DecisionView => json(&journey.decision_view()),
            Projection::Timeline => json(&journey.timeline()),
            Projection::StatusSummary => json(&journey.status_summary()),
            Projection::Next { query } => json(&journey.next(query, viewer).map_err(missing)?),
            Projection::List { query } => json(&journey.list(query, viewer).map_err(missing)?),
            Projection::Mine { kinds } => json(&journey.mine(viewer, kinds)),
            Projection::Snapshot { scope } => json(&journey.snapshot(scope).map_err(missing)?),
            Projection::Explanations { key, field, cursor } => json(
                &journey
                    .explanations(key, *field, *cursor)
                    .map_err(missing)?,
            ),
        })
    }

    /// A10, G3: the message draft `request` names, rendered with the journey's context:
    /// its header, the page's link to it, and the deployment's entities.
    ///
    /// # Errors
    ///
    /// [`HostError::Missing`] when the node, or a message draft by that key on it, is not in
    /// the journey.
    pub fn rendered(&self, request: &DraftRequest) -> Result<RenderedDraft, HostError> {
        let missing = |message: String| HostError::Missing { message };
        let node = self
            .graph
            .node(&request.key)
            .ok_or_else(|| missing(format!("no node {}", request.key)))?;
        let template = node
            .resources
            .iter()
            .find(|resource| resource.key == request.resource)
            .and_then(|resource| match &resource.content {
                ResourceContent::MessageDraft(template) => Some(template),
                _ => None,
            })
            .ok_or_else(|| {
                missing(format!(
                    "no message draft {} on {}",
                    request.resource, request.key
                ))
            })?;
        let context = DraftContext {
            header: &self.document.journey.header,
            url: request.url.as_ref(),
            deployment: &self.document.inputs.deployment,
        };
        Ok(DerivedJourney::new(&self.graph, &self.derived).render_draft(template, &context))
    }
}

#[wasm_bindgen]
impl Derivation {
    /// Reads `document` (refusing another engine version's) and derives it.
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: version skew, an unreadable document, or an invalid
    /// journey.
    #[wasm_bindgen(constructor)]
    pub fn new(document: &str) -> Result<Derivation, String> {
        Ok(Self::of(read_document(document)?)?)
    }

    /// D3: every derived value in the schema's shape, as the server's derive answers it.
    #[must_use]
    pub fn derived(&self) -> String {
        json(&self.derived.to_schema(&self.graph))
    }

    /// One projection: `request` is the JSON of a [`Projection`].
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: an unreadable request, or a node the journey lacks.
    pub fn project(&self, request: &str) -> Result<String, String> {
        Ok(self.projected(&read("projection", request)?)?)
    }

    /// A10, G3: a message draft rendered: `request` is the JSON of a [`DraftRequest`]; the
    /// answer is the JSON of a [`RenderedDraft`].
    ///
    /// # Errors
    ///
    /// The JSON of a [`HostError`]: an unreadable request, or a node or draft the journey
    /// lacks.
    #[wasm_bindgen(js_name = renderDraft)]
    pub fn render_draft(&self, request: &str) -> Result<String, String> {
        Ok(json(&self.rendered(&read("draft request", request)?)?))
    }

    /// What it was derived from, as a query cache keys it (ARCHITECTURE, Web UI): the
    /// journey, its revision, the deployment revision, and today, as JSON.
    #[must_use]
    pub fn key(&self) -> String {
        let journey = &self.document.journey;
        json(&serde_json::json!({
            "journey": journey.header.id,
            "revision": journey.revision,
            "deployment_revision": self.document.inputs.deployment.revision,
            "today": self.document.inputs.today,
        }))
    }
}
