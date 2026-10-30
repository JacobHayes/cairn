//! Generated journeys built directly as documents (not through apply), for the derive passes'
//! property and reference tests: deep containment, explicit edges, conditions, stages
//! that require a milestone, stored states, answers, force includes, keeps, `auto_done` milestones, date
//! snoozes, and weights, all generic.
//!
//! Every generated graph is valid by construction. Nodes are created in depth-first
//! preorder, so each subtree is a contiguous run of indices; a requirement, a condition's
//! decision, or a stage's milestone is always a node whose whole subtree ends before the
//! dependent begins. Laying out each container's entries at its preorder index and each
//! node's start and finish at its subtree's end, deeper nodes first, puts every gate edge
//! forward in time, so the gate graph is acyclic.
// Test support: an index out of range is a bug in this generator, and the panic shows it.
#![allow(clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;

use proptest::prelude::*;

use cairn_schema::limits::{CONTAINMENT_DEPTH_MAX, EDGE_COUNT_PER_NODE_MAX, NODE_COUNT_MAX};
use cairn_schema::{
    Action, AnswerSpec, AnswerValue, BoundedSet, Bypass, Clause, Comparison, Condition,
    ConditionValue, DateRule, DateSource, Days, Decision, Deliverable, Direction, GuardFailure,
    KeyRefs, KindKey, Milestone, Node, NodeKey, NodeKind, NodeState, OneOrMany, Overrides,
    ParticipationKind, ParticipationSource, Participations, Payload, Role, State, Weight,
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
    /// A group's opening milestone, by index: it requires it.
    pub opening: Option<u16>,
    /// The stored state, by index among the kind's states.
    pub state: u8,
    /// Force-included.
    pub force: bool,
    /// Kept under a skipped ancestor.
    pub keep: bool,
    /// A participation: of `owner` (true) or the multi-valued kind, and its source by index
    /// (the kind's role, or one of a few explicit lists, the empty one included).
    pub participation: Option<(bool, u8)>,
    /// Dates: estimate, rules, a stage deadline, a pin, recorded dates.
    pub dates: DateSeed,
}

/// A node's dates, by index and offset. Rules measure from milestones anywhere in the graph
/// (or `created_at`), so they close loops the gate graph cannot; [`build`] drops the dates
/// of nodes on any contradictory chain until the plan holds.
#[derive(Clone, Debug, Default)]
pub struct DateSeed {
    /// A deliverable's or action's estimate, modulo 6 days.
    pub estimate: u8,
    /// `due_by`: its source by index among the milestones and `created_at`, `after` or
    /// `before`, and the offset modulo 15 days.
    pub due_by: Option<(u16, bool, u8)>,
    /// `not_before`, the same way.
    pub not_before: Option<(u16, bool, u8)>,
    /// A group's deadline milestone, by index among those outside it: its `due_by`, unless
    /// its seed gives it a rule.
    pub group_deadline: Option<u16>,
    /// A pin, days after 2026-09-20 modulo 60.
    pub pin: Option<u8>,
    /// Recorded dates for started or finished work: the start, days after 2026-09-20
    /// modulo 20, and the finish, days after the start modulo 10.
    pub recorded: (u8, u8),
}

/// A rule making a node due by a milestone, with no offset.
fn due_before(milestone: NodeKey) -> DateRule<KeyRefs> {
    DateRule {
        direction: Direction::Before,
        sources: OneOrMany::new(vec![DateSource::Node(milestone)])
            .unwrap_or_else(|e| panic!("{e}")),
        offset: Days::try_from(0).unwrap_or_else(|e| panic!("{e}")),
    }
}

fn arb_date_seed() -> impl Strategy<Value = DateSeed> {
    let rule = || prop::option::weighted(0.25, (any::<u16>(), any::<bool>(), any::<u8>()));
    (
        any::<u8>(),
        rule(),
        rule(),
        prop::option::weighted(0.3, any::<u16>()),
        prop::option::weighted(0.15, any::<u8>()),
        (any::<u8>(), any::<u8>()),
    )
        .prop_map(
            |(estimate, due_by, not_before, group_deadline, pin, recorded)| DateSeed {
                estimate,
                due_by,
                not_before,
                group_deadline,
                pin,
                recorded,
            },
        )
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
        prop::option::weighted(0.3, any::<u16>()),
        (
            any::<u8>(),
            prop::bool::weighted(0.1),
            prop::bool::weighted(0.15),
            prop::option::weighted(0.25, (any::<bool>(), any::<u8>())),
        ),
        arb_date_seed(),
    )
        .prop_map(
            |(
                (climb, kind),
                requires,
                condition,
                opening,
                (state, force, keep, participation),
                dates,
            )| {
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
                    dates,
                }
            },
        )
}

