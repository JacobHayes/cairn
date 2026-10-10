//! Pass 5, auto-reach, blocking, and what can be acted on (PRD Containment, Gating, D1, D1a,
//! D2, D5, B6, F1; ARCHITECTURE, Read path: derive, pass 5). One sweep over the instants of
//! the effective dependency graph in a topological order decides, for each instant, whether
//! everything it waits on is satisfied; a node's finish is satisfied when the node satisfies
//! dependencies (D1, D1a). Auto-reach is decided inside the same sweep, at the milestone's
//! instant, so a milestone that reads as reached satisfies the instants after it and auto-reach
//! chains through any number of milestones (F1).
//!
//! What satisfies dependencies, at a node's finish:
//!
//! - a stored or effective skip: once every kept, in-scope node beneath it satisfies
//!   dependencies (D1a, "skipped, kept work pending" until then); a node with no kept work
//!   beneath it satisfies at once;
//! - any other terminal state (`done`, `decided`, `reached`);
//! - a group, which has no performed state: once its relevant or undecided children and its
//!   own requirements, openings, and conditions are satisfied, so an empty group waits for its
//!   gates (PRD Containment);
//! - a pending, relevant `auto_reach` milestone whose dependencies are satisfied and whose
//!   effective date is today or earlier: it reads as reached (F1);
//! - nothing else: open, undecided, and unstarted work holds its dependents.
//!
//! Every other instant (a start, a container's entries) is satisfied when each gate edge it
//! waits on in the pruned set is. A node's `deps_done` is its start: for a container, its
//! entries and its children; for anything else, its own and inherited requirements, openings,
//! and conditions. The order is a topological order of the full set's gate edges, which
//! validation holds acyclic, so kept work beneath a skipped container is decided before the
//! container even where the pruned set leaves out the containment between them (a
//! force-included node under a not-relevant container).
//!
//! Then, per node: blocked (in scope, not closed, `deps_done` false), actionable (D2:
//! relevant, not a group, not closed, `deps_done`), `needs_breakdown` (B10), and whether its
//! snooze holds (B6, from current state: a date snooze while today is before the date, a node
//! snooze while its target is in scope and does not satisfy dependencies), its own or a
//! container's over its subtree. The frontier is every actionable node; the acting frontier
//! leaves out snoozed nodes and `auto_reach` milestones whose date is still ahead; `stalled` (D5) is an empty acting frontier with
//! in-scope work still open, with what it waits on ([`stalled`]).
//!
//! Cost at `node_count_max` (2,000 nodes, about 110,000 edges, 8,000 instants): the
//! topological order is Kahn's algorithm over the full set's gate edges, O(instants + edges);
//! the sweep reads each pruned edge once from its dependent, O(edges), and each kept-work
//! entry once (at most 2,000 x 16, D1a); the per-node flags are O(nodes). Memory: one flag
//! per instant and a few per node, under 100 KiB, plus the frontier lists. Nothing is listed
//! per descendant: a node's blockers are read off its entry chains on demand
//! ([`super::Derived::blocked_by`]), at most about 1,300 edges per node.

mod stalled;

use std::cell::Cell;

use cairn_schema::limits::CONTAINMENT_DEPTH_MAX;
use cairn_schema::{
    Date, KeyRefs, Node, NodeKey, NodeKind, Payload, Relevance, SnoozeTarget, Stalled, State,
};

use super::dates::Dates;
use super::dependencies::{Dependencies, EdgeClass, EdgeSet, Instant, NodeIndex, Point};
use super::{Early, stored_state};
use crate::graph::Graph;

/// What pass 5 decides about one node.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Flags {
    /// Terminal, effectively skipped, auto-reached, or for a group done or skipped: nothing
    /// is left to do on it.
    closed: bool,
    /// Its start is satisfied: every dependency it waits on satisfies dependencies (D4's
    /// `deps_done`).
    deps_done: bool,
    /// In scope, not closed, and `deps_done` false.
    blocked: bool,
    /// D2.
    actionable: bool,
    /// F1: a pending `auto_reach` milestone that reads as reached.
    auto_reached: bool,
    /// F1: a pending, relevant `auto_reach` milestone whose effective date is today or
    /// earlier: it reads as reached once its dependencies are satisfied.
    reaches_when_unblocked: bool,
    /// B10: a placeholder in scope and open, with no children, not marked atomic.
    needs_breakdown: bool,
    /// B6: the node's own stored snooze, while it holds.
    snoozed_own: Option<SnoozeTarget>,
    /// B6: what holds the node: its own snooze, else the nearest container's snooze over
    /// the subtree it sits in.
    snoozed: Option<SnoozeTarget>,
    /// B6: the nearest container whose snooze holds over the node, which is open and in
    /// scope; set even when the node's own snooze holds as well.
    snoozed_via: Option<NodeKey>,
}

