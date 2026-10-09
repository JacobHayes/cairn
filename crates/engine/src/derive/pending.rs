//! Pending relevance and what-if reads (PRD Gating: Kleene evaluation; C2, C12). Relevance
//! treats a decision that is itself undecided as unanswered, so a node gated on it reads
//! `not_relevant` though it may well apply once the decision upstream of it is answered. This
//! module tells the two apart by re-evaluating every condition with such decisions read as
//! Kleene *unknown*: a node whose relevance is `not_relevant` but would be `undecided` that
//! way is **pending** on those decisions. Relevance itself is unchanged, so gravity, blocking
//! and the guards follow the PRD exactly.
//!
//! The same evaluation, run under an assumed answer, is what an answer's effects compare
//! ([`super::super::project`]'s `answer_effects`): a cheap three-valued re-evaluation of
//! relevance, never a full derive.
//!
//! Cost at `node_count_max` (2,000 nodes, 16 clauses each): two relevance passes (the strict
//! one and the unknown-reading one), each about 34,000 steps, and for each pending node a
//! walk up its containment chain (at most `containment_depth_max` steps).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{AnswerValue, Deployment, NodeKey, Relevance, State};

use super::relevance::{Lens, Producer, Relevances, pass_with};
use super::{skip, stored_state};
use crate::graph::Graph;

/// How a node's relevance reads to a person: the relevance value with `not_relevant` split by
/// whether it can still change by answering a decision that is itself undecided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelevanceClass {
    /// Relevant.
    Relevant,
    /// Undecided: waits on a decision that is open and relevant.
    Undecided,
    /// Not relevant only because a decision it reads is undecided itself, so it is
    /// unanswered; read as unknown, the node would be undecided.
    Pending,
    /// Not relevant for good: ruled out by an answer, a skip, or a closed branch.
    Settled,
}

/// Every node's [`RelevanceClass`], and for each pending node the decisions it is pending on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Classes {
    classes: BTreeMap<NodeKey, RelevanceClass>,
    pending_on: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
}

impl Classes {
    /// The node's class.
    ///
    /// # Panics
    ///
    /// When the node is not in the derived graph: a caller's key always comes from it.
    #[must_use]
    pub fn class(&self, key: &NodeKey) -> RelevanceClass {
        let found = self.classes.get(key);
        assert!(found.is_some(), "{key} is not in the derived graph");
        found.copied().unwrap_or(RelevanceClass::Settled)
    }

    /// The decisions a pending node's `not_relevant` value rests on, empty when the node is
    /// not pending: those a condition leaving it undecided reads, its own or an ancestor's,
    /// that are themselves undecided or pending (so unanswered, strictly) and would be unknown.
    #[must_use]
    pub fn pending_on(&self, key: &NodeKey) -> &BTreeSet<NodeKey> {
        static EMPTY: BTreeSet<NodeKey> = BTreeSet::new();
        self.pending_on.get(key).unwrap_or(&EMPTY)
    }

    /// Every node and its class, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeKey, RelevanceClass)> {
        self.classes.iter().map(|(key, class)| (key, *class))
    }
}

/// Gating, C2: every node's relevance class under the journey's stored state, and what each
/// pending node waits on. `graph` is the one derived and `deployment` the one it was derived
/// over.
#[must_use]
pub fn classify(graph: &Graph, deployment: &Deployment) -> Classes {
    classes(graph, deployment, None)
}

/// C12: every node's class if `decision` were decided with `answer` (and in effect, which the
/// caller checks: it is relevant and open or decided).
#[must_use]
pub fn classify_assuming(
    graph: &Graph,
    deployment: &Deployment,
    decision: &NodeKey,
    answer: &AnswerValue,
) -> Classes {
    classes(graph, deployment, Some((decision, answer)))
}

fn classes(
    graph: &Graph,
    deployment: &Deployment,
    assume: Option<(&NodeKey, &AnswerValue)>,
) -> Classes {
    let inherited = skip::inherited(graph);
    let strict = pass_with(
        graph,
        deployment,
        &inherited,
        Lens {
            assume,
            pending: false,
        },
    );
    let unknown = pass_with(
        graph,
        deployment,
        &inherited,
        Lens {
            assume,
            pending: true,
        },
    );
    let mut found = Classes::default();
    for (key, relevance) in strict.iter() {
        let class = match relevance.value {
            Relevance::Relevant => RelevanceClass::Relevant,
            Relevance::Undecided => RelevanceClass::Undecided,
            Relevance::NotRelevant if unknown.value(key) == Relevance::Undecided => {
                RelevanceClass::Pending
            }
            Relevance::NotRelevant => RelevanceClass::Settled,
        };
        if class == RelevanceClass::Pending {
            let waiting = waiting_on(graph, &unknown, &inherited, key);
            assert!(!waiting.is_empty(), "{key} is pending on a decision");
            found.pending_on.insert(key.clone(), waiting);
        }
        found.classes.insert(key.clone(), class);
    }
    assert_eq!(found.classes.len(), graph.document().nodes.len());
    found
}

/// The decisions the unknown-reading pass leaves the node undecided on: up the tree while
/// undecided, the decisions each node's own undecided condition reads that are themselves
/// undecided there and open or decided (so read as unknown, not unanswered). Some of these
/// the strict pass reads another way (unanswered, or as the answer they hold), which is what
/// makes the node pending.
fn waiting_on(
    graph: &Graph,
    unknown: &Relevances,
    inherited: &BTreeMap<NodeKey, NodeKey>,
    key: &NodeKey,
) -> BTreeSet<NodeKey> {
    let document = graph.document();
    let reads_unknown = |decision: &&NodeKey| {
        let state = document
            .nodes
            .get(*decision)
            .map(|node| stored_state(document, node));
        unknown.value(decision) == Relevance::Undecided
            && (state == Some(State::Decided)
                || (state == Some(State::Open) && !inherited.contains_key(*decision)))
    };
    let mut waiting = BTreeSet::new();
    let mut current = Some(key);
    // The depth limit bounds the walk up to the root.
    while let Some(node) = current {
        let Some(found) = unknown.get(node) else {
            break;
        };
        if found.value != Relevance::Undecided {
            break;
        }
        if let Producer::Condition { on, decisions } = &found.producer
            && on == node
        {
            waiting.extend(decisions.iter().filter(reads_unknown).cloned());
        }
        current = graph.tree().parent(node);
    }
    waiting
}
