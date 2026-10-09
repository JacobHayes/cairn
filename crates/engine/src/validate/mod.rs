//! Structural validation (PRD Invariants; A15): the checks every accepted graph passes, each
//! stage in its own module, each collecting every violation it finds rather than stopping at
//! the first. [`GRAPH_STAGES`] check one graph document on its own; the apply pipeline runs
//! them on every graph a patch writes and adds the stages that need the rest of the domain.

mod cycles;
mod insertions;
mod references;
pub(crate) mod state;
mod structure;

use std::collections::BTreeMap;

use cairn_schema::{Deployment, Location, NodeKey, Subject, Violation, ViolationCode};

use crate::graph::{Document, Graph, Tree};

/// One graph under validation.
pub(crate) struct GraphCheck<'a> {
    /// The document.
    pub document: &'a Document,
    /// Its containment tree.
    pub tree: &'a Tree,
    /// A journey's graph, with state; otherwise a route version or draft, with none.
    pub journey: bool,
    /// The snoozes the patch being applied sets, which are held to the stricter container
    /// rule (B6); none when a stored graph is checked on its own.
    pub snoozes_made: Option<&'a BTreeMap<NodeKey, u32>>,
}

/// A validation stage over one graph.
pub(crate) type GraphStage = fn(&GraphCheck<'_>, &mut Vec<Violation>);

/// The graph stages, in order: the tree, ids, keys, and limits; references; dependency
/// cycles among the gate edges of the full effective dependency graph; journey state;
/// insertions.
pub(crate) const GRAPH_STAGES: [GraphStage; 5] = [
    structure::check,
    references::check,
    cycles::check,
    state::check,
    insertions::check,
];

/// Every violation of the graph stages in a document on its own. A document with state is a
/// journey's graph.
#[must_use]
pub(crate) fn graph(document: &Document, tree: &Tree) -> Vec<Violation> {
    let check = GraphCheck {
        document,
        tree,
        journey: !document.state.is_empty(),
        snoozes_made: None,
    };
    let mut violations = Vec::new();
    for stage in GRAPH_STAGES {
        stage(&check, &mut violations);
    }
    violations
}

/// The graph stages on one graph, then, when they found nothing, the plan check (F5;
/// ARCHITECTURE, Write path: structure, references, cycles, plan check): it derives
/// relevance, which needs a graph that holds every other invariant.
pub(crate) fn with_plan(check: &GraphCheck<'_>, deployment: &Deployment, out: &mut Vec<Violation>) {
    let found_before = out.len();
    for stage in GRAPH_STAGES {
        stage(check, out);
    }
    if out.len() > found_before {
        return;
    }
    let graph = Graph::trusted(check.document.clone(), check.tree.clone());
    out.extend(crate::derive::dates::plan_violation(&graph, deployment));
}

/// A violation with no location yet.
#[must_use]
pub(crate) fn violation(code: ViolationCode, message: impl Into<String>) -> Violation {
    Violation {
        code,
        at: Location::default(),
        message: message.into(),
        related: Vec::new(),
        limit: None,
        bypassable: None,
        failures: std::collections::BTreeSet::new(),
        chains: None,
        caused_by: std::collections::BTreeSet::new(),
    }
}

/// A violation about a node, located by its path in the tree when a root reaches it.
#[must_use]
pub(crate) fn at_node(
    tree: &Tree,
    key: &NodeKey,
    code: ViolationCode,
    message: impl Into<String>,
) -> Violation {
    let mut found = violation(code, message);
    found.at.subject = Some(Subject::Node(key.clone()));
    found.at.path = tree.path(key).cloned();
    found
}

/// How a node reference that does not resolve is reported: a key the graph retired is a
/// dangling reference left by a removal (A18), any other is unresolved.
#[must_use]
pub(crate) fn missing_node_code(document: &Document, key: &NodeKey) -> ViolationCode {
    if document.retired_keys.nodes.contains(key) {
        ViolationCode::DanglingReference
    } else {
        ViolationCode::UnresolvedReference
    }
}

/// How a role reference that does not resolve is reported: a role removed while still
/// referenced (A18), or one that never existed.
#[must_use]
pub(crate) fn missing_role_code(document: &Document, key: &cairn_schema::RoleKey) -> ViolationCode {
    if document.retired_keys.roles.contains(key) {
        ViolationCode::StillReferenced
    } else {
        ViolationCode::UnresolvedReference
    }
}

/// How a participation kind reference that does not resolve is reported.
#[must_use]
pub(crate) fn missing_kind_code(document: &Document, key: &cairn_schema::KindKey) -> ViolationCode {
    if document.retired_keys.kinds.contains(key) {
        ViolationCode::StillReferenced
    } else {
        ViolationCode::UndeclaredKind
    }
}
