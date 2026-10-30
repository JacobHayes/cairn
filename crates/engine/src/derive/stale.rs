//! `stale` (D4; ARCHITECTURE, Read path: derive, pass 7): a finished node that something it
//! relied on has gone back on. Two things make one stale, checked against the transition it
//! used (`done` was completed; `decided` answered; a `skipped` node, or a group marked done,
//! is never stale, since its move required nothing):
//!
//! - **a reopened dependency it relied on**: a gate dependency (its own, inherited, its
//!   children; an undecided node's condition gates aside) that is open
//!   now because something was reopened that was done when the node finished. Reopening is the
//!   one signal: a dependency added later, an answer that brings one into scope, or a date
//!   that moves never makes a finished node stale, and neither does finishing ahead of
//!   something open.
//! - **missing evidence**, for a completed node: the evidence guards its completion needed
//!   that fail now, less the failures its bypass recorded (D4: a bypass records the specific
//!   failures present), so only a later, distinct failure counts.
//!
//! Visible, never blocking. Only nodes in scope are judged: a node that is not relevant is out
//! of the journey.
//!
//! "Relied on" reads the record reopening a finish leaves on the node it reopens
//! (`NodeState::reopened`): the dependency satisfied dependencies when the node finished,
//! and was reopened after it. "When" is the journey revision each was recorded at, the
//! journey's own order of writes, so work finished after a reopen, however soon after, did not
//! rely on what was reopened. Two in one patch share a revision and are not ordered, so work
//! finished in the patch that reopens what it waits on is not stale on the graph that patch
//! produced. Reopening a skip or a mark done leaves no record:
//! nothing was done to rely on. A node whose state is derived (a group, a milestone that marks
//! itself done) satisfied dependencies while everything it waits on did ([`Reopens`]), so a
//! group with other work still open then was never relied on; a milestone that marks itself
//! done did only from its date, the one bound read by day, against the node's actual finish
//! date. A dependency added after the node finished is read as if it had been
//! there: today's graph is the only one there is.
//!
//! Which nodes are stale is decided per node here; their reasons are listed on demand
//! ([`reasons`]), since an open dependency inherited from an ancestor would otherwise be
//! listed once per descendant (up to nodes x depth x 64 entries).
//!
//! Cost at `node_count_max`: the artifact links and notes are indexed once, O(annotations log
//! nodes); the reopen spans in one sweep of the instants in blocking's order, each gate edge
//! read once, as blocking reads them; a terminal node whose dependencies are all done is then
//! decided in O(log nodes); any other walks its entry chains once
//! ([`super::dependencies::Dependencies::of`]), so at most 2,000 x 1,300 steps when every node
//! is a stale terminal node.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Date, DependencyVia, GuardFailure, KeyRefs, Node, NodeKey, NodeKind, Payload, Relevance,
    Revision, StaleReason, State,
};

use super::blocking::Blocking;
use super::dates::Dates;
use super::dependencies::{Dependencies, EdgeSet, Instant, NodeIndex, Point};
use super::relevance::Relevances;
use super::{Early, stored_state, unexpanded};
use crate::graph::{Document, Graph, Tree};

/// The stale nodes, in key order, and the reopen records their reasons read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Staleness {
    nodes: BTreeSet<NodeKey>,
    reopens: Reopens,
}

impl Staleness {
    /// D4: the stale nodes of a derived journey.
    #[must_use]
    pub(crate) fn pass(graph: &Graph, early: &Early, dates: &Dates, blocking: &Blocking) -> Self {
        let relevance = &early.relevance;
        let dependencies = &early.dependencies;
        let annotated = annotated_nodes(graph.document());
        let reopens = Reopens::read(graph, early, dates, blocking);
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
                    &reopens,
                    &annotated,
                    &node.key,
                );
                !found.is_empty()
            })
            .map(|node| node.key.clone())
            .collect();
        assert!(nodes.iter().all(|key| relevance.in_scope(key)));
        Self { nodes, reopens }
    }

    /// The node is stale.
    #[must_use]
    pub(crate) fn contains(&self, key: &NodeKey) -> bool {
        self.nodes.contains(key)
    }

    /// The reopen records the reasons read.
    #[must_use]
    pub(crate) fn reopens(&self) -> &Reopens {
        &self.reopens
    }
}

