//! C12, E3: what answering a decision does, per choice (the form's "brings in 6, drops 3, 2
//! decided later" beside each option). The projection re-evaluates relevance under each
//! choice, a three-valued evaluation of conditions ([`crate::derive::pending`]): never a
//! full derive, and nothing is applied or stored. Each node has a class before and after
//! ([`RelevanceClass`]):
//!
//! - **brings in**: relevant after, not before; of these, the decisions that open (still to
//!   be answered, and not under a skip).
//! - **drops**: settled as not relevant after, not before. A node that is not relevant only
//!   until an upstream decision is answered (pending) is never a drop.
//! - **decided later**: a node the decision's answer can change that stays undecided or
//!   pending after this choice, because it hangs on another decision still to be answered.
//! - **drops with progress**: the drops that are started, done, decided, or reached; the
//!   answer does not undo that.
//!
//! The choice the decision already holds changes nothing, so its effects are empty. A
//! multi-choice decision's choices are each the current answer with that choice toggled, so
//! a choice already in the answer shows what removing it does.
//! When the decision's answer is not in effect (it is not relevant, skipped, or under a
//! skip), no answer changes any other node and every effect is empty.
//!
//! Cost at `node_count_max` (2,000 nodes) and 32 choices: two relevance passes per choice
//! (about 34,000 steps each), a comparison per node, and the decision's affected set once,
//! O(nodes x depth x clauses + decisions x nodes): about 2.5 million steps, once per
//! (revision, decision), cached by the caller.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::EXPLANATION_ENTRY_COUNT_MAX;
use cairn_schema::{
    AffectedNodes, AnswerEffects, AnswerSpec, AnswerValue, BoundedSet, ChoiceEffect, Deployment,
    NodeKey, NodeKind, Payload, Slug, State,
};

use super::rows::count;
use super::{DerivedJourney, ProjectionError};
use crate::derive::Relevances;
use crate::derive::pending::{RelevanceClass, classify, classify_assuming};
use crate::derive::skip;

/// One answer to evaluate: the whole answer, the choice it names, whether the choice is the
/// recorded one (a toggled multi choice that is in the answer), and whether the answer is the
/// recorded one, which changes nothing.
struct Candidate {
    answer: AnswerValue,
    choice: Option<Slug>,
    current: bool,
    unchanged: bool,
}

