//! Pass 4, the date network (PRD F1 to F7, A8, A9; Invariants: the date line; ARCHITECTURE,
//! Engine > Date network): one network of instants and difference constraints, "B is at
//! least k days after A", read in two layers. The plan layer (pins, rules, estimates,
//! containment, stage bounds, dependencies; no actuals, no today) must hold no contradictory
//! chain, which validation enforces. The execution layer adds actuals and today and is what
//! derive solves into bounds, `overdue`, and `shortfall`, each with its chain.
//!
//! # Translation table
//!
//! Instants (F2). Each node has 2.2's four instants ([`super::dependencies::Point`]): a start
//! and a finish, and for a container its two entries, which are internal (never shown) and
//! collapsed out of every chain. A decision or milestone has one instant: its start is its
//! finish. The journey has `created_at`, and each date decision its answer, an instant apart
//! from when it was decided.
//!
//! Constraints, each `after >= before + k`. "Holds back" raises `after`'s earliest date;
//! "caps" lowers `before`'s latest date. Every constraint does both (F3); the column says which
//! the author usually means.
//!
//! | Source | Constraint | Usually |
//! |---|---|---|
//! | `requires` R on N (A3), inherited by N's descendants through N's entry | N's anchor >= R.finish + 0 | holds N back |
//! | condition gate: N's condition reads decision D | N's condition anchor >= D + 0 | holds N back |
//! | stage opening: group G `opens_at` M (`gates: false` too: date-only, still a date) | G's anchor >= M + 0 | holds G's contents back |
//! | containment, child C of P | C's anchors >= P's entries + 0; P.start >= C.finish + 0; P.start >= P's entries + 0 | holds C back; caps C by P |
//! | estimate e on N (A9) | N.finish >= N.start + e (e is 0 without an estimate, for a group, and for skipped work) | holds N's finish back |
//! | `due_by {before: S, offset: k}` on N (A8) | S >= N.finish + k | caps N's finish at S - k |
//! | `due_by {after: S, offset: k}` | S >= N.finish - k | caps N's finish at S + k |
//! | `not_before {after: S, offset: k}` | N.start >= S + k | holds N back to S + k |
//! | `not_before {before: S, offset: k}` | N.start >= S - k | holds N back to S - k |
//! | rule source S | a milestone: its instant; a date decision: its answer; `journey.created_at`: `created_at` | |
//! | stage close: G `closes_at` M, `closes: true` (F4) | M >= G.finish + 0 | caps G, and through containment its contents |
//! | pin on N, or a `feeds_milestone` answer in effect (E3) | none: fixes N.finish (a decision's or milestone's one instant) from both sides | both |
//! | date answer of D in effect | none: fixes D's answer from both sides | both |
//! | actual: a recorded start, or the finish of done, decided, or reached work (execution only) | none: fixes the instant, never moved | both |
//! | `created_on` of a journey (execution only) | none: fixes `created_at` | both |
//! | today (execution only) | none: open decision, unstarted work's start, started work's finish no earlier than today | holds back |
//!
//! A node's anchor is its requirement entry if it is a container, else its start; its
//! condition anchor is its condition entry, or its start. Constraints touching a not-relevant
//! node are dropped (2.2's pruned set, and rules and closes whose node or source is not
//! relevant); those through an undecided node are kept and marked conditional (F2).
//! Effectively skipped work, and work skipped itself, has zero duration and seeds nothing.
//!
//! # Cost at the limits
//!
//! At `node_count_max` (2,000 nodes, 64 explicit edges each in plus out, 16 condition clauses
//! each, two rules of 64 sources each): 5 x 2,000 + 1 = 10,001 instant slots (four per node,
//! `created_at`, one answer per node). Constraints: 2.2's edges, about 110,000 (6 per node of
//! work, entries, and containment; 64,000 explicit; 32,000 condition gates; 2,000 openings),
//! plus 256,000 rule constraints and 2,000 stage closes, about 370,000 of 20 bytes each and
//! two `u32` adjacency slots each: about 10 MiB. Building is O(constraints log constraints)
//! for the two adjacency sorts. Each solve (the plan check, earliest, latest) finds the
//! strongly connected components once, O(instants + constraints), visits components in
//! topological order, and relaxes each constraint once outside cycles; only a component with
//! a cycle (gate edges are acyclic, so it needs a rule, a stage close, or a date-only
//! opening) runs queue-based Bellman-Ford, at most (its instants) rounds of (its
//! constraints): O(instants x constraints) = 10,001 x 370,000, about 3.7 x 10^9 relaxations,
//! in the worst case of every instant in one cycle. The plan check reruns Bellman-Ford on a
//! component after each contradictory chain it lists, at most
//! `chain_count_per_rejection_max` + 1 times. Chains are not stored: each bound keeps the
//! constraint that set it, and a chain is walked back from it when asked, at most one step
//! per instant. The cost test (`tests/integration/cost_dates.rs`) counts these operations at
//! the limits.

