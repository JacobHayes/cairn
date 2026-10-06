//! The network itself: instant slots and the constraints between them, built from 2.2's
//! pruned effective dependency graph and the graph's date rules, estimates, and stage closes,
//! as the translation table in [`super`] lays out. Pins, answers, actuals, and today are not
//! constraints; the layers seed them onto slots.

use cairn_schema::limits::NODE_COUNT_MAX;
use cairn_schema::{
    AnswerSpec, DateRule, DateSource, Direction, KeyRefs, Node, NodeKey, NodeKind, Payload,
    Relevance, State,
};

use crate::derive::dependencies::{
    Dependencies, Edge, EdgeSet, EdgeSource, Instant, NodeIndex, Point,
};
use crate::derive::relevance::Relevances;
use crate::derive::skip::Skips;
use crate::derive::stored_state;
use crate::graph::Document;

/// Slots per node: 2.2's four instants.
const POINTS: usize = 4;

/// One instant of the network, by position: node `i`'s instants at `4i..4i + 4` (2.2's
/// [`Instant::slot`]), `created_at` after them, then each node's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Slot(u32);

impl Slot {
    /// The position, for per-slot tables.
    #[must_use]
    pub(crate) fn index(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }

    /// The slot at a position.
    #[must_use]
    pub(crate) fn at(index: usize) -> Self {
        Self(u32::try_from(index).unwrap_or(u32::MAX))
    }
}

/// What a slot stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// A node's instant.
    Node(Instant),
    /// The journey's `created_at`.
    CreatedAt,
    /// A date decision's answer.
    Answer(NodeIndex),
}

/// Which date rule (A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Rule {
    /// `due_by`: caps the node's finish.
    DueBy,
    /// `not_before`: holds the node's start back.
    NotBefore,
}

/// Why a constraint exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// An edge of the effective dependency graph; a `Work` edge carries the node's estimate.
    Dependency(Edge),
    /// A date rule on the node, one constraint per source.
    Rule {
        /// The node the rule is on.
        node: NodeIndex,
        /// Which rule.
        rule: Rule,
    },
    /// A stage's closing milestone bounding the group's finish (F4).
    StageClose {
        /// The stage.
        group: NodeIndex,
    },
}

/// One constraint: `after >= before + offset_days`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Constraint {
    /// The earlier slot.
    pub before: Slot,
    /// The later slot.
    pub after: Slot,
    /// The least number of days between them; may be zero or negative.
    pub offset_days: i32,
    /// Where it comes from.
    pub origin: Origin,
    /// It passes through an undecided node (F2).
    pub conditional: bool,
}

/// Which layer a network is built for (F5, F6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layer {
    /// Pins, rules, estimates, containment, stage bounds, dependencies.
    Plan,
    /// The plan with actuals and today: finished work took the time its actual dates say,
    /// so its estimate no longer holds its finish after its start.
    Execution,
}

/// Per node, what the network needs from the earlier passes: relevance, whether it is
/// skipped (itself, or effectively), and whether its own work counts no days.
pub(crate) struct Scope {
    relevance: Vec<Relevance>,
    skipped: Vec<bool>,
    zero: Vec<bool>,
}

impl Scope {
    /// Reads relevance and skips for every node of the dependency graph. Skipped work has
    /// zero duration (F2); in the execution layer so has done work.
    #[must_use]
    pub(crate) fn new(
        document: &Document,
        dependencies: &Dependencies,
        relevance: &Relevances,
        skips: &Skips,
        layer: Layer,
    ) -> Self {
        let keys = (0..dependencies.node_count()).filter_map(|at| dependencies.key(index(at)));
        let mut scope = Scope {
            relevance: Vec::new(),
            skipped: Vec::new(),
            zero: Vec::new(),
        };
        for key in keys {
            scope.relevance.push(relevance.value(key));
            let state = document
                .nodes
                .get(key)
                .map(|node| stored_state(document, node));
            let skipped = state == Some(State::Skipped) || skips.skipped_by(key).is_some();
            let done = layer == Layer::Execution && state == Some(State::Done);
            scope.skipped.push(skipped);
            scope.zero.push(skipped || done);
        }
        assert_eq!(scope.relevance.len(), dependencies.node_count());
        scope
    }

