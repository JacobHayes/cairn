//! C2, semantic zoom: one aggregation level of the canvas. The rules are fixed here, never
//! authored:
//!
//! - A node is visible when its kind is shown and it is within the drilled-in container (a
//!   proper descendant; every node when none is drilled into).
//! - Each node within the container has a stand-in: itself when visible, else its nearest
//!   visible ancestor within the container, else none. A visible node is drawn under the
//!   stand-in of its parent, or at the top level; a hidden one rolls up into its stand-in.
//! - Edges are the explicit requirements, condition gates, and stage openings of the full
//!   structural graph (C1 draws not-relevant nodes grayed, not hidden). Each is drawn between
//!   its ends' stand-ins; it is left off when either has none (still traceable, C7) and when
//!   both land on the same node; edges landing on the same pair collapse into one.
//! - A blocked visible node shows the hidden-prerequisites marker for each unsatisfied gate
//!   dependency (its own, inherited, condition, or opening; not its children, which roll up
//!   into it) that no drawn edge stands for: the edge from the dependency's stand-in to the
//!   stand-in of the node holding it (the node itself, or the ancestor whose requirement,
//!   condition, or opening it is) is not drawn. So hiding a kind never makes blocked work
//!   look free.
//! - Each visible container carries its roll-ups (C2's badges), each node its display state
//!   (D8), and each group its legacy group state (D1).
//!
//! Cost at `node_count_max` (2,000 nodes, about 110,000 edges): stand-ins in one pass in tree
//! order, O(nodes); the edges in one pass with a sorted map, O(edges log edges); the marker
//! walks a blocked node's entry chains (at most about 1,300 edges each, 2.6 million in all);
//! a group's state reads its descendants (2,000 x 2,000 at worst) and a container's roll-up
//! its children, O(nodes) in all.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    DependencyVia, EdgeOrigin, Level, LevelEdge, LevelNode, NodeKey, NodeKind, PeakGravity,
    Relevance, RollUp, State, UnderlyingEdge,
};

use super::{DerivedJourney, ProjectionError};
use crate::derive::dependencies::{EdgeClass, EdgeSet, EdgeSource};
use crate::derive::held_by;

/// Each node within the level and its stand-in: itself when visible, else its nearest visible
/// ancestor within the level.
struct StandIns<'a> {
    within: Vec<&'a NodeKey>,
    of: BTreeMap<&'a NodeKey, Option<&'a NodeKey>>,
}

impl<'a> StandIns<'a> {
    fn get(&self, key: &NodeKey) -> Option<&'a NodeKey> {
        self.of.get(key).copied().flatten()
    }

    fn visible(&self, key: &NodeKey) -> bool {
        self.get(key) == Some(key)
    }
}

impl<'a> DerivedJourney<'a> {
    /// C2: the level for the shown kinds within the drilled-in container (the whole journey
    /// when none): the visible nodes, each under its nearest visible ancestor, with what rolls
    /// up into it, its hidden-prerequisites marker, and for a container its roll-ups; the
    /// edges re-targeted to visible stand-ins, duplicates collapsed and self-edges dropped.
    ///
    /// # Errors
    ///
    /// When the container is not a node of the journey.
    pub fn level(
        &self,
        shown: &BTreeSet<NodeKind>,
        container: Option<&NodeKey>,
    ) -> Result<Level, ProjectionError> {
        if let Some(key) = container {
            self.known(key)?;
        }
        let stand_ins = self.stand_ins(shown, container);
        let mut rolled_up: BTreeMap<&NodeKey, Vec<NodeKey>> = BTreeMap::new();
        for key in &stand_ins.within {
            match stand_ins.get(key) {
                Some(stand_in) if stand_in != *key => {
                    rolled_up.entry(stand_in).or_default().push((*key).clone());
                }
                Some(_) | None => {}
            }
        }
        let tree = self.graph.tree();
        let nodes = stand_ins
            .within
            .iter()
            .filter(|key| stand_ins.visible(key))
            .map(|key| LevelNode {
                key: (*key).clone(),
                parent: tree
                    .parent(key)
                    .and_then(|parent| stand_ins.get(parent))
                    .cloned(),
                rolled_up: rolled_up.remove(key).unwrap_or_default(),
                hidden_prerequisites: self.hidden_prerequisites(&stand_ins, key),
                kept_work_pending: self.kept_work_pending(key),
                display_state: self.derived.display_state(self.graph, key),
                group_state: (self.node(key).kind() == NodeKind::Group)
                    .then(|| self.derived.group_state(self.graph, key)),
                roll_up: tree.is_container(key).then(|| self.roll_up(key)),
            })
            .collect();
        Ok(Level {
            container: container.cloned(),
            shown: shown.clone(),
            nodes,
            edges: self.level_edges(&stand_ins),
        })
    }

