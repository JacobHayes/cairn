//! Pending relevance and what-if reads (PRD Gating: Kleene evaluation; C2, C12). Relevance
//! treats a decision that is itself undecided as unanswered, so a node gated on it reads
//! `not_relevant` though it may well apply once the decision upstream of it is answered. Pass
//! 1 tells the two apart as it goes ([`super::relevance`]): a `not_relevant` node that is not
//! `not_relevant` with those decisions read as Kleene *unknown* is **pending** on them
//! (`pending_on`), the single source for [`RelevanceClass`] and for display state. Relevance
//! itself is unchanged, so gravity, blocking and the guards follow the PRD exactly.
//!
//! The same pass, run under an assumed answer, is what an answer's effects compare
//! ([`super::super::project`]'s `answer_effects`): a cheap three-valued re-evaluation of
//! relevance, never a full derive.

use cairn_schema::{AnswerValue, Deployment, NodeKey, Relevance};

use super::relevance::{self, NodeRelevance, Relevances};
use super::skip;
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

impl NodeRelevance {
    /// The node's [`RelevanceClass`].
    #[must_use]
    pub fn class(&self) -> RelevanceClass {
        match self.value {
            Relevance::Relevant => RelevanceClass::Relevant,
            Relevance::Undecided => RelevanceClass::Undecided,
            Relevance::NotRelevant if self.pending_on.is_empty() => RelevanceClass::Settled,
            Relevance::NotRelevant => RelevanceClass::Pending,
        }
    }
}

impl Relevances {
    /// The node's [`RelevanceClass`].
    ///
    /// # Panics
    ///
    /// When the node is not in the derived graph: a caller's key always comes from it.
    #[must_use]
    pub fn class(&self, key: &NodeKey) -> RelevanceClass {
        let found = self.get(key);
        assert!(found.is_some(), "{key} is not in the derived graph");
        found.map_or(RelevanceClass::Settled, NodeRelevance::class)
    }
}

/// Gating, C2: every node's relevance under the journey's stored state, with what each
/// pending node waits on. `graph` is the one derived and `deployment` the one it was derived
/// over.
#[must_use]
pub fn classify(graph: &Graph, deployment: &Deployment) -> Relevances {
    relevance::pass(graph, deployment, &skip::inherited(graph), None)
}

/// C12: every node's relevance if `decision` were decided with `answer` (and in effect, which
/// the caller checks: it is relevant and open or decided).
#[must_use]
pub fn classify_assuming(
    graph: &Graph,
    deployment: &Deployment,
    decision: &NodeKey,
    answer: &AnswerValue,
) -> Relevances {
    relevance::pass(
        graph,
        deployment,
        &skip::inherited(graph),
        Some((decision, answer)),
    )
}