    /// The node's relevance.
    #[must_use]
    pub(crate) fn relevance(&self, node: NodeIndex) -> Relevance {
        self.relevance
            .get(node.get())
            .copied()
            .unwrap_or(Relevance::NotRelevant)
    }

    /// Relevant or undecided.
    #[must_use]
    pub(crate) fn in_scope(&self, node: NodeIndex) -> bool {
        self.relevance(node) != Relevance::NotRelevant
    }

    /// Skipped itself or effectively (D1a): zero duration, no seeds.
    #[must_use]
    pub(crate) fn skipped(&self, node: NodeIndex) -> bool {
        self.skipped.get(node.get()).copied().unwrap_or(false)
    }

    /// Its own work counts no days.
    #[must_use]
    pub(crate) fn zero(&self, node: NodeIndex) -> bool {
        self.zero.get(node.get()).copied().unwrap_or(false)
    }
}

fn index(at: usize) -> NodeIndex {
    NodeIndex::from_position(at)
}

/// The network: slots, constraints, and each slot's constraints both ways.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Network {
    node_count: usize,
    /// Per node: a decision or milestone, whose start is its finish.
    single: Vec<bool>,
    constraints: Vec<Constraint>,
    /// Constraint positions by `before` slot, with per-slot offsets.
    outgoing: Vec<u32>,
    outgoing_at: Vec<usize>,
    /// Constraint positions by `after` slot, with per-slot offsets.
    incoming: Vec<u32>,
    incoming_at: Vec<usize>,
}

impl Network {
    /// Builds the network of a graph from its pruned dependency graph and its scope.
    #[must_use]
    pub(crate) fn build(document: &Document, dependencies: &Dependencies, scope: &Scope) -> Self {
        let node_count = dependencies.node_count();
        assert!(node_count <= NODE_COUNT_MAX as usize);
        let single = (0..node_count)
            .map(|at| {
                let node = dependencies
                    .key(index(at))
                    .and_then(|k| document.nodes.get(k));
                node.is_some_and(|node| node.kind().is_leaf())
            })
            .collect();
        let mut network = Network {
            node_count,
            single,
            ..Network::default()
        };
        let mut constraints = Vec::new();
        for edge in dependencies.edges(EdgeSet::Pruned) {
            network.push_edge(document, dependencies, scope, *edge, &mut constraints);
        }
        for node in document.nodes.values() {
            let Some(at) = dependencies.node_index(&node.key) else {
                continue;
            };
            if !scope.in_scope(at) {
                continue;
            }
            network.push_rules(document, dependencies, scope, node, at, &mut constraints);
            network.push_close(document, dependencies, scope, node, at, &mut constraints);
        }
        network.settle(constraints);
        network
    }

    /// A dependency edge as a constraint: the requirement's instant before the dependent's,
    /// by the node's estimate for its own work edge and by nothing otherwise.
    fn push_edge(
        &self,
        document: &Document,
        dependencies: &Dependencies,
        scope: &Scope,
        edge: Edge,
        out: &mut Vec<Constraint>,
    ) {
        let offset_days = match edge.source {
            EdgeSource::Work => {
                let node = dependencies
                    .key(edge.dependent.node)
                    .and_then(|key| document.nodes.get(key));
                let zero = scope.zero(edge.dependent.node);
                node.filter(|_| !zero).map_or(0, estimate_days)
            }
            EdgeSource::Entry
            | EdgeSource::Chain
            | EdgeSource::Containment
            | EdgeSource::Explicit
            | EdgeSource::Condition
            | EdgeSource::StageOpening => 0,
        };
        let conditional =
            undecided(scope, edge.dependent.node) || undecided(scope, edge.requirement.node);
        self.push(
            out,
            Constraint {
                before: self.slot(edge.requirement),
                after: self.slot(edge.dependent),
                offset_days,
                origin: Origin::Dependency(edge),
                conditional,
            },
        );
    }

