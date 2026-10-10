//! The unlocks simulations (PRD Priority, Unlocks; ARCHITECTURE, Read path: derive, pass 6):
//! for each node in the normalization set, which nodes' remaining hard dependencies completing
//! it would satisfy, holding current relevance and dates fixed. Each instant's rule mirrors
//! pass 5's: a start or entry is satisfied when its pruned gate waits are; a group's finish
//! likewise (derived group completion), and so is the finish of an `auto_reach` milestone due
//! today or earlier (it reads as reached once unblocked, F1); a skipped container's finish
//! when its kept work's are (D1a); any other open finish only by completing that node. The
//! counts of unsatisfied waits are taken once from pass 5's instants and each simulation
//! counts down from them, then puts back what it touched.

use cairn_schema::{NodeKind, State};

use super::super::Early;
use super::super::blocking::Blocking;
use super::super::dependencies::{Dependencies, EdgeClass, EdgeSet, Instant, NodeIndex, Point};
use super::super::skip::Skips;
use super::super::stored_state;
use crate::graph::Graph;

/// How an unsatisfied instant comes to be satisfied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// Already satisfied.
    Satisfied,
    /// An open node's finish: only completing the node satisfies it.
    Completion,
    /// Once this many unsatisfied pruned gate waits are satisfied.
    Waits(u32),
    /// A skipped container's finish: once this many unsatisfied kept nodes are.
    Kept(u32),
}

/// The counts every simulation starts from, and its scratch space.
struct Simulation<'a> {
    dependencies: &'a Dependencies,
    rules: Vec<Rule>,
    /// Per node: the skipped containers that list it as kept work.
    kept_by: Vec<Vec<NodeIndex>>,
    /// Per instant: waits satisfied so far in this simulation.
    counted: Vec<u32>,
    /// Per instant: satisfied in this simulation.
    reached: Vec<bool>,
    touched: Vec<usize>,
    operations: u64,
}

/// For each node, in index order, the nodes completing it would unblock when it is in the
/// normalization set (`normalized`), else none; and the instants, edges, and kept-work
/// entries read. `graph` is the one derived.
pub(super) fn simulate(
    graph: &Graph,
    early: &Early,
    blocking: &Blocking,
    normalized: &[bool],
) -> (Vec<Vec<NodeIndex>>, u64) {
    let mut simulation = Simulation::new(graph, &early.dependencies, &early.skips, blocking);
    let unblocks: Vec<Vec<NodeIndex>> = normalized
        .iter()
        .enumerate()
        .map(|(at, &ranked)| {
            if !ranked {
                return Vec::new();
            }
            let completed = NodeIndex::from_position(at);
            simulation
                .complete(completed)
                .into_iter()
                .filter(|instant| instant.point == Point::Start)
                .map(|instant| instant.node)
                .filter(|target| *target != completed)
                .filter(|target| normalized.get(target.get()) == Some(&true))
                .collect()
        })
        .collect();
    assert_eq!(unblocks.len(), early.dependencies.node_count());
    (unblocks, simulation.operations)
}

/// Every instant that completing `completed` satisfies, itself and what derived group
/// completion and reached milestones cascade to, holding current relevance and dates fixed.
pub(in crate::derive) fn cascade(
    graph: &Graph,
    dependencies: &Dependencies,
    skips: &Skips,
    blocking: &Blocking,
    completed: NodeIndex,
) -> Vec<Instant> {
    Simulation::new(graph, dependencies, skips, blocking).complete(completed)
}