mod execution;
mod explain;
pub(crate) mod network;
mod plan;
mod solve;

use std::collections::BTreeSet;

use cairn_schema::{
    AnswerSpec, AnswerValue, Bound, BoundedVec, ChainList, Date, DateOrigin, Deployment,
    EffectiveDate, FixedBy, NodeDates, NodeKey, NodeKind, Payload, ShortChain, State, Subject,
    Violation, ViolationCode,
};

use super::dependencies::{Dependencies, NodeIndex};
use super::relevance::Relevances;
use super::{Early, stored_state};
use crate::graph::{Document, Graph};
use execution::Solved;
use explain::{Explain, Found};
use network::{Layer, Network, Scope, Slot};

/// A date fixed on a slot, and what fixed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fix {
    /// The date, in days since 1970-01-01.
    pub day: i32,
    /// A pin, an answer, an actual, or today.
    pub by: FixedBy,
}

/// What fixes each slot: the plan's pins and date answers, and in the execution layer the
/// actuals and today too.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Seeds {
    /// Per slot: a pin (direct, or from a feeding decision's answer) or a date answer.
    pins: Vec<Option<Fix>>,
    /// Per slot: an actual date, or `created_at`: fixed, never moved.
    actuals: Vec<Option<Fix>>,
    /// Per slot: today, as a floor.
    today: Vec<Option<Fix>>,
    /// Per node: the decision whose answer in effect pins it (E3).
    feeding: Vec<Option<NodeIndex>>,
}

impl Seeds {
    /// The plan's seeds (F5): pins on nodes in scope, the pins `feeds_milestone` answers in
    /// effect give, and the date answers in effect that rules measure from.
    #[must_use]
    pub(crate) fn plan(
        document: &Document,
        dependencies: &Dependencies,
        relevance: &Relevances,
        network: &Network,
        scope: &Scope,
    ) -> Self {
        let slots = network.slot_count();
        let mut seeds = Seeds {
            pins: vec![None; slots],
            actuals: vec![None; slots],
            today: vec![None; slots],
            feeding: vec![None; network.node_count()],
        };
        for (key, date) in &document.state.pins {
            if let Some(at) = dependencies
                .node_index(key)
                .filter(|at| scope.in_scope(*at))
            {
                seeds.set_pin(network.finish(at), day(*date), FixedBy::Pin);
            }
        }
        for node in document.nodes.values() {
            let Payload::Decision(decision) = &node.payload else {
                continue;
            };
            let AnswerSpec::Date { feeds_milestone } = &decision.answer else {
                continue;
            };
            let Some(AnswerValue::Date(answered)) = relevance.answer_in_effect(document, &node.key)
            else {
                continue;
            };
            let Some(at) = dependencies.node_index(&node.key) else {
                continue;
            };
            seeds.set_pin(network.answer(at), day(*answered), FixedBy::Answer);
            let fed = feeds_milestone
                .as_ref()
                .and_then(|m| dependencies.node_index(m));
            if let Some(milestone) = fed.filter(|m| scope.in_scope(*m)) {
                seeds.set_pin(network.finish(milestone), day(*answered), FixedBy::Pin);
                if let Some(feeding) = seeds.feeding.get_mut(milestone.get()) {
                    *feeding = Some(at);
                }
            }
        }
        seeds
    }

