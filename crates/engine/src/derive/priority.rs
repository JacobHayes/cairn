//! Pass 6, gravity and leverage (PRD Priority; ARCHITECTURE, Read path: derive, pass 6), over
//! the pruned gate edges of the effective dependency graph, with the undecided discount and
//! the owner factor from the rank constants.
//!
//! Gravity: a node's downstream set is every node with an instant reachable from its finish
//! along pruned gate edges: explicit requirements, inherited and condition gates, stage
//! openings, and containment (a parent waits on each child, so a child's downstream holds its
//! parent and everything after it). Date-only openings carry no gravity. The set is a union,
//! so a node reached along two paths counts once. Pruning leaves out not-relevant branches;
//! terminal nodes are traversed (their edges stay) but count nothing, and nor do not-relevant
//! ones a requirement entry passes through. What a node counts, its own weight included, is
//! its counted weight: its weight times the undecided discount when it is undecided, and
//! nothing when it is closed (terminal, effectively skipped, auto-reached, or a group whose
//! work is done) or not relevant, so gravity is what still rides on a node, and an upstream
//! node's set holds a downstream node's set and the node itself: gravity never decreases
//! upstream.
//!
//! Leverage: for each node in the rank normalization set (relevant or undecided, open, not a
//! group), completing it is simulated over pass 5's instants: its finish is satisfied, and
//! each instant whose last unsatisfied wait that was is satisfied in turn, cascading through
//! derived group completion, through `auto_reach` milestones due today or earlier (which read
//! as reached once unblocked, F1), and through skipped containers whose kept work it finishes
//! (D1a). Every other open node's finish stays unsatisfied: completing one node completes no
//! other. The nodes whose start (`deps_done`) is satisfied by that are what it unblocks; each
//! counts its weight, the undecided discount, and the owner factor once.
//!
//! Cost at `node_count_max` (2,000 nodes, 8,000 instants, about 110,000 edges): gravity keeps
//! one bit per node for each instant, 8,000 x 32 words (2 MiB), filled in one reverse
//! topological sweep that unions each edge's dependent into its requirement (110,000 x 32
//! word operations); each node's finish row is kept (512 KiB) and summed once, 2,000 x 2,000
//! bits; each container's peak gravity is one step per node and child (at most 4,000),
//! uncounted. Leverage simulates every node in the normalization set. An instant a simulation
//! satisfies has every unsatisfied wait behind it lead back to the completed node's finish, so
//! the simulations of nodes that only completion satisfies never satisfy the same instant, and
//! together they read each instant, each pruned edge, and each kept-work entry (at most
//! 2,000 x 16) at most once. The only overlap is an `auto_reach` milestone due today but still
//! blocked: it is ranked, and its own simulation repeats the part of a cascade behind it. With
//! `m` such milestones the simulations read at most (1 + m) x (instants + edges + kept work);
//! `m` counts the overdue, gated `auto_reach` milestones: few in practice, and in a
//! constructed worst case (every milestone overdue, chained ahead of the rest of the graph)
//! about 700 x 180,000 reads. Memory: two counters and a flag per instant, and the
//! unblocked nodes, one entry per node per simulation that reaches it.

pub(super) mod leverage;

use std::collections::BTreeSet;

use cairn_schema::limits::{NODE_COUNT_MAX, WEIGHT_MAX};
use cairn_schema::{
    Contribution, EntityKey, KindKey, NodeKey, NodeKind, OwnerFactor, RankConstants, Relevance,
    Score, UndecidedDiscount,
};

use super::Early;
use super::blocking::Blocking;
use super::dependencies::{EdgeClass, EdgeSet, Instant, NodeIndex, Point};
use super::participation::Participation;
use crate::graph::{Graph, Tree};

/// Thousandths in one: the unit the discount and the factor are counted in.
const THOUSAND: u64 = 1_000;

// Every sum of terms fits a score that reads back exactly (Score::MAX): `node_count_max`
// targets at `weight_max`, undiscounted, at the largest owner factor.
const _: () = assert!(
    NODE_COUNT_MAX as u64 * WEIGHT_MAX as u64 * THOUSAND * (WEIGHT_MAX as u64 * THOUSAND)
        <= Score::MAX.millionths()
);