    /// A8: each source of the node's `due_by` caps its finish, and each source of its
    /// `not_before` holds its start back, by the signed offset.
    fn push_rules(
        &self,
        document: &Document,
        dependencies: &Dependencies,
        scope: &Scope,
        node: &Node<KeyRefs>,
        at: NodeIndex,
        out: &mut Vec<Constraint>,
    ) {
        let rules = [
            (Rule::DueBy, node.due_by.as_ref()),
            (Rule::NotBefore, node.not_before.as_ref()),
        ];
        for (rule, found) in rules {
            let Some(found) = found else {
                continue;
            };
            let signed = signed_offset_days(found);
            for source in found.sources.as_set() {
                let Some((source, from)) = self.source_slot(document, dependencies, source) else {
                    continue;
                };
                if from.is_some_and(|from| !scope.in_scope(from)) {
                    continue;
                }
                let (before, after, offset_days) = match rule {
                    Rule::DueBy => (self.finish(at), source, -signed),
                    Rule::NotBefore => (source, self.start(at), signed),
                };
                let conditional = undecided(scope, at) || from.is_some_and(|f| undecided(scope, f));
                let origin = Origin::Rule { node: at, rule };
                self.push(
                    out,
                    Constraint {
                        before,
                        after,
                        offset_days,
                        origin,
                        conditional,
                    },
                );
            }
        }
    }

    /// F4: a stage's closing milestone, outside it and in scope, bounds the group's finish
    /// unless the group declares `closes: false`.
    fn push_close(
        &self,
        document: &Document,
        dependencies: &Dependencies,
        scope: &Scope,
        node: &Node<KeyRefs>,
        at: NodeIndex,
        out: &mut Vec<Constraint>,
    ) {
        let Payload::Group(group) = &node.payload else {
            return;
        };
        let Some(closes_at) = group.closes_at.as_ref().filter(|_| group.closes) else {
            return;
        };
        let milestone = document
            .nodes
            .get(closes_at)
            .is_some_and(|found| found.kind() == NodeKind::Milestone);
        let Some(closing) = dependencies.node_index(closes_at) else {
            return;
        };
        let inside = is_ancestor(document, &node.key, closes_at);
        if !milestone || inside || !scope.in_scope(closing) {
            return;
        }
        let conditional = undecided(scope, at) || undecided(scope, closing);
        self.push(
            out,
            Constraint {
                before: self.finish(at),
                after: self.finish(closing),
                offset_days: 0,
                origin: Origin::StageClose { group: at },
                conditional,
            },
        );
    }

    /// Keeps a constraint unless it is a loop on one slot that asks for nothing.
    fn push(&self, out: &mut Vec<Constraint>, constraint: Constraint) {
        assert!(constraint.before.index() < self.slot_count());
        assert!(constraint.after.index() < self.slot_count());
        if constraint.before == constraint.after && constraint.offset_days <= 0 {
            return;
        }
        out.push(constraint);
    }

    /// The slot a rule source names, with the node it belongs to: a milestone's instant, a
    /// date decision's answer, or `created_at`. None for a reference validation reports.
    fn source_slot(
        &self,
        document: &Document,
        dependencies: &Dependencies,
        source: &DateSource<KeyRefs>,
    ) -> Option<(Slot, Option<NodeIndex>)> {
        let key = match source {
            DateSource::CreatedAt => return Some((self.created_at(), None)),
            DateSource::Node(key) => key,
        };
        let at = dependencies.node_index(key)?;
        match &document.nodes.get(key)?.payload {
            Payload::Milestone(_) => Some((self.finish(at), Some(at))),
            Payload::Decision(decision) if matches!(decision.answer, AnswerSpec::Date { .. }) => {
                Some((self.answer(at), Some(at)))
            }
            Payload::Decision(_)
            | Payload::Deliverable(_)
            | Payload::Action(_)
            | Payload::Group(_) => None,
        }
    }