impl<'a> Simulation<'a> {
    fn new(
        graph: &Graph,
        dependencies: &'a Dependencies,
        skips: &Skips,
        blocking: &Blocking,
    ) -> Self {
        let slots = dependencies.node_count() * Point::ALL.len();
        let mut rules = vec![Rule::Satisfied; slots];
        let mut kept_by = vec![Vec::new(); dependencies.node_count()];
        let unsatisfied = |instant: Instant| !blocking.instant(instant);
        for (at, key) in dependencies.keys().iter().enumerate() {
            let node = NodeIndex::from_position(at);
            for point in Point::ALL {
                let instant = Instant::new(node, point);
                if !unsatisfied(instant) {
                    continue;
                }
                let open_waits = dependencies
                    .waits(instant, EdgeSet::Pruned)
                    .filter(|edge| edge.class == EdgeClass::Gate)
                    .filter(|edge| unsatisfied(edge.requirement))
                    .count();
                let skipped = graph
                    .node(key)
                    .is_some_and(|stored| stored_state(graph.document(), stored) == State::Skipped)
                    || skips.skipped_by(key).is_some();
                let group = graph
                    .node(key)
                    .is_some_and(|stored| stored.kind() == NodeKind::Group);
                let rule = match point {
                    Point::Finish if skipped => {
                        let kept = skips.kept_work(key);
                        let open_kept = kept
                            .iter()
                            .filter_map(|kept| dependencies.node_index(kept))
                            .inspect(|kept| {
                                if let Some(listed) = kept_by.get_mut(kept.get()) {
                                    listed.push(node);
                                }
                            })
                            .filter(|kept| unsatisfied(Instant::new(*kept, Point::Finish)))
                            .count();
                        Rule::Kept(count(open_kept))
                    }
                    Point::Finish if !group && !blocking.reaches_when_unblocked(key) => {
                        Rule::Completion
                    }
                    Point::Start | Point::Finish | Point::Entry | Point::ConditionEntry => {
                        Rule::Waits(count(open_waits))
                    }
                };
                assert_ne!(
                    rule,
                    Rule::Waits(0),
                    "an unsatisfied instant waits on something"
                );
                assert_ne!(rule, Rule::Kept(0), "a pending skip waits on kept work");
                if let Some(slot) = rules.get_mut(instant.slot()) {
                    *slot = rule;
                }
            }
        }
        Simulation {
            dependencies,
            rules,
            kept_by,
            counted: vec![0; slots],
            reached: vec![false; slots],
            touched: Vec::new(),
            operations: 0,
        }
    }

    /// Completes `node` in a simulation: the instants it satisfies, itself first, in the order
    /// reached; then puts the scratch space back.
    fn complete(&mut self, node: NodeIndex) -> Vec<Instant> {
        let finish = Instant::new(node, Point::Finish);
        assert!(
            matches!(self.rule(finish), Rule::Completion | Rule::Waits(_)),
            "a node in the normalization set is open and not a group"
        );
        let mut reached = Vec::new();
        let mut queue = vec![finish];
        self.reach(finish);
        while let Some(instant) = queue.pop() {
            self.operations += 1;
            reached.push(instant);
            let dependents: Vec<Instant> = self
                .dependencies
                .waited_by(instant, EdgeSet::Pruned)
                .filter(|edge| edge.class == EdgeClass::Gate)
                .map(|edge| edge.dependent)
                .collect();
            let kept_by: Vec<Instant> = match instant.point {
                Point::Finish => self
                    .kept_by
                    .get(instant.node.get())
                    .map(|skips| {
                        skips
                            .iter()
                            .map(|&skip| Instant::new(skip, Point::Finish))
                            .collect()
                    })
                    .unwrap_or_default(),
                Point::Start | Point::Entry | Point::ConditionEntry => Vec::new(),
            };
            for dependent in dependents {
                self.operations += 1;
                if self.count_down(dependent, false) {
                    queue.push(dependent);
                }
            }
            for skip in kept_by {
                self.operations += 1;
                if self.count_down(skip, true) {
                    queue.push(skip);
                }
            }
        }
        for slot in std::mem::take(&mut self.touched) {
            if let Some(counted) = self.counted.get_mut(slot) {
                *counted = 0;
            }
            if let Some(reached) = self.reached.get_mut(slot) {
                *reached = false;
            }
        }
        reached
    }

    fn rule(&self, instant: Instant) -> Rule {
        self.rules
            .get(instant.slot())
            .copied()
            .unwrap_or(Rule::Satisfied)
    }

    fn reach(&mut self, instant: Instant) {
        if let Some(reached) = self.reached.get_mut(instant.slot()) {
            *reached = true;
        }
        self.touched.push(instant.slot());
    }

    /// One more of the instant's waits (a kept node's finish when `kept`) is satisfied; true
    /// when that was its last.
    fn count_down(&mut self, instant: Instant, kept: bool) -> bool {
        let slot = instant.slot();
        let needed = match (self.rule(instant), kept) {
            (Rule::Waits(needed), false) | (Rule::Kept(needed), true) => needed,
            (Rule::Satisfied | Rule::Completion | Rule::Waits(_) | Rule::Kept(_), _) => {
                return false;
            }
        };
        assert!(
            self.reached.get(slot) != Some(&true),
            "an instant is reached after its last wait, once"
        );
        let Some(counted) = self.counted.get_mut(slot) else {
            return false;
        };
        *counted += 1;
        let done = *counted == needed;
        self.touched.push(slot);
        if done {
            self.reach(instant);
        }
        done
    }
}

fn count(waits: usize) -> u32 {
    u32::try_from(waits).unwrap_or(u32::MAX)
}
