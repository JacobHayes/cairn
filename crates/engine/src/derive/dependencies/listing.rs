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

use super::{Dependencies, EdgeSet, EdgeSource, EffectiveDependency, Instant};

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
        let mut found = BTreeSet::new();
        let mut visited: BTreeSet<Instant> = BTreeSet::new();
        let mut stack = vec![Instant::new(node, super::Point::Start)];
        while let Some(instant) = stack.pop() {
            if !visited.insert(instant) {
                continue;
            }
            for edge in self.waits(instant, set) {
                let Some(owner) = self.key(edge.dependent.node) else {
                    continue;
                };
                let Some(required) = self.key(edge.requirement.node) else {
                    continue;
                };
                let via = match edge.source {
                    EdgeSource::Work => continue,
                    EdgeSource::Entry | EdgeSource::Chain => {
                        stack.push(edge.requirement);
                        continue;
                    }
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
            }
        }
        // The node's start and its two entries, then two per ancestor.
        let bound = 3 + 2 * CONTAINMENT_DEPTH_MAX as usize;
        assert!(visited.len() <= bound, "the walk stays on the entry chains");
        found.into_iter().collect()
    }
}
