//! `stale` (D4; ARCHITECTURE, Read path: derive, pass 7): a terminal node whose completing
//! guards would now fail, checked against the transition it used: `done` was completed
//! (`deps_done`, `has_artifact`, `broken_down`), `decided` answered and `reached` reached
//! (`deps_done`), and a `skipped` node is never stale, since its skip required nothing. The
//! failures a guard bypass recorded are accepted (D4: a bypass records the specific failures
//! present), so only a later, distinct failure makes the node stale. Visible, never blocking.
//! Only nodes in scope are judged: a node that is not relevant is out of the journey.
//!
//! Which nodes are stale is decided per node here; their reasons are listed on demand
//! ([`reasons`]), since an open dependency inherited from an ancestor would otherwise be
//! listed once per descendant (up to nodes x depth x 64 entries).
//!
//! Cost at `node_count_max`: the artifact links are indexed once, O(annotations log nodes); a
//! terminal node whose `deps_done` holds is then decided in O(log nodes); any other walks its entry chains once
//! ([`super::dependencies::Dependencies::of`], at most about 1,300 edges), so at most
//! 2,000 x 1,300 steps when every node is a stale terminal node.

use std::collections::BTreeSet;

use cairn_schema::{
    DependencyVia, GuardFailure, KeyRefs, Node, NodeKey, Payload, Relevance, State,
};

use super::blocking::Blocking;
use super::dependencies::{Dependencies, EdgeClass, EdgeSet};
use super::relevance::Relevances;
use super::stored_state;
use crate::graph::{Document, Graph, Tree};

/// The stale nodes, in key order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Staleness {
    nodes: BTreeSet<NodeKey>,
}

impl Staleness {
    /// D4: the stale nodes of a derived journey.
    #[must_use]
    pub(crate) fn pass(
        graph: &Graph,
        relevance: &Relevances,
        dependencies: &Dependencies,
        blocking: &Blocking,
    ) -> Self {
        let artifacts = artifact_nodes(graph.document());
        let nodes: BTreeSet<NodeKey> = graph
            .document()
            .nodes
            .values()
            .filter(|node| {
                let found = reasons(
                    graph,
                    relevance,
                    dependencies,
                    blocking,
                    &artifacts,
                    &node.key,
                );
                !found.is_empty()
            })
            .map(|node| node.key.clone())
            .collect();
        assert!(nodes.iter().all(|key| relevance.in_scope(key)));
        Self { nodes }
    }

    /// The node is stale.
    #[must_use]
    pub(crate) fn contains(&self, key: &NodeKey) -> bool {
        self.nodes.contains(key)
    }
}

/// D4: why the node is stale, empty when it is not: the failures of the guards its
/// transition required, less those its bypass recorded. `artifacts` are the nodes with an
/// artifact link ([`artifact_nodes`]).
#[must_use]
pub(crate) fn reasons(
    graph: &Graph,
    relevance: &Relevances,
    dependencies: &Dependencies,
    blocking: &Blocking,
    artifacts: &BTreeSet<&NodeKey>,
    key: &NodeKey,
) -> BTreeSet<GuardFailure> {
    let document = graph.document();
    let Some(node) = document.nodes.get(key) else {
        return BTreeSet::new();
    };
    let completed = match stored_state(document, node) {
        State::Done => true,
        State::Decided | State::Reached => false,
        State::Skipped
        | State::Todo
        | State::Active
        | State::Open
        | State::Pending
        | State::Derived => return BTreeSet::new(),
    };
    if !relevance.in_scope(key) {
        return BTreeSet::new();
    }
    let mut failures = if completed {
        static_failures(document, graph.tree(), node, artifacts)
    } else {
        BTreeSet::new()
    };
    if !blocking.deps_done(key) {
        failures.extend(
            guard_open_dependencies(dependencies, blocking, relevance, key)
                .into_iter()
                .map(GuardFailure::OpenDependency),
        );
    }
    let accepted = document
        .state
        .overrides
        .get(key)
        .and_then(|overrides| overrides.bypass.as_ref())
        .map(|bypass| &bypass.failures);
    if let Some(accepted) = accepted {
        failures.retain(|failure| !accepted.contains(failure));
    }
    failures
}