impl DerivedJourney<'_> {
    /// C12, E3: what each answer to the decision does to the journey's scope, from a
    /// three-valued re-evaluation of relevance; none when the node is not a decision.
    ///
    /// # Errors
    ///
    /// When the node is not in the journey.
    ///
    /// # Panics
    ///
    /// Never: a toggled multi-choice answer stays within the decision's choices, which are
    /// within the limit.
    pub fn answer_effects(
        &self,
        key: &NodeKey,
        deployment: &Deployment,
    ) -> Result<Option<AnswerEffects>, ProjectionError> {
        let node = self.known(key)?;
        let Payload::Decision(decision) = &node.payload else {
            return Ok(None);
        };
        let (fills_role, pins) = match &decision.answer {
            AnswerSpec::Date { feeds_milestone } => (None, feeds_milestone.clone()),
            AnswerSpec::Entity { fills_role } | AnswerSpec::EntityList { fills_role } => {
                (fills_role.clone(), None)
            }
            AnswerSpec::Boolean
            | AnswerSpec::SingleChoice(_)
            | AnswerSpec::MultiChoice(_)
            | AnswerSpec::Text => (None, None),
        };
        let applies = self.answer_applies(key);
        let candidates = self.candidates(key, &decision.answer);
        let choices = if applies && !candidates.is_empty() {
            let before = classify(self.graph, deployment);
            let affected = self.conditioned_by().remove(key).unwrap_or_default();
            // A decision under a skip stays closed whatever the answer makes relevant.
            let skipped = skip::inherited(self.graph);
            candidates
                .into_iter()
                .map(|candidate| {
                    self.effect(key, deployment, &before, (&affected, &skipped), candidate)
                })
                .collect()
        } else {
            candidates
                .into_iter()
                .map(|candidate| ChoiceEffect {
                    answer: candidate.answer,
                    choice: candidate.choice,
                    current: candidate.current,
                    brings_in: AffectedNodes::default(),
                    opens_decisions: AffectedNodes::default(),
                    drops: AffectedNodes::default(),
                    drops_with_progress: AffectedNodes::default(),
                    decided_later: AffectedNodes::default(),
                })
                .collect()
        };
        Ok(Some(AnswerEffects {
            decision: key.clone(),
            fills_role,
            pins,
            applies,
            choices,
        }))
    }

    /// E3: the decision's answer would be in effect: it is relevant, and open (not under a
    /// skip) or decided.
    fn answer_applies(&self, key: &NodeKey) -> bool {
        let relevant = self.derived.relevance().value(key) == cairn_schema::Relevance::Relevant;
        let state = self.state(key);
        relevant
            && (state == State::Decided
                || (state == State::Open && self.derived.skips().skipped_by(key).is_none()))
    }

    /// The answers to evaluate, in the decision's order: both booleans, each choice, or each
    /// choice toggled in the current answer; none for the other answer types.
    fn candidates(
        &self,
        key: &NodeKey,
        spec: &AnswerSpec<cairn_schema::KeyRefs>,
    ) -> Vec<Candidate> {
        let recorded = (self.state(key) == State::Decided)
            .then(|| self.graph.document().state.answers.get(key))
            .flatten();
        match spec {
            AnswerSpec::Boolean => [false, true]
                .into_iter()
                .map(|value| {
                    let answer = AnswerValue::Boolean(value);
                    let current = recorded == Some(&answer);
                    Candidate {
                        answer,
                        choice: None,
                        current,
                        unchanged: current,
                    }
                })
                .collect(),
            AnswerSpec::SingleChoice(choices) => choices
                .as_slice()
                .iter()
                .map(|choice| {
                    let answer = AnswerValue::SingleChoice(choice.id.clone());
                    let current = recorded == Some(&answer);
                    Candidate {
                        answer,
                        choice: Some(choice.id.clone()),
                        current,
                        unchanged: current,
                    }
                })
                .collect(),
            AnswerSpec::MultiChoice(choices) => {
                let held: BTreeSet<Slug> = match recorded {
                    Some(AnswerValue::MultiChoice(held)) => held.iter().cloned().collect(),
                    _ => BTreeSet::new(),
                };
                choices
                    .as_slice()
                    .iter()
                    .map(|choice| {
                        let mut toggled = held.clone();
                        let current = !toggled.insert(choice.id.clone());
                        if current {
                            toggled.remove(&choice.id);
                        }
                        let answer = BoundedSet::new(toggled)
                            .unwrap_or_else(|error| panic!("within the choices: {error}"));
                        Candidate {
                            answer: AnswerValue::MultiChoice(answer),
                            choice: Some(choice.id.clone()),
                            current,
                            unchanged: false,
                        }
                    })
                    .collect()
            }
            AnswerSpec::Text
            | AnswerSpec::Date { .. }
            | AnswerSpec::Entity { .. }
            | AnswerSpec::EntityList { .. } => Vec::new(),
        }
    }

    /// One candidate's effect: the classes with it assumed against those before.
    fn effect(
        &self,
        key: &NodeKey,
        deployment: &Deployment,
        before: &Relevances,
        (affected, skipped): (&BTreeSet<NodeKey>, &BTreeMap<NodeKey, NodeKey>),
        candidate: Candidate,
    ) -> ChoiceEffect {
        let mut brings_in = Vec::new();
        let mut opens_decisions = Vec::new();
        let mut drops = Vec::new();
        let mut drops_with_progress = Vec::new();
        let mut decided_later = Vec::new();
        if !candidate.unchanged {
            let after = classify_assuming(self.graph, deployment, key, &candidate.answer);
            for node in self.tree_order(None) {
                let (was, now) = (before.class(node), after.class(node));
                if now == RelevanceClass::Relevant && was != RelevanceClass::Relevant {
                    brings_in.push(node.clone());
                    if self.node(node).kind() == NodeKind::Decision
                        && self.state(node) == State::Open
                        && !skipped.contains_key(node)
                    {
                        opens_decisions.push(node.clone());
                    }
                }
                if now == RelevanceClass::Settled && was != RelevanceClass::Settled {
                    drops.push(node.clone());
                    if matches!(
                        self.state(node),
                        State::Active | State::Done | State::Decided | State::Reached
                    ) {
                        drops_with_progress.push(node.clone());
                    }
                }
                let hangs = matches!(now, RelevanceClass::Undecided | RelevanceClass::Pending);
                if hangs && affected.contains(node) {
                    decided_later.push(node.clone());
                }
            }
        }
        ChoiceEffect {
            answer: candidate.answer,
            choice: candidate.choice,
            current: candidate.current,
            brings_in: affected_nodes(brings_in),
            opens_decisions: affected_nodes(opens_decisions),
            drops: affected_nodes(drops),
            drops_with_progress: affected_nodes(drops_with_progress),
            decided_later: affected_nodes(decided_later),
        }
    }
}

/// The nodes with their count, the first `explanation_entry_count_max` kept.
fn affected_nodes(nodes: Vec<NodeKey>) -> AffectedNodes {
    let total = count(nodes.len());
    AffectedNodes {
        total,
        nodes: nodes
            .into_iter()
            .take(EXPLANATION_ENTRY_COUNT_MAX as usize)
            .collect(),
    }
}