/// A generated journey graph of `nodes` nodes, valid by construction.
pub fn arb_journey(nodes: RangeInclusive<usize>) -> impl Strategy<Value = Graph> {
    prop::collection::vec(arb_seed(), nodes).prop_map(|seeds| build(&seeds))
}

/// A generated route graph of `nodes` nodes: a journey's structure with no state, valid by
/// construction. The seeds and structures are [`arb_journey`]'s; without state every node is
/// in scope, so [`build_route`] holds the plan over all of them rather than the journey's.
pub fn arb_route(nodes: RangeInclusive<usize>) -> impl Strategy<Value = Graph> {
    prop::collection::vec(arb_seed(), nodes).prop_map(|seeds| build_route(&seeds))
}

/// Builds the graph the seeds describe.
///
/// # Panics
///
/// When the built document breaks an invariant: a bug in this generator.
#[must_use]
pub fn build(seeds: &[NodeSeed]) -> Graph {
    plan_held(document(seeds))
}

/// Builds the route the seeds describe: [`build`]'s structure with no state. A journey's
/// stored state takes skipped and unchosen nodes out of scope, which drops their dates from
/// the plan; with no state they are back in, so the plan is held over the route itself.
///
/// # Panics
///
/// When the built document breaks an invariant: a bug in this generator.
#[must_use]
pub fn build_route(seeds: &[NodeSeed]) -> Graph {
    let mut document = document(seeds);
    document.state = cairn_schema::JourneyState::default();
    plan_held(document)
}

/// The document the seeds describe, its plan not yet held.
fn document(seeds: &[NodeSeed]) -> Document {
    let shape = Shape::new(seeds);
    let mut document = people();
    let mut edges = vec![0_u32; seeds.len()];
    for (index, seed) in seeds.iter().enumerate() {
        let node = shape.node(index, seed, &mut edges);
        document.nodes.put(node).unwrap_or_else(|e| panic!("{e}"));
    }
    for (index, seed) in seeds.iter().enumerate() {
        shape.state(&mut document, index, seed);
        shape.dates(&mut document, index, &seed.dates);
    }
    document
}

/// A graph at the limits (PRACTICES, Explicit limits: costed and tested at the limits):
/// `node_count_max` nodes with containment at the depth limit, 32 requirements tried per
/// node, conditions on a third of the nodes, a stage requirement on every group, and stored
/// states of every kind.
#[must_use]
pub fn limit_seeds() -> Vec<NodeSeed> {
    let count = NODE_COUNT_MAX as usize;
    (0..count)
        .map(|i| NodeSeed {
            climb: if i % 24 == 23 { 4 } else { 0 },
            kind: [2, 4, 6, 0, 1, 5][i % 6],
            requires: (0..32)
                .map(|j| u16::try_from((i * 131 + j * 977) % 65_536).unwrap_or(0))
                .collect(),
            condition: (i % 3 == 0).then(|| (u8::try_from(i % 6).unwrap_or(0), 7, 11)),
            opening: Some(u16::try_from(i).unwrap_or(0)),
            state: u8::try_from(i % 4).unwrap_or(0),
            force: i % 50 == 0,
            keep: false,
            participation: (i % 7 == 0).then(|| (i % 2 == 0, u8::try_from(i % 5).unwrap_or(0))),
            dates: DateSeed::default(),
        })
        .collect()
}

