//! The stage that needs derived state (D4, B6; ARCHITECTURE, Write path: apply): the final
//! candidate is derived once, with the patch's today, and every guarded transition the patch
//! attempted (complete, a first answer, reach) is held to relevance and `deps_done` on it, so
//! a guard is never satisfied only for a moment and a bulk completion is accepted in any
//! order (A17). Relevance refuses a node that is not relevant and has no bypass (force include
//! is the escape hatch); an undecided node passes, and neither do its condition gates hold it
//! (`deps_done` as a guard reads it), so finishing undecided work is accepted and reported as
//! a consequence (D7) that it may not apply. A `deps_done` bypass applied in the same patch
//! accepts the open dependencies present, and they are recorded on it. A snooze the patch made must sit on a node in scope that is not closed: not
//! effectively skipped and not a milestone that reads as reached (B6; a blocked node may be
//! snoozed, Gating); a group has work left beneath it. An unsnooze of a node with no snooze of
//! its own is refused while a container's snooze still holds over it on that graph, naming the
//! container (B6).
//!
//! Each failure names the transition's mutation and, in `caused_by`, the patch's other
//! mutations that brought it about: those that wrote the structure of the node, of the
//! dependency, or of the ancestor, condition, or stage that carries it; the state, pin, or
//! overrides of the dependency; or the answer behind a relevance that brought it or the node
//! into or out of scope.
//!
//! Cost at the limits: one derive of the candidate (passes 1 to 5 and `stale`), skipped when
//! the patch made no guarded transition and no snooze; then for each guarded node one walk of
//! its entry chains (at most about 1,300 edges), and for each failure one pass over the
//! patch's writes, at most the mutation limit's events.

use std::collections::BTreeSet;

use cairn_schema::{
    DependencyVia, Event, GraphKey, GraphRecord, Guard, GuardFailure, NodeKey, NodeKind, Record,
    RecordKey, Relevance, Subject, ViolationCode, Write,
};

use super::Check;
use crate::derive::relevance::Producer;
use crate::derive::{Derived, EdgeSet, EffectiveDependency, derive_at};
use crate::graph::{Graph, Tree};
use crate::validate::at_node;
use cairn_schema::RankConstants;