/// Pass 6's output: per node, its gravity with the downstream set it sums, its largest child
/// gravity, and, for the normalization set, what completing it would unblock and its leverage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Priority {
    keys: Vec<NodeKey>,
    /// Words per downstream row: a bit per node in key order.
    words: usize,
    /// Per node: its weight.
    weights: Vec<u32>,
    /// Per node: undecided (Gating), so discounted.
    undecided: Vec<bool>,
    /// Per node: relevant or undecided.
    in_scope: Vec<bool>,
    /// Per node: in the rank normalization set (Priority).
    normalized: Vec<bool>,
    /// Per node: what it counts toward gravity, in millionths.
    counted: Vec<u64>,
    /// Per node: the nodes downstream of its finish, `words` words each.
    downstream: Vec<u64>,
    gravity: Vec<Score>,
    max_child_gravity: Vec<Option<Score>>,
    /// Per node: the largest gravity among its open, in-scope descendants at any depth, and
    /// the position of the descendant that has it.
    peak_gravity: Vec<Option<(usize, Score)>>,
    /// Per node: in scope and not closed.
    open: Vec<bool>,
    /// Per node: its owners (E2), which the owner factor compares.
    owners: Vec<BTreeSet<EntityKey>>,
    /// Per node in the normalization set: the nodes completing it would unblock.
    unblocks: Vec<Vec<NodeIndex>>,
    leverage: Vec<Score>,
    discount: UndecidedDiscount,
    factor: OwnerFactor,
    /// Word operations of the gravity sweep and instants and edges the simulations read.
    operations: u64,
}

impl Priority {
    /// Pass 6 over passes 1 to 5, with the rank constants' discount and owner factor.
    #[must_use]
    pub(crate) fn pass(
        graph: &Graph,
        early: &Early,
        participation: &Participation,
        blocking: &Blocking,
        constants: &RankConstants,
    ) -> Self {
        let dependencies = &early.dependencies;
        let keys = dependencies.keys().to_vec();
        let mut priority = Priority {
            keys: Vec::new(),
            words: keys.len().div_ceil(64),
            weights: Vec::new(),
            undecided: Vec::new(),
            in_scope: Vec::new(),
            normalized: Vec::new(),
            counted: Vec::new(),
            downstream: Vec::new(),
            gravity: Vec::new(),
            max_child_gravity: Vec::new(),
            peak_gravity: Vec::new(),
            open: Vec::new(),
            owners: Vec::new(),
            unblocks: Vec::new(),
            leverage: Vec::new(),
            discount: constants.undecided_discount,
            factor: constants.other_owner_factor,
            operations: 0,
        };
        for key in &keys {
            priority.describe(graph, early, participation, blocking, key);
        }
        priority.keys = keys;
        priority.sweep_gravity(early, blocking);
        priority.sum_gravity(graph);
        let (unblocks, operations) =
            leverage::simulate(graph, early, blocking, &priority.normalized);
        priority.unblocks = unblocks;
        priority.operations += operations;
        priority.leverage = (0..priority.keys.len())
            .map(|at| priority.leverage_at(at, priority.owners.get(at)).0)
            .collect();
        priority
    }

    /// One node's inputs: weight, relevance, whether it counts and is ranked, and owners.
    fn describe(
        &mut self,
        graph: &Graph,
        early: &Early,
        participation: &Participation,
        blocking: &Blocking,
        key: &NodeKey,
    ) {
        let node = graph.node(key);
        let weight = node.map_or(0, |node| node.effective_weight().get());
        let relevance = early.relevance.value(key);
        let group = node.is_some_and(|node| node.kind() == NodeKind::Group);
        let in_scope = relevance != Relevance::NotRelevant;
        let open = in_scope && !blocking.closed(key);
        let undecided = relevance == Relevance::Undecided;
        self.weights.push(weight);
        self.open.push(open);
        self.in_scope.push(in_scope);
        self.undecided.push(undecided);
        self.normalized.push(open && !group);
        let counted = if open {
            u64::from(weight) * self.discounted(undecided) * THOUSAND
        } else {
            0
        };
        self.counted.push(counted);
        self.owners
            .push(participation.entities(key, &KindKey::owner()).clone());
    }

    /// The discount's thousandths when undecided, else a thousand.
    fn discounted(&self, undecided: bool) -> u64 {
        if undecided {
            u64::from(self.discount.get())
        } else {
            THOUSAND
        }
    }