/// A journey of `nodes` nodes (at most `node_count_max`) shaped like a real route rather than
/// the limits' worst case: the limits' containment, kinds, stored states, and stage requirements,
/// but three requirements per node, conditions on one node in six, and no date rules. The
/// wasm build's agreement cases run it at 2,000 nodes.
///
/// # Panics
///
/// When `nodes` is past the limit or the graph breaks an invariant: a bug in this generator.
#[must_use]
pub fn route_like(nodes: usize) -> Graph {
    assert!(nodes <= NODE_COUNT_MAX as usize);
    let seeds: Vec<NodeSeed> = limit_seeds()
        .into_iter()
        .take(nodes)
        .enumerate()
        .map(|(at, seed)| NodeSeed {
            requires: seed.requires.into_iter().take(3).collect(),
            condition: (at % 6 == 0).then_some(seed.condition).flatten(),
            ..seed
        })
        .collect();
    build(&seeds)
}

/// The date network at the limits: [`limit_seeds`]' graph, without conditions so every node
/// is in scope, with both date rules on every node,
/// each with `edge_count_per_node_max` milestone sources, and
/// the same pin on every tenth milestone. Every rule offset is zero and there are no
/// estimates, so the rules tie most instants into one strongly connected component without
/// a positive cycle: Bellman-Ford's worst ground, at the constraint limit.
///
/// # Panics
///
/// When the graph breaks an invariant: a bug in this generator.
#[must_use]
pub fn date_limits() -> Graph {
    // Every node in scope, so no constraint is pruned.
    let seeds: Vec<NodeSeed> = limit_seeds()
        .into_iter()
        .map(|seed| NodeSeed {
            condition: None,
            ..seed
        })
        .collect();
    let mut document = build(&seeds).into_document();
    let milestones: Vec<NodeKey> = document
        .nodes
        .values()
        .filter(|node| node.kind() == NodeKind::Milestone)
        .map(|node| node.key.clone())
        .collect();
    let width = EDGE_COUNT_PER_NODE_MAX as usize;
    let keys: Vec<NodeKey> = document.nodes.as_map().keys().cloned().collect();
    for (at, key) in keys.iter().enumerate() {
        let Some(mut node) = document.nodes.get(key).cloned() else {
            continue;
        };
        let sources: Vec<DateSource<KeyRefs>> = (0..width)
            .map(|step| &milestones[(at * 7 + step * 13) % milestones.len()])
            .filter(|milestone| *milestone != key)
            .map(|milestone| DateSource::Node(milestone.clone()))
            .collect();
        let rule = |direction| DateRule {
            direction,
            sources: OneOrMany::new(sources.clone()).unwrap_or_else(|e| panic!("{e}")),
            offset: Days::try_from(0).unwrap_or_else(|e| panic!("{e}")),
        };
        node.due_by = Some(rule(Direction::After));
        node.not_before = Some(rule(Direction::Before));
        document.nodes.put(node).unwrap_or_else(|e| panic!("{e}"));
    }
    let pinned: cairn_schema::Date = parse("2026-11-02");
    for milestone in milestones.iter().step_by(10) {
        document.state.pins.insert(milestone.clone(), pinned);
    }
    Graph::new(document, &cairn_schema::Deployment::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}"))
}

/// [`date_limits`] with early work at the limits: every piece of work, decision and milestone
/// that was not skipped is finished, ahead of what it waits on, so every constraint into
/// finished work is exempt.
///
/// # Panics
///
/// When the graph breaks an invariant: a bug in this generator.
#[must_use]
pub fn date_limits_early() -> Graph {
    let mut document = date_limits().into_document();
    let keys: Vec<NodeKey> = document.nodes.as_map().keys().cloned().collect();
    let recorded: cairn_schema::Date = parse("2026-09-25");
    for key in &keys {
        let Some(node) = document.nodes.get(key) else {
            continue;
        };
        let finished = match node.kind() {
            NodeKind::Group => continue,
            NodeKind::Decision => State::Decided,
            NodeKind::Milestone | NodeKind::Deliverable | NodeKind::Action => State::Done,
        };
        let Some(stored) = document.state.nodes.get_mut(key) else {
            continue;
        };
        if stored.state == State::Skipped {
            continue;
        }
        stored.state = finished;
        stored.started_on = (finished == State::Done).then_some(recorded);
        stored.finished_on = Some(recorded);
        stored.finished_revision = Some(revision(1));
        if finished == State::Decided {
            document
                .state
                .answers
                .insert(key.clone(), AnswerValue::Boolean(true));
        }
    }
    Graph::new(document, &cairn_schema::Deployment::default())
        .unwrap_or_else(|violations| panic!("{violations:#?}"))
}

