//! A domain document at the limits (PRACTICES, Explicit limits; No recursion): the date
//! network's cost-test graph (`node_count_max` nodes, containment at the depth limit, both
//! date rules on every node at the edge limit) as a journey. No fixture reaches the limits
//! and no store holds this journey, so its expected values are the native engine's through
//! this crate: what the browser's wasm build must match at the depth limit, on its small
//! stack. It is also the derive benchmark's input (ARCHITECTURE, Date network: reported, not
//! gated).

use std::collections::BTreeSet;

use cairn_engine::testing::generated::date_limits;
use cairn_engine::testing::{derive_inputs, journey_id};
use cairn_engine::{Graph, project};
use cairn_schema::{
    Cursor, Deployment, DomainDocument, ExplainedField, Journey, ListQuery, NextQuery, NodeKey,
    NodeKind, Revision, SnapshotScope, from_yaml,
};

use super::{Case, CaseCall, Group, run};
use crate::Projection;
use crate::error::json;

/// The journey at the limits as a domain document, derived at the engine's fixed test clock.
///
/// # Panics
///
/// Never: the generator builds a valid graph and the header is fixed.
#[must_use]
pub fn limits_document() -> DomainDocument {
    let header = from_yaml(&format!(
        "id: {}\nname: At the limits\nstatus: active\ncreated_at: \"2026-10-01T00:00:00Z\"\ncreated_on: \"2026-10-01\"\n",
        journey_id()
    ))
    .unwrap_or_else(|error| panic!("a fixed header: {error}"));
    let journey = Journey {
        header,
        revision: Revision::NONE.next(),
        graph: date_limits().into_document(),
    };
    project::document(&journey, &derive_inputs(Deployment::default()))
}

/// The node deepest in the containment tree: a trace from it walks the tree's full depth.
fn deepest(document: &DomainDocument) -> NodeKey {
    let graph = Graph::new(document.journey.graph.clone(), &document.inputs.deployment)
        .unwrap_or_else(|violations| panic!("the limits graph is valid: {violations:?}"));
    let tree = graph.tree();
    let depth = |key: &NodeKey| {
        let mut depth = 0_u32;
        let mut current = tree.parent(key);
        while let Some(parent) = current {
            depth += 1;
            current = tree.parent(parent);
        }
        depth
    };
    let keys = document.journey.graph.nodes.as_map().keys();
    keys.max_by_key(|key| depth(key))
        .cloned()
        .unwrap_or_else(|| panic!("the limits graph has nodes"))
}

/// The document at the limits with its derive and every projection, each expected as the
/// native build of this crate answers it.
///
/// # Panics
///
/// When a call over the limits document fails natively.
#[must_use]
pub fn limits_group() -> Group {
    let document = limits_document();
    let text = json(&document);
    let deep = deepest(&document);
    let mut calls = vec![("derive".to_owned(), CaseCall::Derive)];
    let requests = [
        Projection::Level {
            shown: NodeKind::ALL.into_iter().collect(),
            container: None,
        },
        Projection::Trace { key: deep.clone() },
        Projection::Explanations {
            key: deep,
            field: ExplainedField::Gravity,
            cursor: Cursor::START,
        },
        Projection::DecisionView,
        Projection::Timeline,
        Projection::StatusSummary,
        Projection::Next {
            query: NextQuery::default(),
        },
        Projection::List {
            query: ListQuery::default(),
        },
        Projection::Mine {
            kinds: BTreeSet::new(),
        },
        Projection::Snapshot {
            scope: SnapshotScope::default(),
        },
    ];
    calls.extend(requests.into_iter().map(|request| {
        (
            format!("project {}", json(&request)),
            CaseCall::Project { request },
        )
    }));
    let cases = calls
        .into_iter()
        .map(|(name, call)| {
            let expected = run(Some(&text), &call)
                .unwrap_or_else(|error| panic!("{name} at the limits: {error:?}"));
            Case {
                name,
                call,
                expected,
            }
        })
        .collect();
    Group {
        label: "at the limits".to_owned(),
        document: Some(text),
        cases,
    }
}