    /// The execution layer's seeds (F3, F6): the plan's, plus each in-scope node's actuals
    /// and today by its state, and the journey's `created_at`. Skipped work seeds nothing.
    #[must_use]
    pub(crate) fn execution(
        document: &Document,
        dependencies: &Dependencies,
        relevance: &Relevances,
        network: &Network,
        scope: &Scope,
        today: Date,
        created_on: Option<Date>,
    ) -> Self {
        let mut seeds = Seeds::plan(document, dependencies, relevance, network, scope);
        let today = day(today);
        for node in document.nodes.values() {
            let Some(at) = dependencies.node_index(&node.key) else {
                continue;
            };
            if !scope.in_scope(at) || scope.skipped(at) {
                continue;
            }
            let stored = document.state.nodes.get(&node.key);
            let started = stored.and_then(|found| found.started_on).map(day);
            let finished = stored.and_then(|found| found.finished_on).map(day);
            let (start, finish) = (network.start(at), network.finish(at));
            match stored_state(document, node) {
                State::Todo | State::Open => seeds.set_today(start, today),
                State::Active => {
                    seeds.set_actual(start, started);
                    seeds.set_today(finish, today);
                }
                State::Done => {
                    seeds.set_actual(start, started);
                    seeds.set_actual(finish, finished);
                }
                State::Decided | State::Reached => seeds.set_actual(finish, finished),
                State::Pending | State::Derived | State::Skipped => {}
            }
        }
        seeds.set_actual(network.created_at(), created_on.map(day));
        seeds
    }

    fn set_today(&mut self, slot: Slot, day: i32) {
        if let Some(today) = self.today.get_mut(slot.index()) {
            *today = Some(Fix {
                day,
                by: FixedBy::Today,
            });
        }
    }

    fn set_actual(&mut self, slot: Slot, day: Option<i32>) {
        if let (Some(actual), Some(day)) = (self.actuals.get_mut(slot.index()), day) {
            *actual = Some(Fix {
                day,
                by: FixedBy::Actual,
            });
        }
    }

    fn set_pin(&mut self, slot: Slot, day: i32, by: FixedBy) {
        if let Some(pin) = self.pins.get_mut(slot.index()) {
            *pin = Some(Fix { day, by });
        }
    }

    /// The slot's pin or date answer.
    #[must_use]
    pub(crate) fn pin(&self, slot: Slot) -> Option<Fix> {
        self.pins.get(slot.index()).copied().flatten()
    }

    /// The slot's actual, if it has one.
    #[must_use]
    pub(crate) fn actual(&self, slot: Slot) -> Option<Fix> {
        self.actuals.get(slot.index()).copied().flatten()
    }

    /// What holds the slot back from below: its actual, else the later of its pin and today.
    #[must_use]
    pub(crate) fn lower(&self, slot: Slot) -> Option<Fix> {
        if let Some(actual) = self.actual(slot) {
            return Some(actual);
        }
        let today = self.today.get(slot.index()).copied().flatten();
        match (self.pin(slot), today) {
            (Some(pin), Some(today)) if today.day > pin.day => Some(today),
            (Some(pin), _) => Some(pin),
            (None, today) => today,
        }
    }

    /// What caps the slot from above: its actual, else its pin.
    #[must_use]
    pub(crate) fn upper(&self, slot: Slot) -> Option<Fix> {
        self.actual(slot).or_else(|| self.pin(slot))
    }

    /// Today, where it holds the slot back.
    #[must_use]
    pub(crate) fn floor(&self, slot: Slot) -> Option<Fix> {
        self.today.get(slot.index()).copied().flatten()
    }

    /// The decision whose answer pins the node, while it is in effect.
    #[must_use]
    pub(crate) fn feeding(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.feeding.get(node.get()).copied().flatten()
    }
}

const EPOCH: Date = Date::constant(1970, 1, 1);

/// A date as days since 1970-01-01 (PRACTICES, Fixed-width integers).
#[must_use]
pub(crate) fn day(date: Date) -> i32 {
    date.since(EPOCH).map_or(0, |span| span.get_days())
}

/// The date `day` days after 1970-01-01, if there is one.
#[must_use]
pub(crate) fn checked_date(day: i32) -> Option<Date> {
    let span = jiff::Span::new().try_days(i64::from(day)).ok()?;
    EPOCH.checked_add(span).ok()
}

/// The date `day` days after 1970-01-01, clamped to the dates there are.
#[must_use]
pub(crate) fn date(day: i32) -> Date {
    let span = jiff::Span::new().try_days(i64::from(day));
    match span.and_then(|span| EPOCH.checked_add(span)) {
        Ok(found) => found,
        Err(_) if day < 0 => Date::MIN,
        Err(_) => Date::MAX,
    }
}

/// The plan layer's verdict on a graph (F5): the contradictory chains it holds, up to
/// `chain_count_per_rejection_max`, and what checking cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCheck {
    chains: ChainList,
    operations: u64,
}

impl PlanCheck {
    /// The contradictory chains found, and whether there were more than are listed.
    #[must_use]
    pub fn chains(&self) -> &ChainList {
        &self.chains
    }

