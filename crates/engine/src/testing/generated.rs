//! Generated journeys built directly as documents (not through apply), for the derive passes'
//! property and reference tests: deep containment, explicit edges, conditions, stage
//! openings, stored states, answers, force includes, and keeps, all generic.
//!
//! Every generated graph is valid by construction. Nodes are created in depth-first
//! preorder, so each subtree is a contiguous run of indices; a requirement, a condition's
//! decision, or a gating stage opening is always a node whose whole subtree ends before the
//! dependent begins. Laying out each container's entries at its preorder index and each
//! node's start and finish at its subtree's end, deeper nodes first, puts every gate edge
//! forward in time, so the gate graph is acyclic. A stage opening with `gates: false` may
//! name any milestone outside its group, so date-only cycles occur too.
// Test support: an index out of range is a bug in this generator, and the panic shows it.
#![allow(clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use proptest::prelude::*;

use cairn_schema::limits::{CONTAINMENT_DEPTH_MAX, EDGE_COUNT_PER_NODE_MAX};
use cairn_schema::{
    Action, AnswerSpec, AnswerValue, BoundedSet, Clause, Comparison, Condition, ConditionValue,
    Decision, Deliverable, Group, KeyRefs, KindKey, Milestone, Node, NodeKey, NodeKind, NodeState,
    Overrides, ParticipationKind, ParticipationSource, Participations, Payload, Provenance, Role,
    State,
};

use super::parse;
use crate::graph::{Document, Graph};

/// One generated node, its references resolved by index against the nodes before it.
#[derive(Clone, Debug, Default)]
pub struct NodeSeed {
    /// How many levels up the open path the node's parent is.
    pub climb: u8,
    /// The kind, by index into a mix weighted toward containers.
    pub kind: u8,
    /// Explicit requirements, by index among the nodes that end before this one.
    pub requires: Vec<u16>,
    /// A condition: its shape and two decisions, by index among those before this node.
    pub condition: Option<(u8, u16, u16)>,
    /// A group's opening milestone, by index, and whether it gates.
    pub opening: Option<(u16, bool)>,
    /// The stored state, by index among the kind's states.
    pub state: u8,
    /// Force-included.
    pub force: bool,
    /// Kept under a skipped ancestor.
    pub keep: bool,
    /// A participation: of `owner` (true) or the multi-valued kind, and its source by index
    /// (the kind's role, or one of a few explicit lists, the empty one included).
    pub participation: Option<(bool, u8)>,
}

fn arb_seed() -> impl Strategy<Value = NodeSeed> {
    (
        // Mostly staying at the same depth or going one up, so trees reach the depth limit.
        (
            prop_oneof![6 => Just(0_u8), 2 => Just(1_u8), 1 => 2..6_u8],
            any::<u8>(),
        ),
        prop::collection::vec(any::<u16>(), 0..3),
        prop::option::weighted(0.3, (any::<u8>(), any::<u16>(), any::<u16>())),
        prop::option::weighted(0.3, (any::<u16>(), any::<bool>())),
        (
            any::<u8>(),
            prop::bool::weighted(0.1),
            prop::bool::weighted(0.15),
            prop::option::weighted(0.25, (any::<bool>(), any::<u8>())),
        ),
    )
        .prop_map(
            |((climb, kind), requires, condition, opening, (state, force, keep, participation))| {
                NodeSeed {
                    climb,
                    kind,
                    requires,
                    condition,
                    opening,
                    state,
                    force,
                    keep,
                    participation,
                }
            },
        )
}

/// A generated journey graph of `nodes` nodes, valid by construction.
pub fn arb_journey(nodes: RangeInclusive<usize>) -> impl Strategy<Value = Graph> {
    prop::collection::vec(arb_seed(), nodes).prop_map(|seeds| build(&seeds))
}

/// Builds the graph the seeds describe.
///
/// # Panics
///
/// When the built document breaks an invariant: a bug in this generator.
#[must_use]
pub fn build(seeds: &[NodeSeed]) -> Graph {
    let shape = Shape::new(seeds);
    let mut document = people();
    let mut edges = vec![0_u32; seeds.len()];
    for (index, seed) in seeds.iter().enumerate() {
        let node = shape.node(index, seed, &mut edges);
        document.nodes.put(node).unwrap_or_else(|e| panic!("{e}"));
    }
    for (index, seed) in seeds.iter().enumerate() {
        shape.state(&mut document, index, seed);
    }
    Graph::new(document).unwrap_or_else(|violations| panic!("{violations:#?}"))
}

/// The tree the seeds make: each node's kind, parent, and the end of its subtree.
struct Shape {
    kinds: Vec<NodeKind>,
    parents: Vec<Option<usize>>,
    ends: Vec<usize>,
}