/// D4's `deps_done` failures: each relevant or undecided hard dependency of the node, its own
/// and those it inherits through its entry chains, that does not satisfy dependencies.
#[must_use]
pub(crate) fn open_dependencies(
    dependencies: &Dependencies,
    blocking: &Blocking,
    key: &NodeKey,
) -> BTreeSet<NodeKey> {
    let open: BTreeSet<NodeKey> = dependencies
        .of(key, EdgeSet::Pruned)
        .into_iter()
        .filter(|dependency| dependency.class == EdgeClass::Gate)
        .filter(|dependency| !blocking.satisfies(&dependency.node))
        .map(|dependency| dependency.node)
        .collect();
    // An open dependency is exactly what keeps `deps_done` false.
    assert_eq!(
        open.is_empty(),
        blocking.deps_done(key) || dependencies.node_index(key).is_none()
    );
    open
}

/// D4's `deps_done` as a guard reads it: [`open_dependencies`], less an undecided node's
/// condition gates. Those hold work until the answer its relevance reads arrives, and
/// finishing undecided work is accepted with a warning rather than refused (Gating, D4), so
/// they hold neither its transition nor make it stale; every other dependency still does.
#[must_use]
pub(crate) fn guard_open_dependencies(
    dependencies: &Dependencies,
    blocking: &Blocking,
    relevance: &Relevances,
    key: &NodeKey,
) -> BTreeSet<NodeKey> {
    let open = open_dependencies(dependencies, blocking, key);
    if open.is_empty() || relevance.get(key).map(|found| found.value) != Some(Relevance::Undecided)
    {
        return open;
    }
    let held: BTreeSet<NodeKey> = dependencies
        .of(key, EdgeSet::Pruned)
        .into_iter()
        .filter(|dependency| dependency.class == EdgeClass::Gate)
        .filter(|dependency| !matches!(dependency.via, DependencyVia::Condition { .. }))
        .filter(|dependency| open.contains(&dependency.node))
        .map(|dependency| dependency.node)
        .collect();
    assert!(held.is_subset(&open));
    held
}

/// G2: the nodes with an artifact link, read once per pass so each completed node's guard is a
/// lookup (at the limits, thousands of completed nodes and a hundred thousand notes).
#[must_use]
pub(crate) fn artifact_nodes(document: &Document) -> BTreeSet<&NodeKey> {
    document
        .state
        .annotations
        .values()
        .filter(|annotation| annotation.is_artifact())
        .filter_map(|annotation| annotation.body.node.as_ref())
        .collect()
}

/// The guards that need no derived state (D4, A16, G2, B10), for a completed node: a
/// deliverable that requires an artifact has an artifact link (among `artifacts`, from
/// [`artifact_nodes`]), and a placeholder has children or is atomic.
#[must_use]
pub(crate) fn static_failures(
    document: &Document,
    tree: &Tree,
    node: &Node<KeyRefs>,
    artifacts: &BTreeSet<&NodeKey>,
) -> BTreeSet<GuardFailure> {
    let mut found = BTreeSet::new();
    let (placeholder, requires_artifact) = match &node.payload {
        Payload::Deliverable(deliverable) => {
            (deliverable.placeholder, deliverable.requires_artifact)
        }
        Payload::Action(action) => (action.placeholder, false),
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => (false, false),
    };
    if requires_artifact && !artifacts.contains(&node.key) {
        found.insert(GuardFailure::MissingArtifact);
    }
    let atomic = document
        .state
        .nodes
        .get(&node.key)
        .is_some_and(|stored| stored.atomic);
    if placeholder && !atomic && !tree.is_container(&node.key) {
        found.insert(GuardFailure::NotBrokenDown);
    }
    // Only deliverables and actions have these guards (A16).
    assert!(
        found.is_empty() || matches!(node.payload, Payload::Deliverable(_) | Payload::Action(_))
    );
    assert!(found.len() <= 2);
    found
}