/// What a write shapes, for naming the mutations behind a failure.
enum Shape<'w> {
    /// A node's own structure: the node, a field, or an edge it is an end of.
    Structure(&'w NodeKey),
    /// A node's state: its stored state or its answer.
    State(&'w NodeKey),
    /// A node's overrides: force include, keep, or a bypass.
    Override(&'w NodeKey),
}

/// The nodes whose records count toward a failure, by what of them.
#[derive(Default)]
struct Involved<'k> {
    structure: BTreeSet<&'k NodeKey>,
    state: BTreeSet<&'k NodeKey>,
    overrides: BTreeSet<&'k NodeKey>,
}

/// Runs the derived checks on the patch's journey.
pub(super) fn check(check: &mut Check<'_, '_>) {
    let session = check.session;
    if session.guarded.is_empty()
        && session.snoozed.is_empty()
        && session.unsnoozed_through.is_empty()
    {
        return;
    }
    let Some(journey) = session.journey() else {
        return;
    };
    // Derive needs a graph that holds every invariant; the graph stages reported it.
    if !check.journey_valid {
        return;
    }
    let tree = Tree::build(&journey.graph);
    let graph = Graph::trusted(journey.graph.clone(), tree.clone());
    // Nothing the guards read depends on the rank constants: the defaults stand in.
    let derived = derive_at(
        &graph,
        Some(journey.header.created_on),
        session.inputs.today,
        &session.candidate.deployment,
        &RankConstants::default(),
    );
    for (key, ordinal) in &session.guarded {
        relevance(check, &graph, &derived, key, *ordinal);
        deps_done(check, &graph, &derived, key, *ordinal);
    }
    snoozes(check, &graph, &derived);
    unsnoozes_through(check, &tree, &derived);
}

/// D4: a guarded transition needs its node in scope on the graph the patch produces: an
/// undecided node takes it, and the consequences report that it may not apply (D7).
fn relevance(check: &mut Check<'_, '_>, graph: &Graph, derived: &Derived, key: &NodeKey, at: u32) {
    let Some(found) = derived.relevance().get(key) else {
        return;
    };
    if found.value != Relevance::NotRelevant {
        return;
    }
    let mut involved = Involved::default();
    involved.structure.insert(key);
    involved.overrides.insert(key);
    if let Producer::Condition { on, decisions } = &found.producer {
        involved.structure.insert(on);
        involved.state.extend(decisions);
    }
    let mut violation = at_node(
        graph.tree(),
        key,
        ViolationCode::NotRelevant,
        "the node is not relevant on the graph this patch produces, and a node out of scope \
         does not take this transition (D4); force include it to go ahead",
    );
    violation.at.mutation = Some(at);
    violation.caused_by = causes(check.events, at, &involved);
    check.violations.push(violation);
}

/// D4: a guarded transition needs every hard dependency of its node satisfied on the graph
/// the patch produces (an undecided node's condition gates aside), or a bypass in the patch
/// that accepts the open ones.
fn deps_done(check: &mut Check<'_, '_>, graph: &Graph, derived: &Derived, key: &NodeKey, at: u32) {
    let open = derived.guard_open_dependencies(key);
    if open.is_empty() {
        return;
    }
    let failures: BTreeSet<GuardFailure> = open
        .iter()
        .cloned()
        .map(GuardFailure::OpenDependency)
        .collect();
    let session = check.session;
    let covered = session.bypassed.contains_key(key)
        && graph
            .document()
            .state
            .overrides
            .get(key)
            .and_then(|overrides| overrides.bypass.as_ref())
            .is_some_and(|bypass| bypass.guards.contains(&Guard::DepsDone));
    if covered {
        check
            .bypassed
            .entry(key.clone())
            .or_default()
            .extend(failures);
        return;
    }
    // Where each open dependency comes from, whatever the node's own state: the ancestor,
    // condition, or stage whose structure carries it.
    let sources: Vec<EffectiveDependency> = derived
        .dependencies()
        .of(key, EdgeSet::Pruned)
        .into_iter()
        .filter(|dependency| open.contains(&dependency.node))
        .collect();
    let mut involved = Involved::default();
    involved.structure.insert(key);
    involved.structure.extend(&open);
    involved
        .structure
        .extend(sources.iter().filter_map(|source| holder(&source.via)));
    involved.state.extend(&open);
    involved.overrides.extend(&open);
    // An answer or condition that brought an open dependency into scope.
    for dependency in &open {
        if let Some(found) = derived.relevance().get(dependency)
            && let Producer::Condition { on, decisions } = &found.producer
        {
            involved.structure.insert(on);
            involved.state.extend(decisions);
        }
    }
    let mut violation = at_node(
        graph.tree(),
        key,
        ViolationCode::GuardFailed,
        format!(
            "this transition needs every hard dependency satisfied, and {} are open on the graph \
             this patch produces (D4); bypass deps_done with a reason to go ahead anyway",
            open.len()
        ),
    );
    violation.at.mutation = Some(at);
    violation.bypassable = Some(Guard::DepsDone);
    violation.failures = failures;
    violation.caused_by = causes(check.events, at, &involved);
    check.violations.push(violation);
}

/// B6: a snooze the patch made sits on a node in scope that is not closed.
fn snoozes(check: &mut Check<'_, '_>, graph: &Graph, derived: &Derived) {
    let tree = graph.tree();
    let session = check.session;
    let Some(journey) = session.journey() else {
        return;
    };
    for (key, at) in &session.snoozed {
        // A later transition in the patch cleared it.
        if !journey.graph.state.snoozes.contains_key(key) {
            continue;
        }
        // Closed: effectively skipped, or a milestone that reads as reached (F1).
        let out_of_scope = !derived.relevance().in_scope(key);
        let nothing_beneath = is_group(graph, key) && !work_beneath(graph, derived, key);
        if !out_of_scope && !derived.blocking().closed(key) && !nothing_beneath {
            continue;
        }
        let message = if nothing_beneath {
            "a group is snoozed only while work in scope is left beneath it (B6)"
        } else {
            "only a node in scope with something left to do is snoozed (B6)"
        };
        let mut violation = at_node(tree, key, ViolationCode::SnoozeNotActionable, message);
        violation.at.mutation = Some(*at);
        check.violations.push(violation);
    }
}

fn is_group(graph: &Graph, key: &NodeKey) -> bool {
    graph
        .node(key)
        .is_some_and(|node| node.kind() == NodeKind::Group)
}

/// B6: whether a node beneath the container is in scope with something left to do.
fn work_beneath(graph: &Graph, derived: &Derived, key: &NodeKey) -> bool {
    graph.tree().descendants(key).iter().any(|below| {
        !is_group(graph, below)
            && derived.relevance().in_scope(below)
            && !derived.blocking().closed(below)
    })
}

/// B6: an unsnooze of a node whose snooze is not its own: refused while a container's snooze
/// holds over it, naming the container to unsnooze instead; otherwise the node is not snoozed.
fn unsnoozes_through(check: &mut Check<'_, '_>, tree: &Tree, derived: &Derived) {
    for (key, at) in &check.session.unsnoozed_through {
        let held = derived.blocking().snoozed_via(key);
        let (code, message) = match held {
            Some(container) => (
                ViolationCode::SnoozedThroughContainer,
                format!("{key} is snoozed through {container}; unsnooze {container} instead (B6)"),
            ),
            None => (
                ViolationCode::UnresolvedReference,
                "the node is not snoozed".to_owned(),
            ),
        };
        let mut violation = at_node(tree, key, code, message);
        violation.at.mutation = Some(*at);
        if let Some(container) = held {
            violation.related.push(Subject::Node(container.clone()));
        }
        check.violations.push(violation);
    }
}

/// The patch's mutations, other than `primary`, whose writes shape an involved node.
fn causes(events: &[Event], primary: u32, involved: &Involved<'_>) -> BTreeSet<u32> {
    let counts = |shape: &Shape<'_>| match shape {
        Shape::Structure(node) => involved.structure.contains(node),
        Shape::State(node) => involved.state.contains(node),
        Shape::Override(node) => involved.overrides.contains(node),
    };
    let found: BTreeSet<u32> = events
        .iter()
        .filter(|event| event.ordinal != primary)
        .filter(|event| {
            event
                .delta
                .iter()
                .any(|write| shapes(write).iter().any(counts))
        })
        .map(|event| event.ordinal)
        .collect();
    assert!(!found.contains(&primary));
    found
}