impl Shape {
    fn new(seeds: &[NodeSeed]) -> Self {
        let depth_max = usize::try_from(CONTAINMENT_DEPTH_MAX).unwrap_or(16);
        let mut kinds = Vec::new();
        let mut parents = Vec::new();
        // The open path: containers on the way down to the newest node.
        let mut path: Vec<usize> = Vec::new();
        for (index, seed) in seeds.iter().enumerate() {
            for _ in 0..seed.climb {
                path.pop();
            }
            while path.len() >= depth_max {
                path.pop();
            }
            let kind = match seed.kind % 8 {
                0 => NodeKind::Decision,
                1 => NodeKind::Milestone,
                2 | 3 => NodeKind::Group,
                4 | 5 => NodeKind::Deliverable,
                _ => NodeKind::Action,
            };
            parents.push(path.last().copied());
            kinds.push(kind);
            if !kind.is_leaf() {
                path.push(index);
            }
        }
        let mut ends: Vec<usize> = (0..seeds.len()).collect();
        for index in (0..seeds.len()).rev() {
            if let Some(parent) = parents[index] {
                ends[parent] = ends[parent].max(ends[index]);
            }
        }
        Shape {
            kinds,
            parents,
            ends,
        }
    }

    /// The nodes whose subtrees end before `index` begins, of the kinds given.
    fn before(&self, index: usize, kinds: &[NodeKind]) -> Vec<usize> {
        (0..index)
            .filter(|&other| self.ends[other] < index && kinds.contains(&self.kinds[other]))
            .collect()
    }

    fn node(&self, index: usize, seed: &NodeSeed, edges: &mut [u32]) -> Node<KeyRefs> {
        let decisions = self.before(index, &[NodeKind::Decision]);
        let condition =
            seed.condition
                .filter(|_| !decisions.is_empty())
                .map(|(shape, first, second)| {
                    let pick = |at: u16| decisions[usize::from(at) % decisions.len()];
                    condition(shape, pick(first), pick(second))
                });
        let gated: BTreeSet<NodeKey> = condition
            .iter()
            .flat_map(|condition| condition.decisions().into_iter().cloned())
            .collect();
        let candidates = self.before(index, &NodeKind::ALL);
        let mut requires = BTreeSet::new();
        for pick in &seed.requires {
            let Some(&other) = candidates.get(usize::from(*pick) % candidates.len().max(1)) else {
                continue;
            };
            let full = |n: usize| edges[n] + 1 > EDGE_COUNT_PER_NODE_MAX;
            if gated.contains(&key(other)) || full(index) || full(other) {
                continue;
            }
            if requires.insert(key(other)) {
                edges[index] += 1;
                edges[other] += 1;
            }
        }
        Node {
            key: key(index),
            id: parse(&format!("g{index}")),
            parent: self.parents[index].map(key),
            title: parse(&format!("Generated {index}")),
            description: None,
            weight: None,
            requires: BoundedSet::new(requires).unwrap_or_default(),
            relevant_when: condition,
            due_by: None,
            not_before: None,
            participations: participations(seed.participation),
            resources: Vec::new(),
            payload: self.payload(index, seed),
        }
    }

    fn payload(&self, index: usize, seed: &NodeSeed) -> Payload<KeyRefs> {
        match self.kinds[index] {
            NodeKind::Decision => Payload::Decision(Decision {
                prompt: parse("Yes?"),
                help: None,
                answer: AnswerSpec::Boolean,
            }),
            NodeKind::Deliverable => Payload::Deliverable(Deliverable::default()),
            NodeKind::Action => Payload::Action(Action::default()),
            NodeKind::Milestone => Payload::Milestone(Milestone::default()),
            NodeKind::Group => Payload::Group(self.stage(index, seed)),
        }
    }

    /// A group's opening: a gating one ends before the group begins; a date-only one is any
    /// milestone outside the group.
    fn stage(&self, index: usize, seed: &NodeSeed) -> Group<KeyRefs> {
        let Some((pick, gates)) = seed.opening else {
            return Group::default();
        };
        let milestones: Vec<usize> = if gates {
            self.before(index, &[NodeKind::Milestone])
        } else {
            (0..self.kinds.len())
                .filter(|&other| self.kinds[other] == NodeKind::Milestone)
                .filter(|&other| other < index || other > self.ends[index])
                .collect()
        };
        let opens_at = milestones
            .get(usize::from(pick) % milestones.len().max(1))
            .map(|&milestone| key(milestone));
        Group {
            opens_at,
            gates,
            ..Group::default()
        }
    }