    /// True when the plan holds no contradictory chain.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.chains.chains.is_empty()
    }

    /// Operations the check spent: slots and constraints visited and relaxations tried.
    #[must_use]
    pub fn operation_count(&self) -> u64 {
        self.operations
    }
}

/// F5: checks a graph's plan layer for contradictory chains, each with its constraints, their
/// sources, the pins on it, its shortfall in days, and the moves that would resolve it.
/// Relevance reads the deployment (entity aliases), so the same one apply validates with.
#[must_use]
pub fn check_plan(graph: &Graph, deployment: &Deployment) -> PlanCheck {
    let early = super::early(graph, deployment);
    let document = graph.document();
    let scope = Scope::new(
        document,
        &early.dependencies,
        &early.relevance,
        &early.skips,
        Layer::Plan,
    );
    let network = Network::build(document, &early.dependencies, &scope);
    let seeds = Seeds::plan(
        document,
        &early.dependencies,
        &early.relevance,
        &network,
        &scope,
    );
    let findings = plan::check(&network, &seeds);
    let explain = Explain {
        document,
        keys: early.dependencies.keys(),
        network: &network,
        seeds: &seeds,
    };
    let shown: Vec<_> = findings
        .chains
        .iter()
        .map(|found| explain.short(found))
        .collect();
    let chains = ChainList {
        chains: BoundedVec::new(shown).unwrap_or_else(|_| unreachable!("cut to the limit")),
        more: findings.more,
    };
    PlanCheck {
        chains,
        operations: findings.cost.operations,
    }
}

/// The plan check as a violation (A15: one `contradictory_chain` listing every chain found),
/// located at the first chain's first node and naming every node on the chains.
#[must_use]
pub(crate) fn plan_violation(graph: &Graph, deployment: &Deployment) -> Option<Violation> {
    let checked = check_plan(graph, deployment);
    if checked.is_consistent() {
        return None;
    }
    let chains = checked.chains;
    let mut nodes: BTreeSet<NodeKey> = BTreeSet::new();
    for short in chains.chains.as_slice() {
        for constraint in &short.chain.constraints {
            for instant in [&constraint.before, &constraint.after] {
                if let cairn_schema::Instant::Node { node, .. } = instant {
                    nodes.insert(node.clone());
                }
            }
        }
    }
    let first = chains.chains.as_slice().first().and_then(|short| {
        short
            .chain
            .constraints
            .iter()
            .find_map(|constraint| match &constraint.after {
                cairn_schema::Instant::Node { node, .. } => Some(node.clone()),
                cairn_schema::Instant::CreatedAt | cairn_schema::Instant::Answer { .. } => None,
            })
    });
    let days: Vec<String> = chains
        .chains
        .as_slice()
        .iter()
        .map(|short| short.shortfall_days.to_string())
        .collect();
    let message = format!(
        "the plan holds {} contradictory chain{}{} short by {} days (F5)",
        days.len(),
        if days.len() == 1 { "" } else { "s" },
        if chains.more { " and more," } else { "," },
        days.join(", "),
    );
    let mut found = match &first {
        Some(key) => crate::validate::at_node(
            graph.tree(),
            key,
            ViolationCode::ContradictoryChain,
            message,
        ),
        None => crate::validate::violation(ViolationCode::ContradictoryChain, message),
    };
    found.related = nodes.into_iter().map(Subject::Node).collect();
    found.chains = Some(chains);
    Some(found)
}

/// Pass 4's output (F1, F3, F6, F7): every instant's earliest and latest bound in the
/// execution layer, with what set each, so a chain is walked back only when asked. Raw dates
/// are read by key; [`Dates::node`] gives a node's dates with their chains, read against the
/// graph they were derived from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dates {
    today: i32,
    /// Node keys in index order (key order), for lookups and chains.
    keys: Vec<NodeKey>,
    network: Network,
    seeds: Seeds,
    solved: Solved,
    /// Per node: unfinished work in scope (non-terminal and not skipped; a group while any
    /// such work lies beneath it).
    unfinished: Vec<bool>,
    /// Per node: a milestone, which has an effective date.
    milestone: Vec<bool>,
}

