//! C2, semantic zoom: one aggregation level of the canvas. The rules are fixed here, never
//! authored:
//!
//! - A node is visible when its kind is shown, its relevance class is shown, no ancestor
//!   within the level is collapsed, and it is within the drilled-in container (a proper
//!   descendant; every node when none is drilled into). The relevance classes are
//!   `relevant`, `conditional` (undecided, or not relevant only because the decision it
//!   reads is undecided itself, [`crate::derive::pending`]), and `not_relevant` (settled).
//! - Each node within the container has a stand-in: itself when visible, else its nearest
//!   visible ancestor within the container, else none. A visible node is drawn under the
//!   stand-in of its parent, or at the top level; a hidden one rolls up into its stand-in. A
//!   collapsed container rolls its whole subtree into its own card (or its stand-in's).
//! - Edges are the explicit requirements, condition gates, and stage openings of the full
//!   structural graph. Each is drawn between its ends' stand-ins; it is left off when either
//!   has none (still traceable, C7) and when both land on the same node; edges landing on the
//!   same pair collapse into one. A node hidden for its relevance class leaves no orphan
//!   line: its edges re-target like any hidden node's.
//! - A blocked visible node shows the hidden-prerequisites marker for each unsatisfied gate
//!   dependency (its own, inherited, condition, or opening; not its children, which roll up
//!   into it) that no drawn edge stands for: the edge from the dependency's stand-in to the
//!   stand-in of the node holding it (the node itself, or the ancestor whose requirement,
//!   condition, or opening it is) is not drawn. A node that rolled up into a visible one
//!   counts toward it when its own unsatisfied dependency has no stand-in at all (so no edge
//!   could be drawn for it). So hiding a kind, a class, or a stage never makes blocked work
//!   look free.
//! - Each visible container carries its roll-ups (C2's badges), each node its display state
//!   (D8), and each group its legacy group state (D1).
//!
//! Cost at `node_count_max` (2,000 nodes, about 110,000 edges): the relevance classes, only
//! when a class is hidden, are two relevance passes ([`crate::derive::pending`], about 34,000
//! steps each); stand-ins in one pass in tree order, O(nodes); the edges in one pass with a
//! sorted map, O(edges log edges); the marker walks a blocked node's entry chains (at most
//! about 1,300 edges each, 2.6 million in all, each node once whether it is visible or rolled
//! up); a group's state reads its descendants (2,000 x 2,000 at worst) and a container's
//! roll-up its children, O(nodes) in all.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    DependencyVia, Deployment, EdgeOrigin, Level, LevelDisplay, LevelEdge, LevelNode, LevelQuery,
    NodeKey, NodeKind, Relevance, RollUp, State, UnderlyingEdge,
};

use super::{DerivedJourney, ProjectionError};
use crate::derive::Relevances;
use crate::derive::dependencies::{EdgeClass, EdgeSet, EdgeSource};
use crate::derive::held_by;
use crate::derive::pending::{RelevanceClass, classify};