    /// Sorts the constraints into per-slot slices both ways.
    fn settle(&mut self, constraints: Vec<Constraint>) {
        let slots = self.slot_count();
        let positions = |key: fn(&Constraint) -> Slot| {
            let mut order: Vec<u32> = (0..constraints.len())
                .map(|at| u32::try_from(at).unwrap_or(u32::MAX))
                .collect();
            order.sort_by_key(|&at| constraints.get(at as usize).map(key));
            let mut counts = vec![0_usize; slots + 1];
            for constraint in &constraints {
                if let Some(count) = counts.get_mut(key(constraint).index() + 1) {
                    *count += 1;
                }
            }
            for at in 1..counts.len() {
                let before = counts.get(at - 1).copied().unwrap_or(0);
                if let Some(count) = counts.get_mut(at) {
                    *count += before;
                }
            }
            (order, counts)
        };
        (self.outgoing, self.outgoing_at) = positions(|constraint| constraint.before);
        (self.incoming, self.incoming_at) = positions(|constraint| constraint.after);
        self.constraints = constraints;
        assert_eq!(self.outgoing.len(), self.constraints.len());
        assert_eq!(self.incoming_at.len(), slots + 1);
    }
}

impl Network {
    /// How many slots there are: four per node, `created_at`, and one answer per node.
    #[must_use]
    pub(crate) fn slot_count(&self) -> usize {
        self.node_count * (POINTS + 1) + 1
    }

    /// How many nodes the network covers.
    #[must_use]
    pub(crate) fn node_count(&self) -> usize {
        self.node_count
    }

    /// The constraints, in build order.
    #[must_use]
    pub(crate) fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// The constraint at a position.
    #[must_use]
    pub(crate) fn constraint(&self, at: u32) -> Option<&Constraint> {
        self.constraints.get(at as usize)
    }

    /// The positions of the constraints whose `before` is the slot.
    #[must_use]
    pub(crate) fn outgoing(&self, slot: Slot) -> &[u32] {
        slice(&self.outgoing, &self.outgoing_at, slot)
    }

    /// The positions of the constraints whose `after` is the slot.
    #[must_use]
    pub(crate) fn incoming(&self, slot: Slot) -> &[u32] {
        slice(&self.incoming, &self.incoming_at, slot)
    }

    /// The slot of a node's instant: a decision's or milestone's start is its finish.
    #[must_use]
    pub(crate) fn slot(&self, instant: Instant) -> Slot {
        let single = self
            .single
            .get(instant.node.get())
            .copied()
            .unwrap_or(false);
        let point = match instant.point {
            Point::Start if single => Point::Finish,
            point => point,
        };
        Slot::at(Instant::new(instant.node, point).slot())
    }

    /// The node's start slot (its one instant for a decision or milestone).
    #[must_use]
    pub(crate) fn start(&self, node: NodeIndex) -> Slot {
        self.slot(Instant::new(node, Point::Start))
    }

    /// The node's finish slot.
    #[must_use]
    pub(crate) fn finish(&self, node: NodeIndex) -> Slot {
        self.slot(Instant::new(node, Point::Finish))
    }

    /// The `created_at` slot.
    #[must_use]
    pub(crate) fn created_at(&self) -> Slot {
        Slot::at(self.node_count * POINTS)
    }

    /// A date decision's answer slot.
    #[must_use]
    pub(crate) fn answer(&self, decision: NodeIndex) -> Slot {
        Slot::at(self.node_count * POINTS + 1 + decision.get())
    }