    /// Fills each node's downstream row: one sweep over the instants in reverse topological
    /// order, each taking every pruned gate dependent's node and that dependent's own set.
    fn sweep_gravity(&mut self, early: &Early, blocking: &Blocking) {
        let dependencies = &early.dependencies;
        let mut reach = BitRows::new(dependencies.node_count() * Point::ALL.len(), self.words);
        let mut operations = 0_u64;
        for &instant in blocking.order().iter().rev() {
            let dependents = dependencies
                .waited_by(instant, EdgeSet::Pruned)
                .filter(|edge| edge.class == EdgeClass::Gate);
            for edge in dependents {
                assert_ne!(edge.dependent, instant, "gate edges are acyclic");
                reach.set(instant.slot(), edge.dependent.node.get());
                reach.union(instant.slot(), edge.dependent.slot());
                operations += u64::try_from(self.words).unwrap_or(u64::MAX);
            }
        }
        self.downstream = (0..dependencies.node_count())
            .flat_map(|at| {
                let finish = Instant::new(NodeIndex::from_position(at), Point::Finish);
                reach.row(finish.slot()).to_vec()
            })
            .collect();
        self.operations += operations;
    }

    /// Each node's gravity (its counted weight and its downstream set's) and its largest
    /// child gravity.
    fn sum_gravity(&mut self, graph: &Graph) {
        self.gravity = (0..self.keys.len())
            .map(|at| {
                let own = self.counted.get(at).copied().unwrap_or(0);
                let rest: u64 = self
                    .downstream_of(at)
                    .map(|other| self.counted_at(other))
                    .sum();
                assert!(
                    !self.downstream_of(at).any(|other| other == at),
                    "a node is not downstream of itself"
                );
                let total = own + rest;
                assert!(total <= Score::MAX.millionths(), "within the limits");
                Score::from_millionths(total)
            })
            .collect();
        let tree = graph.tree();
        self.max_child_gravity = self
            .keys
            .iter()
            .map(|key| {
                let children = tree.children(key);
                (!children.is_empty()).then(|| {
                    children
                        .iter()
                        .filter_map(|child| self.position(child))
                        .filter(|&child| self.in_scope_at(child))
                        .map(|child| self.gravity_at(child))
                        .max()
                        .unwrap_or_default()
                })
            })
            .collect();
        self.peak_gravity = self.sweep_peaks(tree);
    }

    /// Each container's peak: the largest gravity among its open, in-scope descendants at any
    /// depth; among equals a descendant beats its own ancestor, and otherwise the first in
    /// tree order wins. Children before parents over the tree's own order, with an explicit
    /// stack (PRACTICES, No recursion): a node's peak is the best of each child's peak and
    /// the child itself, a child ahead of its own peak only when strictly larger (a child's
    /// gravity holds its parent's, so equal reads deeper). One step per node and child.
    fn sweep_peaks(&self, tree: &Tree) -> Vec<Option<(usize, Score)>> {
        let mut order: Vec<&NodeKey> = Vec::with_capacity(self.keys.len());
        let mut stack: Vec<&NodeKey> = tree.roots().iter().rev().collect();
        while let Some(key) = stack.pop() {
            order.push(key);
            stack.extend(tree.children(key).iter().rev());
        }
        assert_eq!(order.len(), self.keys.len(), "the tree holds every node");
        let mut peaks: Vec<Option<(usize, Score)>> = vec![None; self.keys.len()];
        for key in order.into_iter().rev() {
            let mut best: Option<(usize, Score)> = None;
            for child in tree.children(key) {
                let Some(at) = self.position(child) else {
                    continue;
                };
                let deeper = peaks.get(at).copied().flatten();
                let own =
                    (self.in_scope_at(at) && self.open_at(at)).then(|| (at, self.gravity_at(at)));
                let found = match (deeper, own) {
                    (Some(deeper), Some(own)) if own.1 > deeper.1 => Some(own),
                    (Some(deeper), _) => Some(deeper),
                    (None, own) => own,
                };
                if let Some(found) = found
                    && best.is_none_or(|(_, gravity)| found.1 > gravity)
                {
                    best = Some(found);
                }
            }
            if let Some(slot) = self.position(key).and_then(|at| peaks.get_mut(at)) {
                *slot = best;
            }
        }
        peaks
    }

    fn open_at(&self, at: usize) -> bool {
        self.open.get(at).copied().unwrap_or(false)
    }