/// Pass 5's output: per node, its blocking flags; for the journey, the frontier, the acting
/// frontier, and the stalled diagnostic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Blocking {
    keys: Vec<NodeKey>,
    /// Per instant slot: satisfied.
    satisfied: Vec<bool>,
    /// Per node.
    flags: Vec<Flags>,
    frontier: Vec<NodeKey>,
    acting_frontier: Vec<NodeKey>,
    stalled: Option<Stalled>,
    /// The instants in a topological order of the full set's gate edges, which pass 6 reads
    /// backward.
    order: Vec<Instant>,
    /// Instants ordered and edges read, for the cost test.
    operations: u64,
}

/// The inputs the sweep reads, gathered once.
struct Sweep<'a> {
    graph: &'a Graph,
    early: &'a Early,
    dates: &'a Dates,
    today: Date,
    /// Edges read so far.
    operations: Cell<u64>,
}

impl Blocking {
    /// Pass 5 over passes 1, 2, and 4, at `today`.
    #[must_use]
    pub(crate) fn pass(graph: &Graph, early: &Early, dates: &Dates, today: Date) -> Self {
        let dependencies = &early.dependencies;
        let sweep = Sweep {
            graph,
            early,
            dates,
            today,
            operations: Cell::new(0),
        };
        let (order, ordering) = topological_order(dependencies);
        let mut satisfied = vec![false; dependencies.node_count() * Point::ALL.len()];
        let mut auto_reached = vec![false; dependencies.node_count()];
        for &instant in &order {
            let value = sweep.instant(instant, &satisfied, &mut auto_reached);
            if let Some(slot) = satisfied.get_mut(instant.slot()) {
                *slot = value;
            }
        }
        let mut flags: Vec<Flags> = dependencies
            .keys()
            .iter()
            .enumerate()
            .map(|(at, key)| {
                let reached = auto_reached.get(at).copied().unwrap_or(false);
                sweep.flags(key, NodeIndex::from_position(at), &satisfied, reached)
            })
            .collect();
        sweep.hold_subtrees(dependencies.keys(), &mut flags);
        let mut blocking = Blocking {
            keys: dependencies.keys().to_vec(),
            satisfied,
            flags,
            order,
            operations: ordering + sweep.operations.get(),
            ..Blocking::default()
        };
        blocking.fronts(&sweep);
        blocking.stalled = stalled::diagnose(&blocking, &sweep);
        blocking.assert_frontier(&sweep);
        blocking
    }

    /// The frontier and the acting frontier, in key order (2.5 ranks them).
    fn fronts(&mut self, sweep: &Sweep<'_>) {
        for (key, flags) in self.keys.iter().zip(&self.flags) {
            if !flags.actionable {
                continue;
            }
            self.frontier.push(key.clone());
            if flags.snoozed.is_none() && sweep.reach_ahead(key).is_none() {
                self.acting_frontier.push(key.clone());
            }
        }
    }

    /// PRD Frontier: by construction it has no actionable descendants, since a parent waits
    /// on each child it holds in scope. Checked from below: no actionable node has an
    /// actionable ancestor, at most `containment_depth_max` steps each. A closed or
    /// not-relevant container ends the walk: a terminal container satisfies its dependents
    /// whatever was reopened beneath it (D1; it is stale instead), and nothing above a
    /// not-relevant container waits on what a force include keeps in scope beneath it (2.2's
    /// pruned set).
    fn assert_frontier(&self, sweep: &Sweep<'_>) {
        assert!(self.acting_frontier.len() <= self.frontier.len());
        let tree = sweep.graph.tree();
        for key in &self.frontier {
            let mut ancestor = tree.parent(key);
            let open =
                |above: &&NodeKey| sweep.early.relevance.in_scope(above) && !self.closed(above);
            while let Some(above) = ancestor.filter(open) {
                assert!(
                    !self.actionable(above),
                    "a frontier node {above} has an actionable descendant {key}"
                );
                ancestor = tree.parent(above);
            }
        }
    }

    fn flags_of(&self, key: &NodeKey) -> Option<&Flags> {
        let at = self.keys.binary_search(key).ok()?;
        self.flags.get(at)
    }

    fn index(&self, key: &NodeKey) -> Option<NodeIndex> {
        self.keys
            .binary_search(key)
            .ok()
            .map(NodeIndex::from_position)
    }

