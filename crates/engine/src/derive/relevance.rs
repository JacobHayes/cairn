//! Pass 1, relevance (PRD Gating; A5; ARCHITECTURE, Read path: derive): each node's
//! three-valued relevance and what produced it (C8: node detail names the ancestor or
//! decision). A node's own condition evaluates in Kleene's logic over answers
//! ([`super::condition`]) and combines with its parent's value, `not_relevant` dominating
//! `undecided` dominating `relevant`; a node with no condition takes its parent's value. A
//! force include makes the node relevant whatever its condition and ancestors say, and its
//! descendants take `relevant` as their ancestor value while still evaluating their own
//! conditions.
//!
//! Order: a node's value reads its parent's and those of the decisions its condition names,
//! so nodes are evaluated in a topological order of those inputs (Kahn's algorithm). The
//! order exists for every valid graph: each input is also a gate edge of the effective
//! dependency graph (the condition entry chain, and a condition gate on the decision), which
//! validation holds acyclic. A force-included node has no inputs.
//!
//! D8's pending rule rides the same walk. A decision that is itself undecided counts as
//! unanswered in a condition, so a node reading its answer is `not_relevant` though it may well
//! apply once that decision is answered. Each node therefore also carries its value with those
//! decisions read as still to come (Kleene unknown): when that is not `not_relevant`, the
//! node's `not_relevant` is pending on them. The value itself never changes.
//!
//! Cost at `node_count_max` (2,000 nodes, 16 clauses each): building the order is
//! O(nodes + clauses), about 34,000 steps; each node's condition is evaluated once, a few
//! dozen steps; memory is one record per node, with the decisions its producing condition
//! reads (at most 16), a few hundred kilobytes.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{AnswerValue, Deployment, KeyRefs, Node, NodeKey, Relevance, State};

use super::condition::{self, Term, Truth};
use super::{forced, stored_state};
use crate::graph::{Document, Graph};

/// What produced a node's relevance (C8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Producer {
    /// No condition applies to the node or any ancestor: relevant by default.
    Unconditioned,
    /// The condition on `on` (the node itself or an ancestor), reading `decisions`.
    Condition {
        /// The node whose condition decided the value.
        on: NodeKey,
        /// The decisions that condition reads.
        decisions: BTreeSet<NodeKey>,
    },
    /// A force include on `on` (the node itself or an ancestor).
    Forced {
        /// The node force-included.
        on: NodeKey,
    },
}

/// A node's relevance and its producer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeRelevance {
    /// The value.
    pub value: Relevance,
    /// What produced it.
    pub producer: Producer,
    /// D8: the value with every undecided decision a condition reads taken as still to come
    /// instead of unanswered. It is never more settled than `value`: `not_relevant` here
    /// means no answer to those decisions could make the node apply.
    pub optimistic: Relevance,
    /// D8: when `value` is `not_relevant` and `optimistic` is not, the undecided decisions it
    /// rests on; empty otherwise.
    pub pending_on: BTreeSet<NodeKey>,
}

/// Every node's relevance (pass 1's output).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relevances {
    nodes: BTreeMap<NodeKey, NodeRelevance>,
}

impl Relevances {
    /// The node's relevance and producer, when the node exists.
    #[must_use]
    pub fn get(&self, key: &NodeKey) -> Option<&NodeRelevance> {
        self.nodes.get(key)
    }

    /// The node's relevance value.
    ///
    /// # Panics
    ///
    /// When the node is not in the derived graph: a caller's key always comes from it.
    #[must_use]
    pub fn value(&self, key: &NodeKey) -> Relevance {
        let found = self.nodes.get(key);
        assert!(found.is_some(), "{key} is not in the derived graph");
        found.map_or(Relevance::NotRelevant, |found| found.value)
    }

    /// Relevant or undecided: in scope (PRD Containment, D1a).
    #[must_use]
    pub fn in_scope(&self, key: &NodeKey) -> bool {
        self.value(key) != Relevance::NotRelevant
    }

    /// Every node's relevance, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeKey, &NodeRelevance)> {
        self.nodes.iter()
    }

    /// E3: the decision's answer while it is in effect, decided and relevant. Role fills and
    /// `feeds_milestone` pins follow it: a decision that leaves scope or is reopened no longer
    /// fills or pins, and one that returns to relevant-and-decided does again.
    ///
    /// # Panics
    ///
    /// When the decision is not in the derived graph, or is decided with no answer, which
    /// validation rejects.
    #[must_use]
    pub fn answer_in_effect<'g>(
        &self,
        document: &'g Document,
        decision: &NodeKey,
    ) -> Option<&'g AnswerValue> {
        let node = document.nodes.get(decision)?;
        let decided = stored_state(document, node) == State::Decided;
        let relevant = self.value(decision) == Relevance::Relevant;
        let answer = document.state.answers.get(decision);
        assert!(
            !decided || answer.is_some(),
            "a decided decision holds its answer"
        );
        answer.filter(|_| decided && relevant)
    }
}