    /// What a slot stands for.
    #[must_use]
    pub(crate) fn place(&self, slot: Slot) -> Place {
        let at = slot.index();
        let nodes = self.node_count * POINTS;
        match at.cmp(&nodes) {
            std::cmp::Ordering::Less => {
                let point = match at % POINTS {
                    0 => Point::Start,
                    1 => Point::Finish,
                    2 => Point::Entry,
                    _ => Point::ConditionEntry,
                };
                Place::Node(Instant::new(index(at / POINTS), point))
            }
            std::cmp::Ordering::Equal => Place::CreatedAt,
            std::cmp::Ordering::Greater => Place::Answer(index(at - nodes - 1)),
        }
    }

    /// True for a slot shown to people: a node's start or finish, `created_at`, or an
    /// answer; false for a container's entries, which chains collapse.
    #[must_use]
    pub(crate) fn shown(&self, slot: Slot) -> bool {
        match self.place(slot) {
            Place::Node(instant) => matches!(instant.point, Point::Start | Point::Finish),
            Place::CreatedAt | Place::Answer(_) => true,
        }
    }
}

fn slice<'a>(positions: &'a [u32], offsets: &[usize], slot: Slot) -> &'a [u32] {
    let from = offsets.get(slot.index()).copied().unwrap_or(0);
    let to = offsets.get(slot.index() + 1).copied().unwrap_or(from);
    positions.get(from..to).unwrap_or(&[])
}

fn undecided(scope: &Scope, node: NodeIndex) -> bool {
    scope.relevance(node) == Relevance::Undecided
}

/// A9: a deliverable's or action's estimate in days; zero for any other kind or without one.
#[must_use]
pub(crate) fn estimate_days(node: &Node<KeyRefs>) -> i32 {
    let days = match &node.payload {
        Payload::Deliverable(deliverable) => deliverable.estimate,
        Payload::Action(action) => action.estimate,
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => None,
    };
    days.map_or(0, |days| i32::try_from(days.get()).unwrap_or(i32::MAX))
}

/// A8: a rule's offset with its direction's sign: positive after its sources, negative
/// before them.
#[must_use]
pub(crate) fn signed_offset_days(rule: &DateRule<KeyRefs>) -> i32 {
    let days = i32::try_from(rule.offset.get()).unwrap_or(i32::MAX);
    match rule.direction {
        Direction::After => days,
        Direction::Before => -days,
    }
}

/// True when `ancestor` is a proper ancestor of `key`, by parent links (bounded by depth).
fn is_ancestor(document: &Document, ancestor: &NodeKey, key: &NodeKey) -> bool {
    let mut current = document
        .nodes
        .get(key)
        .and_then(|node| node.parent.as_ref());
    let mut steps = 0_u32;
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        steps += 1;
        if steps > cairn_schema::limits::CONTAINMENT_DEPTH_MAX {
            return false;
        }
        current = document
            .nodes
            .get(parent)
            .and_then(|node| node.parent.as_ref());
    }
    false
}

#[cfg(test)]
mod tests {
    //! The translation table, row by row: each graph's constraints, written as
    //! `before -> after +offset origin`.
    use std::collections::BTreeSet;

    use cairn_schema::{SequentialKeys, from_yaml};

    use super::{Network, Origin, Place, Rule, Scope, Slot};
    use crate::derive::dependencies::{EdgeSource, Point};
    use crate::graph::Graph;
    use crate::testing::derive_inputs;

    fn route(nodes: &str) -> Graph {
        let yaml = format!("format: 1\nroute: test\nname: Test\nnodes:\n{nodes}");
        let file = from_yaml(&yaml).unwrap_or_else(|error| panic!("{error}\n{yaml}"));
        crate::from_file(&file, &mut SequentialKeys::default()).unwrap()
    }

