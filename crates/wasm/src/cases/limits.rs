//! A domain document at the limits (PRACTICES, Explicit limits; No recursion): the date
//! network's cost-test graph (`node_count_max` nodes, containment at the depth limit, both
//! date rules on every node at the edge limit) as a journey. No fixture reaches the limits
//! and no store holds this journey, so its expected values are the native engine's through
//! this crate: what the browser's wasm build must match at the depth limit, on its small
//! stack. It is also the derive benchmark's input (ARCHITECTURE, Date network: reported, not
//! gated).
//!
//! The size budgets' inputs are journeys of 500 and 2,000 generated nodes shaped like a real
//! route ([`budget_group`]): the level with containers collapsed and a relevance class left
//! out, and a trace, which the browser test times in the worker against their budgets.

use std::collections::BTreeSet;

use cairn_engine::testing::generated::{date_limits, route_like};
use cairn_engine::testing::{derive_inputs, journey_id};
use cairn_engine::{Graph, project};
use cairn_schema::{
    Cursor, Deployment, DomainDocument, ExplainedField, Journey, LevelDisplay, LevelQuery,
    ListQuery, NextQuery, NodeKey, NodeKind, Revision, SnapshotScope, from_yaml,
};

use super::{Case, CaseCall, Group, run};
use crate::Projection;
use crate::error::json;

/// A generated graph as a journey's domain document, derived at the engine's fixed test clock.
fn document_of(graph: Graph) -> DomainDocument {
    let header = from_yaml(&format!(
        "id: {}\nname: At the limits\nstatus: active\ncreated_at: \"2026-10-01T00:00:00Z\"\ncreated_on: \"2026-10-01\"\n",
        journey_id()
    ))
    .unwrap_or_else(|error| panic!("a fixed header: {error}"));
    let journey = Journey {
        header,
        revision: Revision::NONE.next(),
        graph: graph.into_document(),
    };
    project::document(&journey, &derive_inputs(Deployment::default()))
}

/// The journey at the limits as a domain document, derived at the engine's fixed test clock.
///
/// # Panics
///
/// Never: the generator builds a valid graph and the header is fixed.
#[must_use]
pub fn limits_document() -> DomainDocument {
    document_of(date_limits())
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
        Projection::level(NodeKind::ALL.into_iter().collect(), None),
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
    calls.extend(requests.into_iter().map(projection_call));
    group_of("at the limits", text, calls)
}

/// One projection as a case's call, named by its request.
fn projection_call(request: Projection) -> (String, CaseCall) {
    (
        format!("project {}", json(&request)),
        CaseCall::Project { request },
    )
}

/// A group over `text`, each call expected as the native build answers it.
fn group_of(label: &str, text: String, calls: Vec<(String, CaseCall)>) -> Group {
    let cases = calls
        .into_iter()
        .map(|(name, call)| {
            let expected = run(Some(&text), &call)
                .unwrap_or_else(|error| panic!("{name} in {label}: {error:?}"));
            Case {
                name,
                call,
                expected,
            }
        })
        .collect();
    Group {
        label: label.to_owned(),
        document: Some(text),
        cases,
    }
}

/// The label of a size-budget group of `nodes` nodes.
#[must_use]
pub fn budget_label(nodes: usize) -> String {
    format!("{nodes} generated nodes")
}

/// A journey of `nodes` generated nodes with the calls its size budget times: the level with
/// ten deep containers collapsed and, in turn, the conditional and the not-relevant class left
/// out (which classifies relevance), and the trace from the deepest node. Each is also an agreement case, so the wasm build matches the native engine
/// at these sizes.
///
/// # Panics
///
/// When a call over the generated journey fails natively.
#[must_use]
pub fn budget_group(nodes: usize) -> Group {
    let document = document_of(route_like(nodes));
    let text = json(&document);
    let graph = Graph::new(document.journey.graph.clone(), &document.inputs.deployment)
        .unwrap_or_else(|violations| panic!("the generated graph is valid: {violations:?}"));
    let tree = graph.tree();
    let depth = |key: &NodeKey| {
        std::iter::successors(tree.parent(key), |parent| tree.parent(parent)).count()
    };
    let deep: Vec<&NodeKey> = graph
        .document()
        .nodes
        .values()
        .filter(|node| node.kind() == NodeKind::Group)
        .map(|node| &node.key)
        .filter(|key| depth(key) >= 2)
        .collect();
    let collapsed: BTreeSet<NodeKey> = deep
        .iter()
        .step_by((deep.len() / 10).max(1))
        .take(10)
        .map(|key| (*key).clone())
        .collect();
    let mut calls: Vec<(String, CaseCall)> = [LevelDisplay::Conditional, LevelDisplay::NotRelevant]
        .into_iter()
        .map(|left_out| {
            projection_call(Projection::Level(LevelQuery {
                collapsed: collapsed.clone(),
                display: LevelDisplay::ALL
                    .into_iter()
                    .filter(|class| *class != left_out)
                    .collect(),
                ..LevelQuery::of_kinds(NodeKind::ALL.into_iter().collect(), None)
            }))
        })
        .collect();
    calls.push(projection_call(Projection::Trace {
        key: deepest(&document),
    }));
    group_of(&budget_label(nodes), text, calls)
}
