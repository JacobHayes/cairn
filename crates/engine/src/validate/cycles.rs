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
//! Cost at the limits: building the graph and finding its strongly connected components are
//! both O(instants + edges), about 120,000 steps.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cairn_schema::limits::EXPLANATION_ENTRY_COUNT_MAX;
use cairn_schema::{NodeKey, Subject, Violation, ViolationCode};

use super::GraphCheck;
use crate::derive::dependencies::{Dependencies, Edge, EdgeClass, EdgeSet, EdgeSource, Instant};

/// Reports each dependency cycle once, naming its nodes and the sources of its edges.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let graph = Dependencies::build(check.document, check.tree);
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
            message(&graph, &members),
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