/// The node whose structure carries a dependency other than the dependent's own.
fn holder(via: &DependencyVia) -> Option<&NodeKey> {
    match via {
        DependencyVia::Explicit | DependencyVia::Containment => None,
        DependencyVia::Inherited { ancestor } => Some(ancestor),
        DependencyVia::Condition { condition_on } => Some(condition_on),
        DependencyVia::StageOpening { group } => Some(group),
    }
}

/// What a write shapes: none for records no guard reads.
fn shapes(write: &Write) -> Vec<Shape<'_>> {
    match write {
        Write::Put(Record::Graph { record, .. }) => match record {
            GraphRecord::Node(node) => vec![Shape::Structure(&node.key)],
            GraphRecord::NodeField { node, .. } => vec![Shape::Structure(node)],
            GraphRecord::Edge(edge) => {
                vec![
                    Shape::Structure(&edge.node),
                    Shape::Structure(&edge.requires),
                ]
            }
            // A pin moves an `auto_reach` milestone's date, so whether it reads as reached.
            GraphRecord::NodeState { node, .. } | GraphRecord::Pin { node, .. } => {
                vec![Shape::State(node)]
            }
            GraphRecord::Answer { decision, .. } => vec![Shape::State(decision)],
            GraphRecord::Overrides { node, .. } => vec![Shape::Override(node)],
            GraphRecord::Role(_)
            | GraphRecord::Kind(_)
            | GraphRecord::DefaultOwner(_)
            | GraphRecord::Participation { .. }
            | GraphRecord::Resource { .. }
            | GraphRecord::RetiredKey(_)
            | GraphRecord::Insertion(_)
            | GraphRecord::LocalEdit { .. }
            | GraphRecord::RoleFill { .. }
            | GraphRecord::Snooze { .. }
            | GraphRecord::Tombstone(_)
            | GraphRecord::Annotation(_) => Vec::new(),
        },
        Write::Remove(RecordKey::InGraph { key, .. }) => match key {
            GraphKey::Node(node) | GraphKey::NodeField { node, .. } => {
                vec![Shape::Structure(node)]
            }
            GraphKey::Edge(edge) => {
                vec![
                    Shape::Structure(&edge.node),
                    Shape::Structure(&edge.requires),
                ]
            }
            GraphKey::NodeState(node) | GraphKey::Answer(node) | GraphKey::Pin(node) => {
                vec![Shape::State(node)]
            }
            GraphKey::Overrides(node) => vec![Shape::Override(node)],
            GraphKey::Role(_)
            | GraphKey::Kind(_)
            | GraphKey::DefaultOwner
            | GraphKey::Participation { .. }
            | GraphKey::Resource { .. }
            | GraphKey::RetiredKey(_)
            | GraphKey::Insertion(_)
            | GraphKey::LocalEdit { .. }
            | GraphKey::RoleFill(_)
            | GraphKey::Snooze(_)
            | GraphKey::Tombstone(_)
            | GraphKey::Annotation { .. } => Vec::new(),
        },
        Write::Put(_) | Write::Remove(_) | Write::CopyGraph { .. } => Vec::new(),
    }
}