/// When an instant was last satisfied without a break, as the records tell, in journey
/// revisions: from its finish ([`Revision::NONE`]: as far back as the records go) to its
/// reopen (`None`: it still is), and for a milestone that marks itself done only from its date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    from: Revision,
    to: Option<Revision>,
    from_day: Option<Date>,
}

impl Span {
    /// Always.
    const ALWAYS: Self = Self {
        from: Revision::NONE,
        to: None,
        from_day: None,
    };

    /// When both spans hold, none when they share no revision.
    fn and(self, other: Self) -> Option<Self> {
        let from = self.from.max(other.from);
        let to = match (self.to, other.to) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        };
        let from_day = self.from_day.max(other.from_day);
        let empty = to.is_some_and(|to| from >= to);
        (!empty).then_some(Self { from, to, from_day })
    }

    /// Whether it held when work finished at `revision`, on `day`: satisfied by then, and
    /// reopened after.
    fn holds(self, revision: Revision, day: Date) -> bool {
        self.from <= revision
            && self.to.is_none_or(|to| revision < to)
            && self.from_day.is_none_or(|from| from <= day)
    }
}

/// Per node that does not satisfy dependencies now, when it last did, read from the reopen
/// records ([`Span`]): a node reopened has its own record; a node whose state is derived (a
/// group, a skip waiting on kept work, a milestone that marks itself done) did while
/// everything it waits on did, and a milestone that marks itself done only from its date. A
/// node that never did has none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Reopens {
    spans: BTreeMap<NodeKey, Span>,
}

impl Reopens {
    /// One sweep in the order blocking decided satisfaction, each instant read from those it
    /// waits on, the way blocking reads them.
    fn read(graph: &Graph, early: &Early, dates: &Dates, blocking: &Blocking) -> Self {
        let dependencies = &early.dependencies;
        let mut spans: Vec<Option<Span>> = vec![None; dependencies.node_count() * Point::ALL.len()];
        for &instant in blocking.order() {
            let span = Self::instant(graph, early, dates, &spans, instant);
            if let Some(slot) = spans.get_mut(instant.slot()) {
                *slot = span;
            }
        }
        let mut reopens = Reopens::default();
        for (at, key) in dependencies.keys().iter().enumerate() {
            let finish = Instant::new(NodeIndex::from_position(at), Point::Finish);
            if let Some(Some(span)) = spans.get(finish.slot())
                && span.to.is_some()
                && early.relevance.in_scope(key)
                && !blocking.satisfies(key)
            {
                reopens.spans.insert(key.clone(), *span);
            }
        }
        reopens
    }

    /// One instant's span, every instant it waits on read.
    fn instant(
        graph: &Graph,
        early: &Early,
        dates: &Dates,
        spans: &[Option<Span>],
        instant: Instant,
    ) -> Option<Span> {
        let dependencies = &early.dependencies;
        let span_of = |requirement: Instant| spans.get(requirement.slot()).copied().flatten();
        let waits = || {
            dependencies
                .waits(instant, EdgeSet::Pruned)
                .try_fold(Span::ALWAYS, |span, edge| {
                    span_of(edge.requirement).and_then(|other| span.and(other))
                })
        };
        if instant.point != Point::Finish {
            return waits();
        }
        let node = dependencies
            .key(instant.node)
            .and_then(|key| graph.node(key))?;
        let document = graph.document();
        let stored = stored_state(document, node);
        let skips = &early.skips;
        if stored.skips_beneath(node.kind()) || skips.skipped_by(&node.key).is_some() {
            return skips
                .kept_work(&node.key)
                .iter()
                .try_fold(Span::ALWAYS, |span, kept| {
                    let at = dependencies.node_index(kept)?;
                    span_of(Instant::new(at, Point::Finish)).and_then(|other| span.and(other))
                });
        }
        let record = document.state.nodes.get(&node.key);
        if stored.is_terminal() {
            let from = record
                .and_then(|stored| stored.finished_revision)
                .unwrap_or(Revision::NONE);
            return Some(Span {
                from,
                ..Span::ALWAYS
            });
        }
        if node.kind() == NodeKind::Group {
            return waits();
        }
        if let Some(reopened) = record.and_then(|stored| stored.reopened) {
            // A finish reopened, a milestone's marked by hand among them.
            return Some(Span {
                from: reopened.finished_revision,
                to: Some(reopened.revision),
                from_day: None,
            });
        }
        if let Payload::Milestone(milestone) = &node.payload
            && milestone.auto_done
            && early.relevance.value(&node.key) == Relevance::Relevant
        {
            let date = dates.effective_date(&node.key)?.date;
            return waits()?.and(Span {
                from_day: Some(date),
                ..Span::ALWAYS
            });
        }
        None
    }