impl Dates {
    /// Pass 4: builds the network from passes 1 and 2 and solves its execution layer at
    /// `today`, with the journey's `created_on` (none for a route).
    #[must_use]
    pub(crate) fn pass(
        graph: &Graph,
        early: &Early,
        today: Date,
        created_on: Option<Date>,
    ) -> Self {
        let document = graph.document();
        let dependencies = &early.dependencies;
        let scope = Scope::new(
            document,
            dependencies,
            &early.relevance,
            &early.skips,
            Layer::Execution,
        );
        let network = Network::build(document, dependencies, &scope);
        let seeds = Seeds::execution(
            document,
            dependencies,
            &early.relevance,
            &network,
            &scope,
            today,
            created_on,
        );
        let solved = execution::solve(&network, &seeds);
        let keys = dependencies.keys().to_vec();
        let milestone = keys
            .iter()
            .map(|key| {
                graph
                    .node(key)
                    .is_some_and(|node| node.kind() == NodeKind::Milestone)
            })
            .collect();
        let unfinished = unfinished(graph, dependencies, &scope);
        assert_eq!(keys.len(), network.node_count());
        Dates {
            today: day(today),
            keys,
            network,
            seeds,
            solved,
            unfinished,
            milestone,
        }
    }

    fn index(&self, key: &NodeKey) -> Option<NodeIndex> {
        let at = self.keys.binary_search(key).ok()?;
        Some(NodeIndex::from_position(at))
    }

    /// F1, F6: a milestone that reads as reached (pass 5) is finished work, so neither it nor
    /// a group whose only unfinished work it was is overdue. Pass 4 cannot know it: auto-reach
    /// reads the effective dates this pass derives.
    pub(crate) fn settle_reached(&mut self, graph: &Graph, reached: impl Fn(&NodeKey) -> bool) {
        let before = self.unfinished.clone();
        let group = |key: &NodeKey| {
            graph
                .node(key)
                .is_some_and(|node| node.kind() == NodeKind::Group)
        };
        for (at, key) in self.keys.iter().enumerate() {
            if let Some(flag) = self.unfinished.get_mut(at) {
                *flag = *flag && !reached(key) && !group(key);
            }
        }
        for (at, key) in self.keys.iter().enumerate() {
            if !self.unfinished.get(at).copied().unwrap_or(false) || group(key) {
                continue;
            }
            let mut current = graph.tree().parent(key);
            while let Some(parent) = current {
                if let Ok(above) = self.keys.binary_search(parent)
                    && before.get(above) == Some(&true)
                    && let Some(flag) = self.unfinished.get_mut(above)
                {
                    *flag = true;
                }
                current = graph.tree().parent(parent);
            }
        }
        // Only ever clears: nothing becomes unfinished.
        assert!(
            self.unfinished
                .iter()
                .zip(&before)
                .all(|(after, before)| !after || *before)
        );
    }

    /// F3: the earliest the node can start.
    #[must_use]
    pub fn earliest_start(&self, key: &NodeKey) -> Option<Date> {
        let at = self.index(key)?;
        self.solved.earliest(self.network.start(at)).map(date)
    }

    /// F3: the latest the node can start without missing what caps it.
    #[must_use]
    pub fn latest_start(&self, key: &NodeKey) -> Option<Date> {
        let at = self.index(key)?;
        self.solved.latest(self.network.start(at)).map(date)
    }

    /// F3: the node's due date, the latest it can finish.
    #[must_use]
    pub fn due(&self, key: &NodeKey) -> Option<Date> {
        let at = self.index(key)?;
        self.solved.latest(self.network.finish(at)).map(date)
    }

    /// F3: latest start minus today; none is "no deadline". Work whose start is a fact is
    /// judged by its finish (F4, F6): its slack is its due date minus today.
    #[must_use]
    pub fn slack_days(&self, key: &NodeKey) -> Option<i32> {
        let at = self.index(key)?;
        let start = self.network.start(at);
        let judged = if self.seeds.actual(start).is_some() {
            self.network.finish(at)
        } else {
            start
        };
        let latest = self.solved.latest(judged)?;
        Some(latest.saturating_sub(self.today))
    }

    /// F6: unfinished work in scope whose due date is before today.
    #[must_use]
    pub fn overdue(&self, key: &NodeKey) -> bool {
        let Some(at) = self.index(key) else {
            return false;
        };
        let unfinished = self.unfinished.get(at.get()).copied().unwrap_or(false);
        let due = self.solved.latest(self.network.finish(at));
        unfinished && due.is_some_and(|due| due < self.today)
    }

