//! One node's effective dependencies, read off the graph by walking its entry chains (PRD
//! Containment: an ancestor's requirement applies to every descendant, and each implicit
//! edge names its source). From the node's start the walk follows its own entries and the
//! chain links up through its ancestors' entries, collecting the requirements, openings, and
//! condition gates waiting there, and the children its start waits on. Nothing is copied per
//! descendant: the list is built on demand, at most 2 x `containment_depth_max` entries
//! visited.

use std::collections::BTreeSet;

use cairn_schema::limits::CONTAINMENT_DEPTH_MAX;
use cairn_schema::{DependencyVia, NodeKey};

use super::{Dependencies, Edge, EdgeSet, EdgeSource, EffectiveDependency, Instant, NodeIndex};

impl Dependencies {
    /// The node's effective dependencies in the set, each with how it arose: its own
    /// explicit edges, its stage opening, and its condition gates; its ancestors' requirements
    /// (`Inherited`), stage openings, and condition gates, which reach it through the entry
    /// chains; and, for a container, its children (`Containment`). A dependency reached two
    /// ways is listed once per way. Sorted.
    ///
    /// # Panics
    ///
    /// Never for a key in the graph: the walk is bounded by the depth limit.
    #[must_use]
    pub fn of(&self, key: &NodeKey, set: EdgeSet) -> Vec<EffectiveDependency> {
        let Some(node) = self.node_index(key) else {
            return Vec::new();
        };
        self.walk(key, vec![Instant::new(node, super::Point::Start)], set)
    }

    /// What waits at the node's entries, its own and inherited, in the set: what it passes
    /// down to its descendants, whatever its own state or relevance (a not-relevant
    /// container's requirement entry stays as a pass-through). For a node with no children,
    /// its start's dependencies but its children's, which it has none of. Sorted.
    #[must_use]
    pub fn passed_down(&self, key: &NodeKey, set: EdgeSet) -> Vec<EffectiveDependency> {
        let Some(node) = self.node_index(key) else {
            return Vec::new();
        };
        let entries = if self.is_container(node) {
            vec![
                Instant::new(node, super::Point::Entry),
                Instant::new(node, super::Point::ConditionEntry),
            ]
        } else {
            vec![Instant::new(node, super::Point::Start)]
        };
        self.walk(key, entries, set)
    }

    /// The dependencies reached from `from` along the entry chains.
    fn walk(&self, key: &NodeKey, from: Vec<Instant>, set: EdgeSet) -> Vec<EffectiveDependency> {
        let mut found = BTreeSet::new();
        let from: Vec<(Instant, bool)> = from.into_iter().map(|at| (at, false)).collect();
        self.carried(
            &from,
            set,
            |_| false,
            |edge, _| {
                let (Some(owner), Some(required)) = (
                    self.key(edge.dependent.node),
                    self.key(edge.requirement.node),
                ) else {
                    return;
                };
                let via = match edge.source {
                    EdgeSource::Work | EdgeSource::Entry | EdgeSource::Chain => return,
                    EdgeSource::Containment => DependencyVia::Containment,
                    EdgeSource::Explicit if owner == key => DependencyVia::Explicit,
                    EdgeSource::Explicit => DependencyVia::Inherited {
                        ancestor: owner.clone(),
                    },
                    EdgeSource::Condition => DependencyVia::Condition {
                        condition_on: owner.clone(),
                    },
                    EdgeSource::StageOpening => DependencyVia::StageOpening {
                        group: owner.clone(),
                    },
                };
                found.insert(EffectiveDependency {
                    node: required.clone(),
                    via,
                    class: edge.class,
                });
            },
        );
        found.into_iter().collect()
    }

    /// The one walk of the entry chains: from each instant in `from` (with whether the path
    /// to it is already conditional), follows the entry and chain links up through the
    /// ancestors and calls `visit` for each other edge waiting on the way (a requirement,
    /// opening, condition gate, or containment link), with whether the path it came by
    /// passes through a node `undecided` names, the edge's own two nodes included.
    pub(crate) fn carried(
        &self,
        from: &[(Instant, bool)],
        set: EdgeSet,
        undecided: impl Fn(NodeIndex) -> bool,
        mut visit: impl FnMut(&Edge, bool),
    ) {
        let mut visited: BTreeSet<Instant> = BTreeSet::new();
        let mut stack: Vec<(Instant, bool)> = from.to_vec();
        while let Some((instant, conditional)) = stack.pop() {
            if !visited.insert(instant) {
                continue;
            }
            let conditional = conditional || undecided(instant.node);
            for edge in self.waits(instant, set) {
                let path = conditional || undecided(edge.requirement.node);
                match edge.source {
                    EdgeSource::Work => {}
                    EdgeSource::Entry | EdgeSource::Chain => stack.push((edge.requirement, path)),
                    EdgeSource::Containment
                    | EdgeSource::Explicit
                    | EdgeSource::Condition
                    | EdgeSource::StageOpening => visit(edge, path),
                }
            }
        }
        // The walk's first instants and a container's two entries, then two per ancestor.
        let bound = from.len() + 2 + 2 * CONTAINMENT_DEPTH_MAX as usize;
        assert!(visited.len() <= bound, "the walk stays on the entry chains");
    }
}