    fn name(graph: &Graph, network: &Network, slot: Slot) -> String {
        let derived = crate::derive(
            graph,
            None,
            &derive_inputs(cairn_schema::Deployment::default()),
        );
        let key = |node| derived.dependencies().key(node).unwrap().to_string();
        match network.place(slot) {
            Place::Node(instant) => {
                let point = match instant.point {
                    Point::Start => "start",
                    Point::Finish => "finish",
                    Point::Entry => "entry",
                    Point::ConditionEntry => "condition_entry",
                };
                format!("{}.{point}", key(instant.node))
            }
            Place::CreatedAt => "created_at".to_owned(),
            Place::Answer(node) => format!("{}.answer", key(node)),
        }
    }

    fn constraints(graph: &Graph) -> BTreeSet<String> {
        let derived = crate::derive(
            graph,
            None,
            &derive_inputs(cairn_schema::Deployment::default()),
        );
        let document = graph.document();
        let dependencies = derived.dependencies();
        let scope = Scope::new(
            document,
            dependencies,
            derived.relevance(),
            derived.skips(),
            super::Layer::Plan,
        );
        let network = Network::build(document, dependencies, &scope);
        network
            .constraints()
            .iter()
            .map(|constraint| {
                let origin = match constraint.origin {
                    Origin::Dependency(edge) => match edge.source {
                        EdgeSource::Work => "work",
                        EdgeSource::Entry => "entry",
                        EdgeSource::Chain => "chain",
                        EdgeSource::Containment => "containment",
                        EdgeSource::Explicit => "explicit",
                        EdgeSource::Condition => "condition",
                        EdgeSource::StageOpening => "opening",
                    },
                    Origin::Rule {
                        rule: Rule::DueBy, ..
                    } => "due_by",
                    Origin::Rule {
                        rule: Rule::NotBefore,
                        ..
                    } => "not_before",
                    Origin::StageClose { .. } => "close",
                };
                let conditional = if constraint.conditional {
                    " conditional"
                } else {
                    ""
                };
                format!(
                    "{} -> {} {:+} {origin}{conditional}",
                    name(graph, &network, constraint.before),
                    name(graph, &network, constraint.after),
                    constraint.offset_days,
                )
            })
            .collect()
    }

    const MILESTONE: &str = "- {key: n_m, id: m, kind: milestone, title: M}\n";

    /// One row: a graph, constraints it must have, and constraints it must not.
    fn row(row: &str, nodes: &str, present: &[&str], absent: &[&str]) {
        let found = constraints(&route(nodes));
        for expected in present {
            assert!(
                found.contains(*expected),
                "{row}: missing {expected} in {found:#?}"
            );
        }
        for unexpected in absent {
            assert!(!found.contains(*unexpected), "{row}: has {unexpected}");
        }
    }

    #[test]
    fn dependencies_hold_the_dependent_after_the_requirement() {
        row(
            "a requirement on a leaf holds its start after the requirement's finish",
            "- {key: n_a, id: a, kind: action, title: A, requires: [b]}\n- {key: n_b, id: b, kind: action, title: B}\n",
            &["n_b.finish -> n_a.start +0 explicit"],
            &[],
        );
        row(
            "a requirement on a container waits at its entry, which its children and its own start wait on",
            "- {key: n_g, id: g, kind: group, title: G, requires: [b]}\n- {key: n_c, id: c, parent: g, kind: action, title: C}\n- {key: n_b, id: b, kind: action, title: B}\n",
            &[
                "n_b.finish -> n_g.entry +0 explicit",
                "n_g.entry -> n_c.start +0 chain",
                "n_g.condition_entry -> n_c.start +0 chain",
                "n_g.entry -> n_g.start +0 entry",
                "n_c.finish -> n_g.start +0 containment",
                "n_g.start -> n_g.finish +0 work",
            ],
            &["n_b.finish -> n_c.start +0 explicit"],
        );
        row(
            "a condition gate holds the node after the decision, its one instant",
            "- {key: n_d, id: d, kind: decision, title: D, prompt: P, answer_type: boolean}\n- {key: n_a, id: a, kind: action, title: A, relevant_when: {equals: {decision: d, value: true}}}\n",
            &["n_d.finish -> n_a.start +0 condition conditional"],
            &[],
        );
        row(
            "a stage opening holds the stage after its milestone, gating or not",
            &format!(
                "{MILESTONE}- {{key: n_g, id: g, kind: group, title: G, opens_at: m, gates: false}}\n"
            ),
            &["n_m.finish -> n_g.start +0 opening"],
            &[],
        );
    }