/// The inputs of a node's relevance: its parent and the decisions its condition reads,
/// none when it is force-included.
fn inputs<'d>(document: &Document, node: &'d Node<KeyRefs>) -> BTreeSet<&'d NodeKey> {
    if forced(document, &node.key) {
        return BTreeSet::new();
    }
    let decisions = node.relevant_when.iter().flat_map(|c| c.decisions());
    node.parent.iter().chain(decisions).collect()
}

/// Nodes in an order where each comes after its inputs (Kahn's algorithm).
fn order(document: &Document) -> Vec<&NodeKey> {
    let mut waiting: BTreeMap<&NodeKey, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<&NodeKey, Vec<&NodeKey>> = BTreeMap::new();
    for node in document.nodes.values() {
        let inputs = inputs(document, node);
        waiting.insert(&node.key, inputs.len());
        for input in inputs {
            dependents.entry(input).or_default().push(&node.key);
        }
    }
    let mut ready: Vec<&NodeKey> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(key, _)| *key)
        .collect();
    let mut ordered = Vec::with_capacity(document.nodes.len());
    while let Some(next) = ready.pop() {
        ordered.push(next);
        for dependent in dependents.get(next).into_iter().flatten() {
            if let Some(count) = waiting.get_mut(dependent) {
                *count -= 1;
                if *count == 0 {
                    ready.push(dependent);
                }
            }
        }
    }
    assert_eq!(
        ordered.len(),
        document.nodes.len(),
        "relevance inputs are acyclic in a valid graph"
    );
    ordered
}

/// How a relevance pass reads decisions beyond the stored state: the what-if reads
/// ([`super::pending`], answer effects) re-run the pass with one of these; the derive's own
/// pass uses none.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Lens<'a> {
    /// A decision read as decided with this answer, while it is relevant and open or decided
    /// (E3): the answer a person is about to give.
    pub assume: Option<(&'a NodeKey, &'a AnswerValue)>,
    /// A decision that is itself undecided is read as unknown, not unanswered: Kleene's logic
    /// through a chain of decisions, to tell a node that is not relevant for good from one
    /// that is not relevant only until an upstream decision is answered.
    pub pending: bool,
}

/// Pass 1: every node's relevance. `inherited_skips` names, for each node under a skip it is
/// not kept from, the skipped ancestor (D1a): an open decision there is effectively skipped,
/// so a condition reads it as unanswered.
#[must_use]
pub(crate) fn pass(
    graph: &Graph,
    deployment: &Deployment,
    inherited_skips: &BTreeMap<NodeKey, NodeKey>,
) -> Relevances {
    pass_with(graph, deployment, inherited_skips, Lens::default())
}

/// Pass 1 read through a [`Lens`].
#[must_use]
pub(crate) fn pass_with(
    graph: &Graph,
    deployment: &Deployment,
    inherited_skips: &BTreeMap<NodeKey, NodeKey>,
    lens: Lens<'_>,
) -> Relevances {
    let document = graph.document();
    let mut relevances = Relevances::default();
    for key in order(document) {
        let Some(node) = document.nodes.get(key) else {
            continue;
        };
        let found = evaluate(
            document,
            node,
            &relevances,
            deployment,
            inherited_skips,
            lens,
        );
        relevances.nodes.insert(key.clone(), found);
    }
    assert_eq!(relevances.nodes.len(), document.nodes.len());
    relevances
}

/// One node's relevance, its inputs already evaluated.
fn evaluate(
    document: &Document,
    node: &Node<KeyRefs>,
    done: &Relevances,
    deployment: &Deployment,
    inherited_skips: &BTreeMap<NodeKey, NodeKey>,
    lens: Lens<'_>,
) -> NodeRelevance {
    if forced(document, &node.key) {
        return NodeRelevance {
            value: Relevance::Relevant,
            producer: Producer::Forced {
                on: node.key.clone(),
            },
            optimistic: Relevance::Relevant,
            pending_on: BTreeSet::new(),
        };
    }
    let inherited = node.parent.as_ref().map_or(
        NodeRelevance {
            value: Relevance::Relevant,
            producer: Producer::Unconditioned,
            optimistic: Relevance::Relevant,
            pending_on: BTreeSet::new(),
        },
        |parent| {
            let found = done.get(parent);
            assert!(found.is_some(), "a parent is evaluated before its children");
            found.cloned().unwrap_or(NodeRelevance {
                value: Relevance::NotRelevant,
                producer: Producer::Unconditioned,
                optimistic: Relevance::NotRelevant,
                pending_on: BTreeSet::new(),
            })
        },
    );
    let Some(condition) = &node.relevant_when else {
        return inherited;
    };
    let term = |decision: &NodeKey| term(document, done, inherited_skips, lens, decision);
    let own = relevance_of(condition::evaluate(condition, term, deployment));
    let hopeful =
        |decision: &NodeKey| hopeful_term(document, done, inherited_skips, lens, decision);
    let own_optimistic = relevance_of(condition::evaluate(condition, hopeful, deployment));
    let value = dominant(inherited.value, own);
    let optimistic = dominant(inherited.optimistic, own_optimistic);
    let mut pending_on = BTreeSet::new();
    if value == Relevance::NotRelevant && optimistic != Relevance::NotRelevant {
        if inherited.value == Relevance::NotRelevant {
            pending_on.extend(inherited.pending_on.iter().cloned());
        }
        if own == Relevance::NotRelevant {
            let still_to_come = condition
                .decisions()
                .into_iter()
                .filter(|decision| {
                    term(decision) == Term::Unanswered && hopeful(decision) == Term::Unknown
                })
                .cloned();
            pending_on.extend(still_to_come);
        }
    }
    let producer = if value == own {
        Producer::Condition {
            on: node.key.clone(),
            decisions: condition.decisions().into_iter().cloned().collect(),
        }
    } else {
        inherited.producer
    };
    NodeRelevance {
        value,
        producer,
        optimistic,
        pending_on,
    }
}