    /// Whether work that finished at `revision`, on `day`, relied on open `dependency`: it
    /// satisfied dependencies then, and has been reopened since.
    fn relied_on(&self, dependency: &NodeKey, revision: Revision, day: Date) -> bool {
        self.spans
            .get(dependency)
            .is_some_and(|span| span.holds(revision, day))
    }
}

/// D4: why the node is stale, empty when it is not: each open dependency it relied on that a
/// reopen took back, and for a completed node the evidence its completion needed that is now
/// missing, less what its bypass recorded. `annotated` are the nodes with an artifact link or
/// a note ([`annotated_nodes`]).
#[must_use]
pub(crate) fn reasons(
    graph: &Graph,
    relevance: &Relevances,
    dependencies: &Dependencies,
    blocking: &Blocking,
    reopens: &Reopens,
    annotated: &Annotated<'_>,
    key: &NodeKey,
) -> BTreeSet<StaleReason> {
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
    let accepted = document
        .state
        .overrides
        .get(key)
        .and_then(|overrides| overrides.bypass.as_ref())
        .map(|bypass| &bypass.failures);
    if let Some(accepted) = accepted {
        failures.retain(|failure| !accepted.contains(failure));
    }
    let mut found: BTreeSet<StaleReason> = failures.into_iter().map(StaleReason::from).collect();
    let finished = document
        .state
        .nodes
        .get(key)
        .and_then(|stored| stored.finished_revision.zip(stored.finished_on));
    if let Some((revision, day)) = finished
        && !blocking.deps_done(key)
    {
        found.extend(
            guard_open_dependencies(dependencies, blocking, relevance, key)
                .into_iter()
                .filter(|open| reopens.relied_on(open, revision, day))
                .map(StaleReason::Reopened),
        );
    }
    found
}

/// Each relevant or undecided hard dependency of the node, its own and those it inherits
/// through its entry chains, that does not satisfy dependencies.
#[must_use]
pub(crate) fn open_dependencies(
    dependencies: &Dependencies,
    blocking: &Blocking,
    key: &NodeKey,
) -> BTreeSet<NodeKey> {
    let open: BTreeSet<NodeKey> = dependencies
        .of(key, EdgeSet::Pruned)
        .into_iter()
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

/// [`open_dependencies`], less an undecided node's condition gates: those hold work until the
/// answer its relevance reads arrives, and finishing undecided work is accepted with a warning
/// (Gating, D4), so they never make it stale; every other dependency still counts.
#[must_use]
pub(crate) fn guard_open_dependencies(
    dependencies: &Dependencies,
    blocking: &Blocking,
    relevance: &Relevances,
    key: &NodeKey,
) -> BTreeSet<NodeKey> {
    let open = open_dependencies(dependencies, blocking, key);
    let undecided = relevance.get(key).map(|found| found.value) == Some(Relevance::Undecided);
    if open.is_empty() || !undecided {
        return open;
    }
    let held: BTreeSet<NodeKey> = dependencies
        .of(key, EdgeSet::Pruned)
        .into_iter()
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
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group => (false, false),
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
