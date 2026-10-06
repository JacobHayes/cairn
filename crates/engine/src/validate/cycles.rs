//! Dependency cycles (PRD Invariants: the dependency graph, including implicit gates from
//! conditions, inherited requirements, containment, and stage openings, is acyclic). The
//! check reads the gate edges of the full effective dependency graph
//! ([`crate::derive::dependencies`]), which is structural, so routes and journeys are held to
//! it alike and no relevance or override can make a cycle legal. A date-only edge (a stage
//! opening with `gates: false`) never closes a cycle here.
//!
//! Each cycle is reported once, naming its nodes, and its message names the source of each
//! edge on it (PRD Containment: implicit edges name their source in validation messages).
//!
//! Snooze wait cycles (B6): a node snooze makes the snoozed node's own work wait for its
//! target, so a target that transitively depends on the snoozed node (an ancestor of it, a
//! dependent, or another snoozed node waiting the other way) would hold it hidden forever. The
//! same search, with each node snooze as a wait from the snoozed node's start to the target's
//! finish, finds them over the full set, so whether a snooze is legal never changes with an
//! answer. Each snooze on a cycle is reported at its node. They are looked for only when the
//! dependency graph itself has no cycle, so one cause is reported once.
//!
//! Cost at the limits: building the graph and finding its strongly connected components are
//! both O(instants + edges), about 120,000 steps.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cairn_schema::limits::EXPLANATION_ENTRY_COUNT_MAX;
use cairn_schema::{NodeKey, SnoozeTarget, Subject, Violation, ViolationCode};

use super::GraphCheck;
use crate::derive::dependencies::{
    Dependencies, Edge, EdgeClass, EdgeSet, EdgeSource, Instant, Point, Waits,
};

/// Reports each dependency cycle once, naming its nodes and the sources of its edges; then,
/// when there is none, each node snooze on a wait cycle (B6).
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let graph = Dependencies::build(check.document, check.tree);
    let found_before = out.len();
    dependency_cycles(check, &graph, out);
    if out.len() == found_before {
        snooze_cycles(check, &graph, out);
    }
}

/// B6: each node snooze whose target transitively depends on the snoozed node.
fn snooze_cycles(check: &GraphCheck<'_>, graph: &Dependencies, out: &mut Vec<Violation>) {
    let snoozes: Vec<(&NodeKey, NodeKey, Instant, Instant)> = check
        .document
        .state
        .snoozes
        .iter()
        .filter_map(|(snoozed, until)| {
            // A snooze on itself is the state stage's to report.
            let SnoozeTarget::Node(target) = until.clone() else {
                return None;
            };
            if target == *snoozed {
                return None;
            }
            let waiting = Instant::new(graph.node_index(snoozed)?, Point::Start);
            let awaited = Instant::new(graph.node_index(&target)?, Point::Finish);
            Some((snoozed, target, waiting, awaited))
        })
        .collect();
    if snoozes.is_empty() {
        return;
    }
    let waits = Waits::new(
        snoozes
            .iter()
            .map(|(_, _, waiting, awaited)| (*waiting, *awaited)),
    );
    let cycles = graph.cycles_with(&waits);
    for (snoozed, target, waiting, awaited) in snoozes {
        let on_cycle = cycles
            .iter()
            .any(|members| members.contains(&waiting) && members.contains(&awaited));
        if !on_cycle {
            continue;
        }
        let mut found = super::at_node(
            check.tree,
            snoozed,
            ViolationCode::SnoozeCycle,
            format!(
                "{snoozed} is snoozed until {target}, which cannot finish before {snoozed} does (B6)"
            ),
        );
        found.related.push(Subject::Node(target));
        out.push(found);
    }
}

/// Reports each dependency cycle once.
fn dependency_cycles(check: &GraphCheck<'_>, graph: &Dependencies, out: &mut Vec<Violation>) {
    for members in graph.gate_cycles() {
        let nodes: BTreeSet<&NodeKey> = members
            .iter()
            .filter_map(|instant| graph.key(instant.node))
            .collect();
        let Some(first) = nodes.first() else {
            continue;
        };
        let mut found = super::at_node(
            check.tree,
            first,
            ViolationCode::DependencyCycle,
            message(graph, &members),
        );
        found.related = nodes
            .iter()
            .map(|key| Subject::Node((*key).clone()))
            .collect();
        assert!(found.related.len() > 1, "a cycle joins two nodes or more");
        out.push(found);
    }
}

/// The cycle's edges by source, up to `explanation_entry_count_max` of them.
fn message(graph: &Dependencies, members: &BTreeSet<Instant>) -> String {
    let edges: Vec<&Edge> = members
        .iter()
        .flat_map(|instant| graph.waits(*instant, EdgeSet::Full))
        .filter(|edge| edge.class == EdgeClass::Gate && members.contains(&edge.requirement))
        .filter(|edge| edge.dependent.node != edge.requirement.node)
        .collect();
    let mut text = String::from("these nodes wait on each other:");
    let limit = EXPLANATION_ENTRY_COUNT_MAX as usize;
    for edge in edges.iter().take(limit) {
        let (Some(dependent), Some(requirement)) = (
            graph.key(edge.dependent.node),
            graph.key(edge.requirement.node),
        ) else {
            continue;
        };
        let _ = write!(text, " {}", describe(edge.source, dependent, requirement));
    }
    if edges.len() > limit {
        let _ = write!(text, " and {} more", edges.len() - limit);
    }
    text
}

fn describe(source: EdgeSource, dependent: &NodeKey, requirement: &NodeKey) -> String {
    match source {
        EdgeSource::Explicit => format!("[{dependent} requires {requirement}]"),
        EdgeSource::Condition => format!("[{dependent}'s condition reads {requirement}]"),
        EdgeSource::StageOpening => format!("[stage {dependent} opens at {requirement}]"),
        EdgeSource::Containment => format!("[{dependent} contains {requirement}]"),
        EdgeSource::Chain => {
            format!("[{dependent} inherits the requirements and conditions of {requirement}]")
        }
        EdgeSource::Work | EdgeSource::Entry => format!("[{dependent} waits on {requirement}]"),
    }
}
