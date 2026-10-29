//! The stage that needs derived state (D4, B6; ARCHITECTURE, Write path: apply): the final
//! candidate is derived once, with the patch's today, and every transition the patch
//! attempted that needs its node in scope (start, complete or a first answer) is held to
//! relevance on it, so relevance is never satisfied only for a moment. Relevance refuses a
//! node that is not relevant (force include is the escape hatch); an undecided node passes,
//! so finishing undecided work is accepted and reported as a consequence (D7) that it may not
//! apply. Order is never checked: a node is started, completed or answered whatever it waits
//! on and whatever its own date. A snooze the patch made must sit on a node in scope that is
//! not closed: not effectively skipped and not a milestone that reads as done (B6; a blocked
//! node may be snoozed, Gating); a group has work left beneath it. An unsnooze of a node with
//! no snooze of its own is refused while a container's snooze still holds over it on that
//! graph, naming the container (B6).
//!
//! Each failure names the transition's mutation and, in `caused_by`, the patch's other
//! mutations that brought it about: those that wrote the structure of the node, or of the
//! condition behind its relevance, or the answer that decided it.
//!
//! Cost at the limits: one derive of the candidate (passes 1 to 5 and `stale`), skipped when
//! the patch made no such transition and no snooze; then for each failure one pass over the
//! patch's writes, at most the mutation limit's events.

use std::collections::BTreeSet;

use cairn_schema::{
    Event, GraphKey, GraphRecord, NodeKey, Record, RecordKey, Relevance, Subject, ViolationCode,
    Write,
};

use super::Check;
use crate::derive::relevance::Producer;
use crate::derive::{Derived, derive_at};
use crate::graph::{Graph, Tree};
use crate::validate::at_node;

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
    let derived = derive_at(
        &graph,
        Some(journey.header.created_on),
        session.inputs.today,
        &session.candidate.deployment,
    );
    for (key, ordinal) in &session.guarded {
        relevance(check, &graph, &derived, key, *ordinal);
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
         does not take this transition; force include it to go ahead",
    );
    violation.at.mutation = Some(at);
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
        // Closed: effectively skipped, or a milestone that reads as done (F1).
        if derived.snoozable(graph, key) {
            continue;
        }
        let nothing_beneath = derived.group_without_work(graph, key);
        let message = if nothing_beneath {
            "a group is snoozed only while work in scope is left beneath it"
        } else {
            "only a node in scope with something left to do is snoozed"
        };
        let mut violation = at_node(tree, key, ViolationCode::SnoozeNotActionable, message);
        violation.at.mutation = Some(*at);
        check.violations.push(violation);
    }
}

/// B6: an unsnooze of a node whose snooze is not its own: refused while a container's snooze
/// holds over it, naming the container to unsnooze instead; otherwise the node is not snoozed.
fn unsnoozes_through(check: &mut Check<'_, '_>, tree: &Tree, derived: &Derived) {
    for (key, at) in &check.session.unsnoozed_through {
        let held = derived.blocking().snoozed_via(key);
        let (code, message) = match held {
            Some(container) => (
                ViolationCode::SnoozedThroughContainer,
                format!("{key} is snoozed through {container}; unsnooze {container} instead"),
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
            GraphRecord::NodeState { node, .. } => vec![Shape::State(node)],
            GraphRecord::Answer { decision, .. } => vec![Shape::State(decision)],
            GraphRecord::Overrides { node, .. } => vec![Shape::Override(node)],
            GraphRecord::Pin { .. }
            | GraphRecord::Role(_)
            | GraphRecord::Kind(_)
            | GraphRecord::DefaultOwner(_)
            | GraphRecord::Participation { .. }
            | GraphRecord::Resource { .. }
            | GraphRecord::RetiredKey(_)
            | GraphRecord::Insertion(_)
            | GraphRecord::Tick { .. }
            | GraphRecord::RoleFill { .. }
            | GraphRecord::Snooze { .. }
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
            GraphKey::NodeState(node) | GraphKey::Answer(node) => vec![Shape::State(node)],
            GraphKey::Overrides(node) => vec![Shape::Override(node)],
            GraphKey::Pin(_)
            | GraphKey::Role(_)
            | GraphKey::Kind(_)
            | GraphKey::DefaultOwner
            | GraphKey::Participation { .. }
            | GraphKey::Resource { .. }
            | GraphKey::RetiredKey(_)
            | GraphKey::Insertion(_)
            | GraphKey::Tick { .. }
            | GraphKey::RoleFill(_)
            | GraphKey::Snooze(_)
            | GraphKey::Annotation { .. } => Vec::new(),
        },
        Write::Put(_) | Write::Remove(_) | Write::CopyGraph { .. } => Vec::new(),
    }
}