    /// A stored state legal for the node's kind, its answer, and its overrides.
    fn state(&self, document: &mut Document, index: usize, seed: &NodeSeed) {
        let kind = self.kinds[index];
        let mut stored = NodeState::initial(kind, Provenance::Local);
        let options: &[State] = match kind {
            NodeKind::Decision => &[State::Open, State::Decided, State::Decided, State::Skipped],
            NodeKind::Deliverable | NodeKind::Action => {
                &[State::Todo, State::Active, State::Done, State::Skipped]
            }
            NodeKind::Milestone => &[State::Pending, State::Reached, State::Skipped],
            NodeKind::Group => &[
                State::Derived,
                State::Derived,
                State::Derived,
                State::Skipped,
            ],
        };
        stored.state = options[usize::from(seed.state) % options.len()];
        if stored.state == State::Skipped {
            stored.skip_reason = Some(parse("Generated."));
        }
        let state = &mut document.state;
        if stored.state == State::Decided {
            state.answers.insert(
                key(index),
                AnswerValue::Boolean(seed.state.is_multiple_of(2)),
            );
        }
        state.nodes.insert(key(index), stored);
        let overrides = Overrides {
            force_include: seed.force.then(|| parse("Generated.")),
            keep: seed.keep.then(|| parse("Generated.")),
            bypass: None,
        };
        if !overrides.is_empty() {
            state.overrides.insert(key(index), overrides);
        }
    }
}

/// The roles and kinds every generated graph declares, and its direct role fills: a
/// single-valued role that is the default owner, a multi-valued role, and a multi-valued
/// kind wired to it.
fn people() -> Document {
    let mut document = Document::default();
    let role = |key: &str, multi: bool| Role {
        key: parse(key),
        id: parse(key.trim_start_matches("r_")),
        title: None,
        multi,
    };
    for found in [role("r_one", false), role("r_many", true)] {
        document.roles.put(found).unwrap_or_else(|e| panic!("{e}"));
    }
    let kind = ParticipationKind {
        key: parse("k_many"),
        id: parse("many"),
        title: None,
        multi: true,
    };
    document
        .participation_kinds
        .put(kind)
        .unwrap_or_else(|e| panic!("{e}"));
    document.default_owner = Some(parse("r_one"));
    let fills = [("r_one", &["e_a"][..]), ("r_many", &["e_a", "e_b"][..])];
    for (role, entities) in fills {
        let entities = BoundedSet::new(entities.iter().map(|entity| parse(entity)));
        let entities = entities.unwrap_or_else(|e| panic!("{e}"));
        document.state.role_fills.insert(parse(role), entities);
    }
    document
}

/// A node's participations from its seed.
fn participations(seed: Option<(bool, u8)>) -> Participations<KeyRefs> {
    let Some((owner, source)) = seed else {
        return Participations::default();
    };
    let (kind, role, lists): (KindKey, &str, &[&[&str]]) = if owner {
        (KindKey::owner(), "r_one", &[&["e_a"], &["e_b"], &[]])
    } else {
        (
            parse("k_many"),
            "r_many",
            &[&["e_a"], &["e_b"], &["e_a", "e_c"], &[]],
        )
    };
    let choice = usize::from(source) % (lists.len() + 1);
    let found = match lists.get(choice) {
        Some(entities) => {
            let entities = BoundedSet::new(entities.iter().map(|entity| parse(entity)));
            ParticipationSource::Entities(entities.unwrap_or_else(|e| panic!("{e}")))
        }
        None => ParticipationSource::Role(parse(role)),
    };
    let map = BTreeMap::from([(kind, found)]);
    Participations::try_from(map).unwrap_or_else(|e| panic!("{e}"))
}

/// The key of generated node `index`.
#[must_use]
pub fn key(index: usize) -> NodeKey {
    parse(&format!("n_g{index}"))
}

/// One of a few condition shapes over two decisions.
fn condition(shape: u8, first: usize, second: usize) -> Condition<KeyRefs> {
    let equals = |decision: usize, value: bool| {
        Clause::Equals(Comparison {
            decision: key(decision),
            value: ConditionValue::Boolean(value),
        })
    };
    let clause = match shape % 6 {
        0 => equals(first, true),
        1 => Clause::Not(Box::new(equals(first, true))),
        2 => Clause::Answered(key(first)),
        3 => Clause::NotEquals(Comparison {
            decision: key(first),
            value: ConditionValue::Boolean(false),
        }),
        4 => Clause::All(vec![equals(first, true), Clause::Answered(key(second))]),
        _ => Clause::Any(vec![equals(first, false), equals(second, true)]),
    };
    Condition::new(clause).unwrap_or_else(|e| panic!("{e}"))
}