    fn position(&self, key: &NodeKey) -> Option<usize> {
        self.keys.binary_search(key).ok()
    }

    fn counted_at(&self, at: usize) -> u64 {
        self.counted.get(at).copied().unwrap_or(0)
    }

    fn gravity_at(&self, at: usize) -> Score {
        self.gravity.get(at).copied().unwrap_or_default()
    }

    fn in_scope_at(&self, at: usize) -> bool {
        self.in_scope.get(at).copied().unwrap_or(false)
    }

    /// The nodes downstream of the node at `at`, in key order.
    fn downstream_of(&self, at: usize) -> impl Iterator<Item = usize> + '_ {
        let from = at * self.words;
        let row = self.downstream.get(from..from + self.words).unwrap_or(&[]);
        row.iter().enumerate().flat_map(|(word_at, &word)| {
            (0..64_usize)
                .filter(move |bit| word & (1_u64 << bit) != 0)
                .map(move |bit| word_at * 64 + bit)
        })
    }

    /// One unblocked target's term: its weight, discounted when undecided, times the owner
    /// factor when its owners and `acting` differ.
    fn term(&self, target: usize, acting: Option<&BTreeSet<EntityKey>>) -> (u64, bool) {
        let weight = u64::from(self.weights.get(target).copied().unwrap_or(0));
        let undecided = self.undecided.get(target).copied().unwrap_or(false);
        let other = !same_owner(acting, self.owners.get(target));
        let factor = if other {
            u64::from(self.factor.get())
        } else {
            THOUSAND
        };
        (weight * self.discounted(undecided) * factor, other)
    }

    /// The leverage of the node at `at` with the owner factor relative to `acting`, and its
    /// contributions, largest first, then by key.
    fn leverage_at(
        &self,
        at: usize,
        acting: Option<&BTreeSet<EntityKey>>,
    ) -> (Score, Vec<Contribution>) {
        let targets = self.unblocks.get(at).map_or(&[][..], Vec::as_slice);
        let mut contributions: Vec<Contribution> = targets
            .iter()
            .filter_map(|target| {
                let (score, other_owner) = self.term(target.get(), acting);
                let node = self.keys.get(target.get())?.clone();
                Some(Contribution {
                    node,
                    score: Score::from_millionths(score),
                    other_owner,
                })
            })
            .collect();
        assert_eq!(contributions.len(), targets.len(), "every target is a node");
        contributions.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.node.cmp(&b.node)));
        let total: u64 = contributions.iter().map(|c| c.score.millionths()).sum();
        assert!(total <= Score::MAX.millionths(), "within the limits");
        (Score::from_millionths(total), contributions)
    }
}

/// The owner factor's test (Priority): the target's owners and the acting owners share an
/// entity, or neither has any. A target no one owns differs from an owned node, and an owned
/// target from an unowned node.
fn same_owner(acting: Option<&BTreeSet<EntityKey>>, target: Option<&BTreeSet<EntityKey>>) -> bool {
    let empty = BTreeSet::new();
    let acting = acting.unwrap_or(&empty);
    let target = target.unwrap_or(&empty);
    if acting.is_empty() || target.is_empty() {
        return acting.is_empty() && target.is_empty();
    }
    !acting.is_disjoint(target)
}

impl Priority {
    fn at(&self, key: &NodeKey) -> Option<usize> {
        self.position(key)
    }

    /// Priority, Gravity: the node's counted weight plus that of every node downstream of
    /// it, each once.
    #[must_use]
    pub fn gravity(&self, key: &NodeKey) -> Score {
        self.at(key)
            .map_or_else(Score::default, |at| self.gravity_at(at))
    }