    /// F1: a milestone's effective date: its actual date if reached, else its pin, else its
    /// derived due. None for other kinds, or when nothing gives one.
    #[must_use]
    pub fn effective_date(&self, key: &NodeKey) -> Option<EffectiveDate> {
        let at = self.index(key)?;
        if !self.milestone.get(at.get()).copied().unwrap_or(false) {
            return None;
        }
        let slot = self.network.finish(at);
        let effective = |day, origin| EffectiveDate {
            date: date(day),
            origin,
        };
        if let Some(actual) = self.seeds.actual(slot) {
            return Some(effective(actual.day, DateOrigin::Actual));
        }
        if let Some(pin) = self.seeds.pin(slot) {
            return Some(effective(pin.day, DateOrigin::Pin));
        }
        let due = self.solved.latest(slot)?;
        Some(effective(due, DateOrigin::Due))
    }

    /// F6: days the plan can no longer be met by at the node's start or finish, the larger.
    #[must_use]
    pub fn shortfall_days(&self, key: &NodeKey) -> Option<u32> {
        let found = self.shortfall_found(self.index(key)?)?;
        Some(found.shortfall_days.unsigned_abs())
    }

    fn shortfall_found(&self, at: NodeIndex) -> Option<Found> {
        let (start, finish) = (self.network.start(at), self.network.finish(at));
        let at_start = self.solved.shortfall(&self.network, &self.seeds, start);
        let at_finish = (finish != start)
            .then(|| self.solved.shortfall(&self.network, &self.seeds, finish))
            .flatten();
        match (at_start, at_finish) {
            (Some(early), Some(late)) if late.shortfall_days > early.shortfall_days => Some(late),
            (Some(early), _) => Some(early),
            (None, late) => late,
        }
    }

    /// Operations the solve spent, for the cost test.
    #[must_use]
    pub fn operation_count(&self) -> u64 {
        self.solved.cost.operations
    }

    /// The network's size: instant slots and constraints, for the cost test.
    #[must_use]
    pub fn network_size(&self) -> (usize, usize) {
        (self.network.slot_count(), self.network.constraints().len())
    }

    /// F3, F6, F7: the node's dates with the chain behind each bound and its shortfall with
    /// its resolution moves, read against `graph`, the graph these dates were derived from.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived.
    #[must_use]
    pub fn node(&self, graph: &Graph, key: &NodeKey) -> NodeDates {
        assert_eq!(
            graph.document().nodes.len(),
            self.keys.len(),
            "the derived graph"
        );
        let Some(at) = self.index(key) else {
            return NodeDates::default();
        };
        let explain = Explain {
            document: graph.document(),
            keys: &self.keys,
            network: &self.network,
            seeds: &self.seeds,
        };
        let (start, finish) = (self.network.start(at), self.network.finish(at));
        let bound = |day: Option<i32>, found: Found| {
            day.map(|day| Bound {
                date: date(day),
                chain: explain.chain(&found),
            })
        };
        let shortfall: Option<ShortChain> =
            self.shortfall_found(at).map(|found| explain.short(&found));
        NodeDates {
            earliest_start: bound(
                self.solved.earliest(start),
                self.solved.earliest_chain(&self.network, start),
            ),
            latest_start: bound(
                self.solved.latest(start),
                self.solved.latest_chain(&self.network, start),
            ),
            due: bound(
                self.solved.latest(finish),
                self.solved.latest_chain(&self.network, finish),
            ),
            slack_days: self.slack_days(key),
            shortfall,
            effective_date: self.effective_date(key),
        }
    }
}

/// Per node: in scope, not skipped, and non-terminal; a group (whose state is derived) while
/// any such non-group node lies beneath it.
fn unfinished(graph: &Graph, dependencies: &Dependencies, scope: &Scope) -> Vec<bool> {
    let document = graph.document();
    let mut unfinished = vec![false; dependencies.node_count()];
    for node in document.nodes.values() {
        let Some(at) = dependencies.node_index(&node.key) else {
            continue;
        };
        let open = scope.in_scope(at)
            && !scope.skipped(at)
            && !stored_state(document, node).is_terminal()
            && node.kind() != NodeKind::Group;
        if !open {
            continue;
        }
        if let Some(found) = unfinished.get_mut(at.get()) {
            *found = true;
        }
        let mut current = graph.tree().parent(&node.key);
        while let Some(parent) = current {
            let group = graph
                .node(parent)
                .is_some_and(|p| p.kind() == NodeKind::Group);
            let parent_at = dependencies.node_index(parent);
            let counts = parent_at.is_some_and(|p| scope.in_scope(p) && !scope.skipped(p));
            if let Some(found) = parent_at
                .filter(|_| group && counts)
                .and_then(|p| unfinished.get_mut(p.get()))
            {
                *found = true;
            }
            current = graph.tree().parent(parent);
        }
    }
    unfinished
}