/// The day this many days after the generators' first day.
/// A journey revision, for a recorded finish or reopen.
fn revision(at: u32) -> cairn_schema::Revision {
    cairn_schema::Revision::try_from(at).unwrap_or_else(|e| panic!("{e:?}"))
}

fn day(days: u32) -> cairn_schema::Date {
    let start: cairn_schema::Date = parse("2026-09-20");
    start
        .checked_add(jiff::Span::new().days(i64::from(days)))
        .unwrap_or_else(|e| panic!("{e}"))
}

/// The graph of `document`, with the dates of every node on a contradictory chain dropped
/// until its plan holds (F5); a graph with no dates at all holds, so this ends. A test that
/// changes a generated document's state calls this on the result: the state decides which
/// nodes are in scope, and the plan holds over those.
///
/// # Panics
///
/// When the document breaks an invariant other than a contradictory chain.
#[must_use]
pub fn plan_held(mut document: Document) -> Graph {
    let deployment = cairn_schema::Deployment::default();
    loop {
        let violations = match Graph::new(document.clone(), &deployment) {
            Ok(graph) => return graph,
            Err(violations) => violations,
        };
        let mut on_chains = BTreeSet::new();
        for found in violations.as_slice() {
            assert_eq!(
                found.code,
                cairn_schema::ViolationCode::ContradictoryChain,
                "{found:#?}"
            );
            for subject in &found.related {
                if let cairn_schema::Subject::Node(node) = subject {
                    on_chains.insert(node.clone());
                }
            }
        }
        assert!(
            !on_chains.is_empty(),
            "a contradictory chain names its nodes"
        );
        for key in &on_chains {
            undate(&mut document, key);
        }
    }
}

/// Drops a node's rules, estimate, and pin.
fn undate(document: &mut Document, key: &NodeKey) {
    document.state.pins.remove(key);
    let Some(mut node) = document.nodes.get(key).cloned() else {
        return;
    };
    node.due_by = None;
    node.not_before = None;
    match &mut node.payload {
        Payload::Deliverable(deliverable) => deliverable.estimate = None,
        Payload::Action(action) => action.estimate = None,
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group => {}
    }
    document.nodes.put(node).unwrap_or_else(|e| panic!("{e}"));
}