    /// Priority, Gravity: the nodes downstream that contribute to the node's gravity, with
    /// what each adds, largest first, then by key. Listed on demand from the node's
    /// downstream set.
    #[must_use]
    pub fn gravity_from(&self, key: &NodeKey) -> Vec<Contribution> {
        let Some(at) = self.at(key) else {
            return Vec::new();
        };
        let mut contributions: Vec<Contribution> = self
            .downstream_of(at)
            .filter(|&other| self.counted_at(other) > 0)
            .filter_map(|other| {
                Some(Contribution {
                    node: self.keys.get(other)?.clone(),
                    score: Score::from_millionths(self.counted_at(other)),
                    other_owner: false,
                })
            })
            .collect();
        contributions.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.node.cmp(&b.node)));
        contributions
    }

    /// C7: every node downstream of the node through pruned gate edges, terminal ones
    /// included, in key order: the set gravity sums.
    #[must_use]
    pub fn downstream(&self, key: &NodeKey) -> Vec<&NodeKey> {
        let Some(at) = self.at(key) else {
            return Vec::new();
        };
        self.downstream_of(at)
            .filter_map(|other| self.keys.get(other))
            .collect()
    }

    /// Priority: a container's largest in-scope child gravity; none for a node with no
    /// children.
    #[must_use]
    pub fn max_child_gravity(&self, key: &NodeKey) -> Option<Score> {
        self.at(key)
            .and_then(|at| self.max_child_gravity.get(at).copied().flatten())
    }

    /// Priority: a container's peak gravity: the largest gravity among its open, in-scope
    /// descendants at any depth, with the descendant that has it (among equals a descendant
    /// beats its own ancestor, otherwise the first in tree order); none for a node with no
    /// such descendant.
    #[must_use]
    pub fn peak_gravity(&self, key: &NodeKey) -> Option<(&NodeKey, Score)> {
        let (peak, gravity) = self
            .at(key)
            .and_then(|at| self.peak_gravity.get(at).copied().flatten())?;
        Some((self.keys.get(peak)?, gravity))
    }

    /// Priority: relevant or undecided, open, and not a group: the nodes rank normalizes over
    /// and ranks, and the nodes leverage is computed for.
    #[must_use]
    pub fn in_normalization_set(&self, key: &NodeKey) -> bool {
        self.at(key)
            .is_some_and(|at| self.normalized.get(at).copied().unwrap_or(false))
    }

    /// Priority, Leverage: what completing the node frees up, with the owner factor relative
    /// to the node's own owners (the global value); zero outside the normalization set.
    #[must_use]
    pub fn leverage(&self, key: &NodeKey) -> Score {
        self.at(key)
            .and_then(|at| self.leverage.get(at).copied())
            .unwrap_or_default()
    }

    /// Priority, Leverage: the nodes completing this would unblock, each with its term and
    /// whether someone other than the node's owners owns it, largest first.
    #[must_use]
    pub fn leverage_from(&self, key: &NodeKey) -> Vec<Contribution> {
        self.at(key)
            .map(|at| self.leverage_at(at, self.owners.get(at)).1)
            .unwrap_or_default()
    }

    /// Priority, "prioritize for me": the node's leverage with the owner factor relative to
    /// the viewer's entities rather than the node's owners.
    #[must_use]
    pub fn leverage_for(&self, key: &NodeKey, viewer: &BTreeSet<EntityKey>) -> Score {
        self.at(key)
            .map(|at| self.leverage_at(at, Some(viewer)).0)
            .unwrap_or_default()
    }

    /// Word operations, instants, and edges the pass read, for the cost test.
    #[must_use]
    pub fn operation_count(&self) -> u64 {
        self.operations
    }
}

/// Rows of bits, one per instant, a bit per node.
struct BitRows {
    words: usize,
    bits: Vec<u64>,
}

impl BitRows {
    fn new(rows: usize, words: usize) -> Self {
        Self {
            words,
            bits: vec![0; rows * words],
        }
    }

    fn row(&self, row: usize) -> &[u64] {
        let from = row * self.words;
        self.bits.get(from..from + self.words).unwrap_or(&[])
    }

    fn set(&mut self, row: usize, bit: usize) {
        if let Some(word) = self.bits.get_mut(row * self.words + bit / 64) {
            *word |= 1 << (bit % 64);
        }
    }

    /// Row `into` takes every bit of row `from`, a different row.
    fn union(&mut self, into: usize, from: usize) {
        assert_ne!(into, from, "a row unions another");
        let words = self.words;
        let (low, high) = self.bits.split_at_mut(into.max(from) * words);
        let (target, source) = if into < from {
            (
                low.get_mut(into * words..(into + 1) * words),
                high.get(..words),
            )
        } else {
            (
                high.get_mut(..words),
                low.get(from * words..(from + 1) * words),
            )
        };
        if let (Some(target), Some(source)) = (target, source) {
            for (word, other) in target.iter_mut().zip(source) {
                *word |= other;
            }
        }
    }
}
