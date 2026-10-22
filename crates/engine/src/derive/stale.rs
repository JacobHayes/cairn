//! `stale` (D4; ARCHITECTURE, Read path: derive, pass 7): a terminal node whose completing
//! guards would now fail, checked against the transition it used: `done` was completed
//! (`deps_done`, `has_artifact`, `has_note`, `broken_down`, and for work `date_arrived`), and
//! `decided` answered (`deps_done`, `date_arrived`), and a `skipped` node is never stale,
//! since its skip required nothing. A milestone is never held by its date: marking it done
//! records when it happened. The
//! failures a guard bypass recorded are accepted (D4: a bypass records the specific failures
//! present), so only a later, distinct failure makes the node stale. Visible, never blocking.
//! Only nodes in scope are judged: a node that is not relevant is out of the journey.
//!
//! Which nodes are stale is decided per node here; their reasons are listed on demand
//! ([`reasons`]), since an open dependency inherited from an ancestor would otherwise be
//! listed once per descendant (up to nodes x depth x 64 entries).
//!
//! Cost at `node_count_max`: the artifact links and notes are indexed once, O(annotations log nodes); a
//! terminal node whose `deps_done` holds is then decided in O(log nodes); any other walks its entry chains once
//! ([`super::dependencies::Dependencies::of`], at most about 1,300 edges), so at most
//! 2,000 x 1,300 steps when every node is a stale terminal node.

use std::collections::BTreeSet;

use cairn_schema::{
    Date, DependencyVia, GuardFailure, KeyRefs, Node, NodeKey, Payload, Relevance, State,
};

use super::blocking::Blocking;
use super::dates::Dates;
use super::dependencies::{Dependencies, EdgeClass, EdgeSet};
use super::relevance::Relevances;
use super::{stored_state, unexpanded};
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
        dates: &Dates,
    ) -> Self {
        let annotated = annotated_nodes(graph.document());
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
                    dates,
                    &annotated,
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
/// transition required, less those its bypass recorded. `annotated` are the nodes with an
/// artifact link or a note ([`annotated_nodes`]).
#[must_use]
pub(crate) fn reasons(
    graph: &Graph,
    relevance: &Relevances,
    dependencies: &Dependencies,
    blocking: &Blocking,
    dates: &Dates,
    annotated: &Annotated<'_>,
    key: &NodeKey,
) -> BTreeSet<GuardFailure> {
    let document = graph.document();
    let Some(node) = document.nodes.get(key) else {
        return BTreeSet::new();
    };
    let stored = stored_state(document, node);
    if stored.skips_beneath(node.kind()) {
        // A skip, or a group marked done, is not guarded: nothing it required can fail.
        return BTreeSet::new();
    }
    let completed = match stored {
        State::Done => true,
        State::Decided => false,
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
        static_failures(document, graph.tree(), node, annotated)
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
    failures.extend(too_early(node, dates).map(GuardFailure::TooEarly));
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

/// A8, D4: the `date_arrived` guard's failure: the date a deliverable's, action's, or
/// decision's own not-before rules hold it to ([`Dates::not_before`]), while that date is
/// after today. None for a milestone (marking it done records when it happened, whatever its
/// pin) and for a group (it has no completing transition).
#[must_use]
pub(crate) fn too_early(node: &Node<KeyRefs>, dates: &Dates) -> Option<Date> {
    match node.payload {
        Payload::Deliverable(_) | Payload::Action(_) | Payload::Decision(_) => {}
        Payload::Milestone(_) | Payload::Group(_) => return None,
    }
    dates
        .not_before(&node.key)
        .filter(|date| *date > dates.today())
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

/// The nodes with an artifact link (G2) and the nodes with a note (G4), read once per pass
/// so each completed node's guard is a lookup (at the limits, thousands of completed nodes
/// and a hundred thousand notes).
#[derive(Debug, Default)]
pub(crate) struct Annotated<'a> {
    artifacts: BTreeSet<&'a NodeKey>,
    notes: BTreeSet<&'a NodeKey>,
}

/// G2, G4: the nodes of `document` that carry an artifact link or a note of their own, a
/// child's or the journey's not counting.
#[must_use]
pub(crate) fn annotated_nodes(document: &Document) -> Annotated<'_> {
    let mut annotated = Annotated::default();
    for annotation in document.state.annotations.values() {
        let Some(node) = annotation.body.node.as_ref() else {
            continue;
        };
        if annotation.is_artifact() {
            annotated.artifacts.insert(node);
        } else if annotation.is_note() {
            annotated.notes.insert(node);
        }
    }
    annotated
}

/// The guards that need no derived state (D4, A16, G2, G4, G5, B10), for a completed node: a
/// deliverable that requires an artifact has an artifact link, a deliverable or action that
/// requires a note has a note (both among `annotated`, from [`annotated_nodes`]), one that
/// requires its checklist has every item ticked (one failure per unticked item), and a
/// placeholder has children or is atomic.
#[must_use]
pub(crate) fn static_failures(
    document: &Document,
    tree: &Tree,
    node: &Node<KeyRefs>,
    annotated: &Annotated<'_>,
) -> BTreeSet<GuardFailure> {
    let mut found = BTreeSet::new();
    let (requires_artifact, requires_note) = match &node.payload {
        Payload::Deliverable(deliverable) => {
            (deliverable.requires_artifact, deliverable.requires_note)
        }
        Payload::Action(action) => (false, action.requires_note),
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => (false, false),
    };
    if requires_artifact && !annotated.artifacts.contains(&node.key) {
        found.insert(GuardFailure::MissingArtifact);
    }
    if requires_note && !annotated.notes.contains(&node.key) {
        found.insert(GuardFailure::MissingNote);
    }
    if node.payload.requires_checklist()
        && let Some(checklist) = node.payload.checklist()
    {
        let ticks = document.state.ticks.get(&node.key);
        for id in checklist.ids() {
            if !ticks.is_some_and(|ticks| ticks.contains(id)) {
                found.insert(GuardFailure::Unticked(id.clone()));
            }
        }
    }
    if unexpanded(document, tree, node) {
        found.insert(GuardFailure::NotBrokenDown);
    }
    // Only deliverables and actions have these guards (A16).
    assert!(
        found.is_empty() || matches!(node.payload, Payload::Deliverable(_) | Payload::Action(_))
    );
    assert!(
        found.len()
            <= 3 + node
                .payload
                .checklist()
                .map_or(0, cairn_schema::Checklist::len)
    );
    found
}