    #[test]
    fn work_takes_its_estimate_and_a_decision_or_milestone_is_one_instant() {
        row(
            "an estimate separates a node's start from its finish",
            "- {key: n_a, id: a, kind: deliverable, title: A, estimate: 3}\n- {key: n_b, id: b, kind: action, title: B}\n",
            &[
                "n_a.start -> n_a.finish +3 work",
                "n_b.start -> n_b.finish +0 work",
            ],
            &[],
        );
        row(
            "a decision or milestone is one instant, with no work of its own",
            &format!(
                "{MILESTONE}- {{key: n_d, id: d, kind: decision, title: D, prompt: P, answer_type: text, requires: [m]}}\n"
            ),
            &["n_m.finish -> n_d.finish +0 explicit"],
            &[
                "n_d.finish -> n_d.finish +0 work",
                "n_m.start -> n_m.finish +0 work",
            ],
        );
    }

    #[test]
    fn rules_cap_finishes_and_hold_starts_back_by_their_signed_offsets() {
        row(
            "due_by before a milestone caps the finish that many days ahead of it",
            &format!(
                "{MILESTONE}- {{key: n_a, id: a, kind: action, title: A, due_by: {{before: m, offset: 14}}}}\n"
            ),
            &["n_a.finish -> n_m.finish +14 due_by"],
            &[],
        );
        row(
            "due_by after a source caps the finish that many days past it",
            "- {key: n_a, id: a, kind: action, title: A, due_by: {after: journey.created_at, offset: 3}}\n",
            &["n_a.finish -> created_at -3 due_by"],
            &[],
        );
        row(
            "not_before after a milestone holds the start back that many days",
            &format!(
                "{MILESTONE}- {{key: n_a, id: a, kind: action, title: A, not_before: {{after: m, offset: 2}}}}\n"
            ),
            &["n_m.finish -> n_a.start +2 not_before"],
            &[],
        );
        row(
            "not_before before a date answer and a milestone is one constraint per source; the offset defaults to zero",
            &format!(
                "{MILESTONE}- {{key: n_d, id: d, kind: decision, title: D, prompt: P, answer_type: date}}\n- {{key: n_a, id: a, kind: action, title: A, not_before: {{before: [d, m], offset: 2}}}}\n- {{key: n_b, id: b, kind: action, title: B, not_before: {{after: d}}}}\n"
            ),
            &[
                "n_d.answer -> n_a.start -2 not_before",
                "n_m.finish -> n_a.start -2 not_before",
                "n_d.answer -> n_b.start +0 not_before",
            ],
            &[],
        );
    }

    #[test]
    fn a_stage_close_caps_the_group_unless_it_does_not_close() {
        row(
            "a stage close caps the group's finish at its milestone",
            &format!(
                "{MILESTONE}- {{key: n_g, id: g, kind: group, title: G, closes_at: m}}\n- {{key: n_c, id: c, parent: g, kind: action, title: C}}\n"
            ),
            &[
                "n_g.finish -> n_m.finish +0 close",
                "n_c.finish -> n_g.start +0 containment",
            ],
            &[],
        );
        row(
            "closes: false drops the close",
            &format!(
                "{MILESTONE}- {{key: n_g, id: g, kind: group, title: G, closes_at: m, closes: false}}\n"
            ),
            &[],
            &["n_g.finish -> n_m.finish +0 close"],
        );
    }
}