/// A weight from 0 to 5 for most nodes, from the recorded finish offset's high bits (its low
/// bits are the offset, modulo 10); the rest keep their kind's default.
fn weight(seed: &NodeSeed) -> Option<Weight> {
    let bits = seed.dates.recorded.1 / 16;
    (!bits.is_multiple_of(4))
        .then(|| Weight::try_from(u32::from(bits % 6)).unwrap_or(Weight::DEFAULT))
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
        let requires = self.with_opening(index, seed, requires, edges);
        Node {
            key: key(index),
            id: parse(&format!("g{index}")),
            parent: self.parents[index].map(key),
            title: parse(&format!("Generated {index}")),
            description: None,
            weight: weight(seed),
            requires,
            relevant_when: condition,
            due_by: None,
            not_before: None,
            participations: participations(seed.participation),
            resources: Vec::new(),
            from_route: None,
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
            // Half the milestones auto-done (F1); the estimate seed is a milestone's spare bits.
            NodeKind::Milestone => Payload::Milestone(Milestone {
                auto_done: seed.dates.estimate.is_multiple_of(2),
                ..Milestone::default()
            }),
            NodeKind::Group => Payload::Group,
        }
    }

    /// A group's opening milestone, which ends before the group begins, joins its requirements
    /// (the edge limit allowing).
    fn with_opening(
        &self,
        index: usize,
        seed: &NodeSeed,
        mut requires: BTreeSet<NodeKey>,
        edges: &mut [u32],
    ) -> cairn_schema::node::Requires<KeyRefs> {
        let milestones = self.before(index, &[NodeKind::Milestone]);
        let opening = seed
            .opening
            .filter(|_| self.kinds[index] == NodeKind::Group)
            .and_then(|pick| milestones.get(usize::from(pick) % milestones.len().max(1)));
        if let Some(&milestone) = opening {
            let full = |n: usize| edges[n] + 1 > EDGE_COUNT_PER_NODE_MAX;
            if !full(index) && !full(milestone) && requires.insert(key(milestone)) {
                edges[index] += 1;
                edges[milestone] += 1;
            }
        }
        BoundedSet::new(requires).unwrap_or_default()
    }

    /// A group's deadline, when its seed picks a milestone outside it.
    fn group_deadline(
        &self,
        index: usize,
        seed: &DateSeed,
        milestones: &[usize],
    ) -> Option<DateRule<KeyRefs>> {
        let outside: Vec<usize> = milestones
            .iter()
            .copied()
            .filter(|&other| other < index || other > self.ends[index])
            .collect();
        let pick = usize::from(seed.group_deadline?) % outside.len().max(1);
        outside
            .get(pick)
            .map(|&milestone| due_before(key(milestone)))
    }

    /// The node's dates from its seed.
    fn dates(&self, document: &mut Document, index: usize, seed: &DateSeed) {
        let milestones: Vec<usize> = (0..self.kinds.len())
            .filter(|&other| other != index && self.kinds[other] == NodeKind::Milestone)
            .collect();
        let rule = |found: Option<(u16, bool, u8)>| {
            let (pick, after, offset) = found?;
            let at = usize::from(pick) % (milestones.len() + 1);
            let source = milestones
                .get(at)
                .map_or(DateSource::CreatedAt, |&m| DateSource::Node(key(m)));
            let rule = DateRule {
                direction: if after {
                    Direction::After
                } else {
                    Direction::Before
                },
                sources: OneOrMany::new(vec![source]).unwrap_or_else(|e| panic!("{e}")),
                offset: Days::try_from(u32::from(offset % 15)).unwrap_or_else(|e| panic!("{e}")),
            };
            Some(rule)
        };
        let started = u32::from(seed.recorded.0 % 20);
        let finished = started + u32::from(seed.recorded.1 % 10);
        if let Some(stored) = document.state.nodes.get_mut(&key(index)) {
            let (start, finish) = match stored.state {
                State::Active => (Some(day(started)), None),
                // A group marked done has no performed dates.
                State::Done if self.kinds[index] == NodeKind::Group => (None, None),
                // Some work is completed straight from todo, with no start recorded.
                State::Done => (
                    (seed.recorded.0 < 192).then_some(day(started)),
                    Some(day(finished)),
                ),
                State::Decided => (None, Some(day(finished))),
                State::Todo | State::Open | State::Pending | State::Derived | State::Skipped => {
                    (None, None)
                }
            };
            stored.started_on = start;
            stored.finished_on = finish;
            stored.finished_revision =
                finish.map(|_| revision(1 + u32::from(seed.recorded.0) % 10));
        }
        if let Some(pin) = seed.pin {
            document
                .state
                .pins
                .insert(key(index), day(u32::from(pin % 60)));
        }
        let Some(mut node) = document.nodes.get(&key(index)).cloned() else {
            return;
        };
        node.due_by = rule(seed.due_by);
        node.not_before = rule(seed.not_before);
        let estimate = Days::try_from(u32::from(seed.estimate % 6)).ok();
        match &mut node.payload {
            Payload::Deliverable(deliverable) => deliverable.estimate = estimate,
            Payload::Action(action) => action.estimate = estimate,
            Payload::Group => {
                if node.due_by.is_none() {
                    node.due_by = self.group_deadline(index, seed, &milestones);
                }
            }
            Payload::Decision(_) | Payload::Milestone(_) => {}
        }
        document.nodes.put(node).unwrap_or_else(|e| panic!("{e}"));
    }

    /// A guard bypass on some finished work (D4), recording a missing note or breakdown.
    fn bypass(&self, index: usize, seed: &NodeSeed, state: State) -> Option<Bypass> {
        let work = matches!(self.kinds[index], NodeKind::Deliverable | NodeKind::Action);
        let pick = seed.dates.recorded.0 / 32;
        if state != State::Done || !work || pick.is_multiple_of(4) {
            return None;
        }
        let failure = if pick >= 6 {
            GuardFailure::NotBrokenDown
        } else {
            GuardFailure::MissingNote
        };
        Some(Bypass {
            reason: pick.is_multiple_of(2).then(|| parse("Generated.")),
            failures: BTreeSet::from([failure]),
        })
    }

    /// A reopen record on some open nodes (D4): a finish reopened around the fixed clock.
    fn reopened(seed: &NodeSeed, state: State) -> Option<cairn_schema::Reopened> {
        let recorded = u32::from(seed.dates.recorded.0);
        if state.is_terminal() || recorded % 3 != 1 {
            return None;
        }
        Some(cairn_schema::Reopened {
            revision: revision(20 + recorded % 10),
            finished_revision: revision(1 + recorded % 5),
        })
    }

    /// A stored state legal for the node's kind, its answer, and its overrides.
    fn state(&self, document: &mut Document, index: usize, seed: &NodeSeed) {
        let kind = self.kinds[index];
        let mut stored = NodeState::initial(kind);
        // A weighted mix, not a copy of the kind's machine: repeats make decided decisions and
        // derived groups likelier, so generated graphs carry the open and active work the cost
        // and priority tests rely on. The schema owns which states are legal (D1).
        let options: &[State] = match kind {
            NodeKind::Decision => &[State::Open, State::Decided, State::Decided, State::Skipped],
            NodeKind::Deliverable | NodeKind::Action => {
                &[State::Todo, State::Active, State::Done, State::Skipped]
            }
            NodeKind::Milestone => &[State::Pending, State::Done, State::Skipped],
            NodeKind::Group => &[
                State::Derived,
                State::Derived,
                State::Derived,
                State::Skipped,
                State::Done,
            ],
        };
        assert!(options.iter().all(|state| state.legal_for(kind)));
        stored.state = options[usize::from(seed.state) % options.len()];
        if stored.state.skips_beneath(kind) {
            stored.reason = Some(parse("Generated."));
        }
        let bypass = self.bypass(index, seed, stored.state);
        stored.reopened = Self::reopened(seed, stored.state);
        let state = &mut document.state;
        if stored.state == State::Decided {
            state.answers.insert(
                key(index),
                AnswerValue::Boolean(seed.state.is_multiple_of(2)),
            );
        }
        // Some open work is snoozed until a day around the fixed clock (B6).
        if !stored.state.is_terminal() && kind != NodeKind::Group && seed.state % 5 == 1 {
            let until: cairn_schema::Date = parse("2026-10-01");
            let days = i64::from(seed.state % 10);
            let until = until
                .checked_add(jiff::Span::new().days(days))
                .unwrap_or_else(|e| panic!("{e}"));
            state
                .snoozes
                .insert(key(index), cairn_schema::SnoozeTarget::Date(until));
        }
        state.nodes.insert(key(index), stored);
        let overrides = Overrides {
            force_include: seed.force.then(|| parse("Generated.")),
            keep: seed.keep.then(|| parse("Generated.")),
            bypass,
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
    let fills = [
        ("r_one", &["who_a"][..]),
        ("r_many", &["who_a", "who_b"][..]),
    ];
    for (role, identities) in fills {
        let identities = BoundedSet::new(identities.iter().map(|identity| parse(identity)));
        let identities = identities.unwrap_or_else(|e| panic!("{e}"));
        document.state.role_fills.insert(parse(role), identities);
    }
    document
}

/// A node's participations from its seed.
fn participations(seed: Option<(bool, u8)>) -> Participations<KeyRefs> {
    let Some((owner, source)) = seed else {
        return Participations::default();
    };
    let (kind, role, lists): (KindKey, &str, &[&[&str]]) = if owner {
        (KindKey::owner(), "r_one", &[&["who_a"], &["who_b"], &[]])
    } else {
        (
            parse("k_many"),
            "r_many",
            &[&["who_a"], &["who_b"], &["who_a", "who_c"], &[]],
        )
    };
    let choice = usize::from(source) % (lists.len() + 1);
    let found = match lists.get(choice) {
        Some(identities) => {
            let identities = BoundedSet::new(identities.iter().map(|identity| parse(identity)));
            ParticipationSource::Identities(identities.unwrap_or_else(|e| panic!("{e}")))
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