    /// D1, D1a: the node satisfies dependencies: what its dependents wait for.
    #[must_use]
    pub fn satisfies(&self, key: &NodeKey) -> bool {
        self.index(key)
            .is_some_and(|at| self.instant(Instant::new(at, Point::Finish)))
    }

    /// Whether everything the instant waits on is satisfied (a finish: the node satisfies
    /// dependencies).
    #[must_use]
    pub(crate) fn instant(&self, instant: Instant) -> bool {
        self.satisfied.get(instant.slot()).copied().unwrap_or(false)
    }

    /// D4's `deps_done`: every relevant or undecided hard dependency of the node, inherited
    /// and containment ones included, satisfies dependencies.
    #[must_use]
    pub fn deps_done(&self, key: &NodeKey) -> bool {
        self.flags_of(key).is_some_and(|flags| flags.deps_done)
    }

    /// Gating, Blocked: in scope and open, with a dependency that does not satisfy
    /// dependencies.
    #[must_use]
    pub fn blocked(&self, key: &NodeKey) -> bool {
        self.flags_of(key).is_some_and(|flags| flags.blocked)
    }

    /// D2: relevant, not blocked, open, and not a group.
    #[must_use]
    pub fn actionable(&self, key: &NodeKey) -> bool {
        self.flags_of(key).is_some_and(|flags| flags.actionable)
    }

    /// Nothing is left to do on the node: terminal, effectively skipped, auto-reached, or a
    /// group that is done or skipped (D5: not "effectively unfinished").
    #[must_use]
    pub fn closed(&self, key: &NodeKey) -> bool {
        self.flags_of(key).is_some_and(|flags| flags.closed)
    }

    /// F1: a pending `auto_reach` milestone that reads as reached.
    #[must_use]
    pub fn auto_reached(&self, key: &NodeKey) -> bool {
        self.flags_of(key).is_some_and(|flags| flags.auto_reached)
    }

    /// F1: a pending, relevant `auto_reach` milestone whose effective date is today or
    /// earlier, which reads as reached once its dependencies are satisfied (pass 6 completes
    /// it in the unlocks simulations).
    #[must_use]
    pub(crate) fn reaches_when_unblocked(&self, key: &NodeKey) -> bool {
        self.flags_of(key)
            .is_some_and(|flags| flags.reaches_when_unblocked)
    }

    /// B10: a placeholder in scope and open, with no children, not marked atomic.
    #[must_use]
    pub fn needs_breakdown(&self, key: &NodeKey) -> bool {
        self.flags_of(key)
            .is_some_and(|flags| flags.needs_breakdown)
    }

    /// B6: what holds the node off the acting frontier: its own snooze while it holds, else
    /// the snooze of the container it is held through.
    #[must_use]
    pub fn snoozed(&self, key: &NodeKey) -> Option<&SnoozeTarget> {
        self.flags_of(key).and_then(|flags| flags.snoozed.as_ref())
    }

    /// B6: the node's own snooze, while it holds, whatever a container holds over it.
    #[must_use]
    pub fn snoozed_own(&self, key: &NodeKey) -> Option<&SnoozeTarget> {
        self.flags_of(key)
            .and_then(|flags| flags.snoozed_own.as_ref())
    }

    /// B6: the nearest container whose snooze holds over the node (open and in scope), the
    /// reason a descendant names.
    #[must_use]
    pub fn snoozed_via(&self, key: &NodeKey) -> Option<&NodeKey> {
        self.flags_of(key)
            .and_then(|flags| flags.snoozed_via.as_ref())
    }

    /// PRD Frontier: every actionable node, in key order.
    #[must_use]
    pub fn frontier(&self) -> &[NodeKey] {
        &self.frontier
    }

    /// PRD Acting frontier: the frontier without snoozed nodes and `auto_reach` milestones
    /// whose date is still ahead, in key order.
    #[must_use]
    pub fn acting_frontier(&self) -> &[NodeKey] {
        &self.acting_frontier
    }

    /// D5: the stalled diagnostic, when the journey is stalled.
    #[must_use]
    pub fn stalled(&self) -> Option<&Stalled> {
        self.stalled.as_ref()
    }

    /// Every node's key, in index order.
    #[must_use]
    pub(crate) fn keys(&self) -> &[NodeKey] {
        &self.keys
    }

    /// The instants in a topological order of the full set's gate edges: each after every
    /// instant it waits on in either set.
    #[must_use]
    pub(crate) fn order(&self) -> &[Instant] {
        &self.order
    }

    /// Instants ordered and edges read by the pass, for the cost test.
    #[must_use]
    pub fn operation_count(&self) -> u64 {
        self.operations
    }
}