/// Each node within the level and its stand-in: itself when visible, else its nearest visible
/// ancestor within the level.
struct StandIns<'a> {
    within: Vec<&'a NodeKey>,
    of: BTreeMap<&'a NodeKey, Option<&'a NodeKey>>,
    /// The collapsed containers within the level.
    collapsed: BTreeSet<&'a NodeKey>,
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
    /// C2: the level the query asks for: the visible nodes, each under its nearest visible
    /// ancestor, with what rolls up into it, its hidden-prerequisites marker, and for a
    /// container its roll-ups; the edges re-targeted to visible stand-ins, duplicates
    /// collapsed and self-edges dropped. `deployment` is the one the journey was derived
    /// over; it is read only when a relevance class is left out.
    ///
    /// # Errors
    ///
    /// When the container, or a collapsed container, is not a node of the journey.
    pub fn level(
        &self,
        query: &LevelQuery,
        deployment: &Deployment,
    ) -> Result<Level, ProjectionError> {
        let classes = (query.display.len() < LevelDisplay::ALL.len())
            .then(|| classify(self.graph, deployment));
        self.level_with(query, classes.as_ref())
    }

    /// The level for the query, given the relevance classes when it leaves one out.
    pub(super) fn level_with(
        &self,
        query: &LevelQuery,
        classes: Option<&Relevances>,
    ) -> Result<Level, ProjectionError> {
        if let Some(key) = &query.container {
            self.known(key)?;
        }
        for key in &query.collapsed {
            self.known(key)?;
        }
        assert!(
            classes.is_some() || query.display.len() == LevelDisplay::ALL.len(),
            "a level that leaves a class out has the classes"
        );
        let stand_ins = self.stand_ins(query, classes);
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
            .map(|key| {
                let members = rolled_up.remove(key).unwrap_or_default();
                LevelNode {
                    key: (*key).clone(),
                    parent: tree
                        .parent(key)
                        .and_then(|parent| stand_ins.get(parent))
                        .cloned(),
                    hidden_prerequisites: self.hidden_prerequisites(&stand_ins, key, &members),
                    rolled_up: members,
                    kept_work_pending: self.kept_work_pending(key),
                    display_state: self.derived.display_state(self.graph, key),
                    group_state: (self.node(key).kind() == NodeKind::Group)
                        .then(|| self.derived.group_state(self.graph, key)),
                    roll_up: tree.is_container(key).then(|| self.roll_up(key)),
                }
            })
            .collect();
        Ok(Level {
            container: query.container.clone(),
            shown: query.shown.clone(),
            collapsed: stand_ins
                .collapsed
                .iter()
                .map(|key| (*key).clone())
                .collect(),
            display: query.display.clone(),
            nodes,
            edges: self.level_edges(&stand_ins),
        })
    }

    /// Each node within the level and its stand-in, in one pass in tree order.
    fn stand_ins(&self, query: &LevelQuery, classes: Option<&Relevances>) -> StandIns<'a> {
        let within = self.tree_order(query.container.as_ref());
        let collapsed: BTreeSet<&NodeKey> = within
            .iter()
            .copied()
            .filter(|key| query.collapsed.contains(*key))
            .collect();
        let mut of: BTreeMap<&NodeKey, Option<&NodeKey>> = BTreeMap::new();
        let mut inside: BTreeSet<&NodeKey> = BTreeSet::new();
        let tree = self.graph.tree();
        // Tree order puts each parent first, so its stand-in and collapse are known.
        for key in &within {
            let under_collapse = tree
                .parent(key)
                .is_some_and(|parent| inside.contains(parent) || collapsed.contains(parent));
            if under_collapse {
                inside.insert(key);
            }
            let shown_class = classes.is_none_or(|classes| {
                query.display.contains(&match classes.class(key) {
                    RelevanceClass::Relevant => LevelDisplay::Relevant,
                    RelevanceClass::Undecided | RelevanceClass::Pending => {
                        LevelDisplay::Conditional
                    }
                    RelevanceClass::Settled => LevelDisplay::NotRelevant,
                })
            });
            let stand_in =
                if !under_collapse && shown_class && query.shown.contains(&self.node(key).kind()) {
                    Some(*key)
                } else {
                    tree.parent(key)
                        .and_then(|parent| of.get(parent).copied().flatten())
                };
            of.insert(key, stand_in);
        }
        StandIns {
            within,
            of,
            collapsed,
        }
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

    /// C2: a blocked node's unsatisfied gate dependencies that no drawn edge stands for, and
    /// those of the nodes rolled up into it (`members`) that have no stand-in at all, so no
    /// edge could stand for them. A dependency inside the card (both ends stand in it) is
    /// the card's own business, not a hidden prerequisite, for a member.
    fn hidden_prerequisites(
        &self,
        stand_ins: &StandIns<'_>,
        key: &NodeKey,
        members: &[NodeKey],
    ) -> Vec<NodeKey> {
        let mut hidden: BTreeSet<NodeKey> = BTreeSet::new();
        for (blocked, own) in std::iter::once((key, true)).chain(members.iter().map(|m| (m, false)))
        {
            hidden.extend(self.unsatisfied_without_edge(stand_ins, blocked, own));
        }
        hidden.into_iter().collect()
    }

    /// The unsatisfied gate dependencies of `key`, if it is blocked, that no drawn edge
    /// stands for: those whose stand-in or whose holder's has none, and for a card's own
    /// node (`own`) those landing on the card itself.
    fn unsatisfied_without_edge<'k>(
        &'k self,
        stand_ins: &'k StandIns<'_>,
        key: &'k NodeKey,
        own: bool,
    ) -> impl Iterator<Item = NodeKey> + 'k {
        let blocking = self.derived.blocking();
        let dependencies = if blocking.blocked(key) {
            self.derived.dependencies().of(key, EdgeSet::Pruned)
        } else {
            Vec::new()
        };
        dependencies
            .into_iter()
            .filter(|dependency| dependency.class == EdgeClass::Gate)
            .filter(|dependency| dependency.via != DependencyVia::Containment)
            .filter(move |dependency| !blocking.satisfies(&dependency.node))
            .filter(move |dependency| {
                let holder = held_by(&dependency.via).unwrap_or(key);
                match (stand_ins.get(&dependency.node), stand_ins.get(holder)) {
                    (Some(from), Some(to)) => own && from == to,
                    _ => true,
                }
            })
            .map(|dependency| dependency.node)
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
            subtree_gravity: derived.priority().subtree_gravity(key),
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