/// A condition's truth as the relevance it gives.
fn relevance_of(truth: Truth) -> Relevance {
    match truth {
        Truth::True => Relevance::Relevant,
        Truth::False => Relevance::NotRelevant,
        Truth::Unknown => Relevance::Undecided,
    }
}

/// Gating: `not_relevant` dominates `undecided`, which dominates `relevant`.
fn dominant(first: Relevance, second: Relevance) -> Relevance {
    let rank = |value: Relevance| match value {
        Relevance::Relevant => 0,
        Relevance::Undecided => 1,
        Relevance::NotRelevant => 2,
    };
    if rank(second) > rank(first) {
        second
    } else {
        first
    }
}

/// What a decision contributes to a condition (Gating): its answer when decided and
/// relevant; unknown when open, relevant, and not under a skip; unanswered otherwise
/// (skipped, effectively skipped, not relevant, or itself undecided). A [`Lens`] changes two
/// readings: an assumed answer stands in for the decision's own while it is relevant and open
/// or decided, and with `pending` an undecided decision that is open or decided reads as
/// unknown.
fn term<'g>(
    document: &'g Document,
    done: &Relevances,
    inherited_skips: &BTreeMap<NodeKey, NodeKey>,
    lens: Lens<'g>,
    decision: &NodeKey,
) -> Term<'g> {
    let found = done.get(decision);
    assert!(
        found.is_some(),
        "a decision is evaluated before the conditions reading it"
    );
    let value = found.map(|found| found.value);
    let Some(node) = document.nodes.get(decision) else {
        return Term::Unanswered;
    };
    let state = stored_state(document, node);
    let under_skip = inherited_skips.contains_key(decision);
    let open = state == State::Open && !under_skip;
    if value == Some(Relevance::Undecided) && lens.pending && (open || state == State::Decided) {
        return Term::Unknown;
    }
    if value != Some(Relevance::Relevant) {
        return Term::Unanswered;
    }
    if let Some((assumed, answer)) = lens.assume
        && assumed == decision
        && (open || state == State::Decided)
    {
        return Term::Answered(answer);
    }
    match state {
        State::Decided => done
            .answer_in_effect(document, decision)
            .map_or(Term::Unanswered, Term::Answered),
        State::Open if !under_skip => Term::Unknown,
        State::Open
        | State::Skipped
        | State::Todo
        | State::Active
        | State::Done
        | State::Pending
        | State::Reached
        | State::Derived => Term::Unanswered,
    }
}

/// D8: what a decision contributes to a condition when decisions that are themselves
/// undecided are read as still to come: as [`term`] says, except that a decision that is not
/// relevant yet could still become so (its own `optimistic` value is not `not_relevant`), and
/// then, unless it is skipped, its answer is to come.
fn hopeful_term<'g>(
    document: &'g Document,
    done: &Relevances,
    inherited_skips: &BTreeMap<NodeKey, NodeKey>,
    lens: Lens<'g>,
    decision: &NodeKey,
) -> Term<'g> {
    let found = done.get(decision);
    assert!(
        found.is_some(),
        "a decision is evaluated before the conditions reading it"
    );
    let Some(found) = found else {
        return Term::Unanswered;
    };
    if found.value == Relevance::Relevant || found.optimistic == Relevance::NotRelevant {
        return term(document, done, inherited_skips, lens, decision);
    }
    let Some(node) = document.nodes.get(decision) else {
        return Term::Unanswered;
    };
    match stored_state(document, node) {
        // A skip above it reaches only a decision still open (D1a): a decided one keeps its
        // answer for when it applies again.
        State::Decided => Term::Unknown,
        State::Open if !inherited_skips.contains_key(decision) => Term::Unknown,
        State::Open
        | State::Skipped
        | State::Todo
        | State::Active
        | State::Done
        | State::Pending
        | State::Reached
        | State::Derived => Term::Unanswered,
    }
}