impl Sweep<'_> {
    fn dependencies(&self) -> &Dependencies {
        &self.early.dependencies
    }

    fn node(&self, at: NodeIndex) -> Option<&Node<KeyRefs>> {
        self.dependencies()
            .key(at)
            .and_then(|key| self.graph.node(key))
    }

    /// Every gate the instant waits on in the pruned set is satisfied.
    fn waits_satisfied(&self, instant: Instant, satisfied: &[bool]) -> bool {
        self.dependencies()
            .waits(instant, EdgeSet::Pruned)
            .inspect(|_| self.operations.set(self.operations.get() + 1))
            .filter(|edge| edge.class == EdgeClass::Gate)
            .all(|edge| satisfied.get(edge.requirement.slot()) == Some(&true))
    }

    /// One instant's value, every instant before it in the order decided.
    fn instant(&self, instant: Instant, satisfied: &[bool], auto_reached: &mut [bool]) -> bool {
        if instant.point != Point::Finish {
            return self.waits_satisfied(instant, satisfied);
        }
        let Some(node) = self.node(instant.node) else {
            return false;
        };
        let document = self.graph.document();
        let key = &node.key;
        let state = stored_state(document, node);
        let skips = &self.early.skips;
        if state == State::Skipped || skips.skipped_by(key).is_some() {
            // D1a: a skip satisfies once the kept work beneath it does.
            return skips.kept_work(key).iter().all(|kept| {
                self.dependencies().node_index(kept).is_some_and(|at| {
                    let finish = Instant::new(at, Point::Finish);
                    satisfied.get(finish.slot()) == Some(&true)
                })
            });
        }
        if state.is_terminal() {
            return true;
        }
        let waits = self.waits_satisfied(instant, satisfied);
        if node.kind() == NodeKind::Group {
            return waits;
        }
        let reached = waits && self.reads_as_reached(node);
        if let Some(flag) = auto_reached.get_mut(instant.node.get()) {
            *flag = reached;
        }
        reached
    }

    /// F1: a pending, relevant `auto_reach` milestone whose effective date is today or
    /// earlier (its dependencies are checked by the caller).
    fn reads_as_reached(&self, node: &Node<KeyRefs>) -> bool {
        let Payload::Milestone(milestone) = &node.payload else {
            return false;
        };
        let pending = stored_state(self.graph.document(), node) == State::Pending;
        let relevant = self.early.relevance.value(&node.key) == Relevance::Relevant;
        let due = self.dates.effective_date(&node.key);
        milestone.auto_reach && pending && relevant && due.is_some_and(|due| due.date <= self.today)
    }

    /// PRD Acting frontier: an `auto_reach` milestone's effective date, while it is ahead.
    fn reach_ahead(&self, key: &NodeKey) -> Option<Date> {
        let node = self.graph.node(key)?;
        let Payload::Milestone(milestone) = &node.payload else {
            return None;
        };
        let date = self.dates.effective_date(key)?.date;
        (milestone.auto_reach && date > self.today).then_some(date)
    }

    /// One node's flags, from the sweep's instants.
    fn flags(&self, key: &NodeKey, at: NodeIndex, satisfied: &[bool], reached: bool) -> Flags {
        let Some(node) = self.graph.node(key) else {
            return Flags::default();
        };
        let document = self.graph.document();
        let value = |point| satisfied.get(Instant::new(at, point).slot()) == Some(&true);
        let state = stored_state(document, node);
        let group = node.kind() == NodeKind::Group;
        let skipped = state == State::Skipped || self.early.skips.skipped_by(key).is_some();
        let closed = skipped || state.is_terminal() || reached || (group && value(Point::Finish));
        let relevance = self.early.relevance.value(key);
        let in_scope = relevance != Relevance::NotRelevant;
        let deps_done = value(Point::Start);
        let open = in_scope && !closed;
        let own = self.holding(key, satisfied);
        Flags {
            closed,
            deps_done,
            blocked: open && !deps_done,
            actionable: open && deps_done && !group && relevance == Relevance::Relevant,
            auto_reached: reached,
            reaches_when_unblocked: self.reads_as_reached(node),
            needs_breakdown: open && self.unexpanded(node),
            snoozed: own.clone(),
            snoozed_own: own,
            snoozed_via: None,
        }
    }

    /// B6: a container's snooze holds over its subtree. Each open node in scope beneath a
    /// container whose snooze holds names the nearest such container, and is held by that
    /// snooze unless its own holds. O(nodes x depth), the walk bounded by the depth limit.
    fn hold_subtrees(&self, keys: &[NodeKey], flags: &mut [Flags]) {
        let tree = self.graph.tree();
        let holders: Vec<Option<SnoozeTarget>> = flags
            .iter()
            .zip(keys)
            .map(|(held, key)| {
                let open = self.early.relevance.in_scope(key) && !held.closed;
                held.snoozed_own.clone().filter(|_| open)
            })
            .collect();
        if holders.iter().all(Option::is_none) {
            return;
        }
        let holder = |key: &NodeKey| -> Option<&SnoozeTarget> {
            let at = keys.binary_search(key).ok()?;
            holders.get(at)?.as_ref()
        };
        for (at, key) in keys.iter().enumerate() {
            let open = self.early.relevance.in_scope(key)
                && flags.get(at).is_some_and(|held| !held.closed);
            if !open {
                continue;
            }
            let mut ancestor = tree.parent(key);
            let mut steps = 0_u32;
            while let Some(above) = ancestor {
                if let Some(target) = holder(above) {
                    if let Some(held) = flags.get_mut(at) {
                        held.snoozed_via = Some(above.clone());
                        if held.snoozed.is_none() {
                            held.snoozed = Some(target.clone());
                        }
                    }
                    break;
                }
                steps += 1;
                assert!(steps <= CONTAINMENT_DEPTH_MAX + 1, "containment is bounded");
                ancestor = tree.parent(above);
            }
        }
    }

    /// B10: a placeholder with no children that is not marked atomic.
    fn unexpanded(&self, node: &Node<KeyRefs>) -> bool {
        let placeholder = match &node.payload {
            Payload::Deliverable(deliverable) => deliverable.placeholder,
            Payload::Action(action) => action.placeholder,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
        };
        let atomic = self
            .graph
            .document()
            .state
            .nodes
            .get(&node.key)
            .is_some_and(|stored| stored.atomic);
        placeholder && !atomic && !self.graph.tree().is_container(&node.key)
    }

    /// B6: the node's stored snooze, when it holds from current state.
    fn holding(&self, key: &NodeKey, satisfied: &[bool]) -> Option<SnoozeTarget> {
        let until = self.graph.document().state.snoozes.get(key)?;
        let holds = match until {
            SnoozeTarget::Date(date) => self.today < *date,
            SnoozeTarget::Node(target) => {
                let in_scope = self.early.relevance.in_scope(target);
                let done = self.dependencies().node_index(target).is_some_and(|at| {
                    satisfied.get(Instant::new(at, Point::Finish).slot()) == Some(&true)
                });
                in_scope && !done
            }
        };
        holds.then(|| until.clone())
    }
}