    fn stand_ins(&self, shown: &BTreeSet<NodeKind>, container: Option<&NodeKey>) -> StandIns<'a> {
        let within = self.tree_order(container);
        let mut of: BTreeMap<&NodeKey, Option<&NodeKey>> = BTreeMap::new();
        let tree = self.graph.tree();
        // Tree order puts each parent first, so its stand-in is known.
        for key in &within {
            let stand_in = if shown.contains(&self.node(key).kind()) {
                Some(*key)
            } else {
                tree.parent(key)
                    .and_then(|parent| of.get(parent).copied().flatten())
            };
            of.insert(key, stand_in);
        }
        StandIns { within, of }
    }

    /// The canvas edges (explicit, condition, opening) of the full set between stand-ins.
    fn level_edges(&self, stand_ins: &StandIns<'_>) -> Vec<LevelEdge> {
        let dependencies = self.derived.dependencies();
        let mut drawn: BTreeMap<(&NodeKey, &NodeKey), BTreeSet<UnderlyingEdge>> = BTreeMap::new();
        for edge in dependencies.edges(EdgeSet::Full) {
            let origin = match edge.source {
                EdgeSource::Explicit => EdgeOrigin::Explicit,
                EdgeSource::Condition => EdgeOrigin::Condition,
                EdgeSource::StageOpening => EdgeOrigin::StageOpening,
                EdgeSource::Work
                | EdgeSource::Entry
                | EdgeSource::Chain
                | EdgeSource::Containment => continue,
            };
            let (Some(requirement), Some(dependent)) = (
                dependencies.key(edge.requirement.node),
                dependencies.key(edge.dependent.node),
            ) else {
                continue;
            };
            let ends = (stand_ins.get(requirement), stand_ins.get(dependent));
            let (Some(from), Some(to)) = ends else {
                continue;
            };
            if from == to {
                continue;
            }
            drawn.entry((from, to)).or_default().insert(UnderlyingEdge {
                requirement: requirement.clone(),
                dependent: dependent.clone(),
                origin,
                gates: edge.class == EdgeClass::Gate,
            });
        }
        drawn
            .into_iter()
            .map(|((from, to), underlying)| LevelEdge {
                from: from.clone(),
                to: to.clone(),
                implicit: underlying
                    .iter()
                    .all(|edge| edge.origin != EdgeOrigin::Explicit),
                gates: underlying.iter().any(|edge| edge.gates),
                underlying: underlying.into_iter().collect(),
            })
            .collect()
    }

    /// C2: a blocked node's unsatisfied gate dependencies that no drawn edge stands for.
    fn hidden_prerequisites(&self, stand_ins: &StandIns<'_>, key: &NodeKey) -> Vec<NodeKey> {
        let blocking = self.derived.blocking();
        if !blocking.blocked(key) {
            return Vec::new();
        }
        let hidden: BTreeSet<NodeKey> = self
            .derived
            .dependencies()
            .of(key, EdgeSet::Pruned)
            .into_iter()
            .filter(|dependency| dependency.class == EdgeClass::Gate)
            .filter(|dependency| dependency.via != DependencyVia::Containment)
            .filter(|dependency| !blocking.satisfies(&dependency.node))
            .filter(|dependency| {
                let holder = held_by(&dependency.via).unwrap_or(key);
                match (stand_ins.get(&dependency.node), stand_ins.get(holder)) {
                    (Some(from), Some(to)) => from == to,
                    _ => true,
                }
            })
            .map(|dependency| dependency.node)
            .collect();
        hidden.into_iter().collect()
    }

    /// D1a: a skipped, in-scope container whose kept work is not yet satisfied.
    fn kept_work_pending(&self, key: &NodeKey) -> bool {
        let skipped =
            self.state(key) == State::Skipped || self.derived.skips().skipped_by(key).is_some();
        skipped && self.derived.relevance().in_scope(key) && !self.derived.blocking().satisfies(key)
    }

    /// C2: a container's badges, from its children.
    fn roll_up(&self, key: &NodeKey) -> RollUp {
        let derived = self.derived;
        let blocking = derived.blocking();
        let relevance = derived.relevance();
        let children = self.graph.tree().children(key);
        let open = |child: &&NodeKey| relevance.in_scope(child) && !blocking.closed(child);
        let relevant_open: Vec<&NodeKey> = children
            .iter()
            .filter(|child| relevance.value(child) == Relevance::Relevant)
            .filter(|child| !blocking.closed(child))
            .collect();
        RollUp {
            ready_to_finish: self.node(key).kind() != NodeKind::Group
                && relevance.in_scope(key)
                && blocking.deps_done(key)
                && !blocking.closed(key),
            children_active: children
                .iter()
                .any(|child| self.state(child) == State::Active),
            all_blocked: !relevant_open.is_empty()
                && relevant_open.iter().all(|child| blocking.blocked(child)),
            decision_needed: children.iter().any(|child| {
                self.node(child).kind() == NodeKind::Decision && blocking.actionable(child)
            }),
            needs_breakdown: children.iter().any(|child| blocking.needs_breakdown(child)),
            max_child_gravity: derived.priority().max_child_gravity(key),
            peak_gravity: derived
                .priority()
                .peak_gravity(key)
                .map(|(node, gravity)| PeakGravity {
                    node: node.clone(),
                    gravity,
                }),
            min_child_slack_days: children
                .iter()
                .filter(open)
                .filter_map(|child| derived.dates().slack_days(child))
                .min(),
            owners: children
                .iter()
                .flat_map(|child| self.owners(child))
                .cloned()
                .collect(),
        }
    }
}