/// The instants in a topological order of the full set's gate edges (Kahn's algorithm), each
/// after every instant it waits on, with the instants and edges it read. Validation holds
/// those edges acyclic.
fn topological_order(dependencies: &Dependencies) -> (Vec<Instant>, u64) {
    let slots = dependencies.node_count() * Point::ALL.len();
    let instants: Vec<Instant> = (0..dependencies.node_count())
        .flat_map(|at| Point::ALL.map(|point| Instant::new(NodeIndex::from_position(at), point)))
        .collect();
    let gate_waits = |instant: Instant| {
        dependencies
            .waits(instant, EdgeSet::Full)
            .filter(|edge| edge.class == EdgeClass::Gate)
            .count()
    };
    let mut waiting: Vec<usize> = vec![0; slots];
    for instant in &instants {
        if let Some(count) = waiting.get_mut(instant.slot()) {
            *count = gate_waits(*instant);
        }
    }
    let mut ready: Vec<Instant> = instants
        .iter()
        .rev()
        .filter(|instant| waiting.get(instant.slot()) == Some(&0))
        .copied()
        .collect();
    let mut order = Vec::with_capacity(slots);
    while let Some(next) = ready.pop() {
        order.push(next);
        let dependents = dependencies
            .waited_by(next, EdgeSet::Full)
            .filter(|edge| edge.class == EdgeClass::Gate);
        for edge in dependents {
            if let Some(count) = waiting.get_mut(edge.dependent.slot()) {
                *count -= 1;
                if *count == 0 {
                    ready.push(edge.dependent);
                }
            }
        }
    }
    assert_eq!(
        order.len(),
        slots,
        "gate edges are acyclic in a valid graph"
    );
    // Each instant once, and each edge into it read twice: counted, then released.
    let edges = dependencies.edges(EdgeSet::Full).count();
    let operations = u64::try_from(slots + 2 * edges).unwrap_or(u64::MAX);
    (order, operations)
}
