//! Where a graph names an identity (E6, H3): the one typed rule every reader of a graph's
//! identity references uses, the engine's checks and both stores' reference queries alike.

use std::collections::BTreeSet;

use crate::condition::{Clause, Condition, ConditionValue};
use crate::graph::Graph;
use crate::id::{IdentityKey, NodeKey, RoleKey};
use crate::node::{AnswerSpec, ParticipationSource, Payload};
use crate::refs::KeyRefs;
use crate::state::AnswerValue;

/// Where a graph names an identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mention {
    /// An identity or identity-list answer to a decision.
    Answer(NodeKey),
    /// A direct role fill.
    RoleFill(RoleKey),
    /// An explicit participation on a node.
    Participation(NodeKey),
}

impl Graph {
    /// Every identity the graph names in an answer, a role fill, or an explicit participation,
    /// with where it names it, by the key it holds (an identity or an alias).
    #[must_use]
    pub fn identity_mentions(&self) -> Vec<(Mention, &IdentityKey)> {
        let mut found = Vec::new();
        for (decision, answer) in &self.state.answers {
            match answer {
                AnswerValue::Identity(identity) => {
                    found.push((Mention::Answer(decision.clone()), identity));
                }
                AnswerValue::IdentityList(identities) => found.extend(
                    identities
                        .iter()
                        .map(|identity| (Mention::Answer(decision.clone()), identity)),
                ),
                AnswerValue::Boolean(_)
                | AnswerValue::SingleChoice(_)
                | AnswerValue::MultiChoice(_)
                | AnswerValue::Text(_)
                | AnswerValue::Date(_) => {}
            }
        }
        for (role, identities) in &self.state.role_fills {
            found.extend(
                identities
                    .iter()
                    .map(|identity| (Mention::RoleFill(role.clone()), identity)),
            );
        }
        for node in self.nodes.values() {
            for source in node.participations.as_map().values() {
                if let ParticipationSource::Identities(identities) = source {
                    found.extend(
                        identities
                            .iter()
                            .map(|identity| (Mention::Participation(node.key.clone()), identity)),
                    );
                }
            }
        }
        found
    }

    /// Every identity a node's condition compares an identity or identity-list decision with
    /// (E6: a merge can change what such a condition evaluates to), by the node holding the
    /// condition. See [`condition_identities`].
    #[must_use]
    pub fn condition_identity_mentions(&self) -> Vec<(NodeKey, IdentityKey)> {
        let mut found = Vec::new();
        for node in self.nodes.values() {
            if let Some(condition) = &node.relevant_when {
                let keys =
                    condition_identities(condition, |decision| self.is_identity_decision(decision));
                found.extend(keys.into_iter().map(|key| (node.key.clone(), key)));
            }
        }
        found
    }

    /// Whether `node` is a decision answered with an identity or identity list.
    #[must_use]
    pub fn is_identity_decision(&self, node: &NodeKey) -> bool {
        self.nodes
            .get(node)
            .is_some_and(|target| match &target.payload {
                Payload::Decision(spec) => matches!(
                    spec.answer,
                    AnswerSpec::Identity { .. } | AnswerSpec::IdentityList { .. }
                ),
                Payload::Deliverable(_)
                | Payload::Action(_)
                | Payload::Milestone(_)
                | Payload::Group(_) => false,
            })
    }

    /// Every identity key the graph names anywhere (an answer, a fill, a participation, or an
    /// identity-typed condition), by the key it holds.
    #[must_use]
    pub fn identity_references(&self) -> BTreeSet<IdentityKey> {
        let mentioned = self.identity_mentions();
        let compared = self.condition_identity_mentions();
        mentioned
            .into_iter()
            .map(|(_, key)| key.clone())
            .chain(compared.into_iter().map(|(_, key)| key))
            .collect()
    }
}

/// The identities `condition` compares an identity-typed decision with, where `identity_typed`
/// says whether a decision is one. Only those clauses count: a text decision's comparison that
/// spells a key names no one. Cost: one walk of the condition, at most 16 clauses.
#[must_use]
pub fn condition_identities(
    condition: &Condition<KeyRefs>,
    identity_typed: impl Fn(&NodeKey) -> bool,
) -> Vec<IdentityKey> {
    let mut found = Vec::new();
    let mut stack = vec![condition.root()];
    while let Some(clause) = stack.pop() {
        let (decision, values): (&NodeKey, Vec<&ConditionValue>) = match clause {
            Clause::All(children) | Clause::Any(children) => {
                stack.extend(children);
                continue;
            }
            Clause::Not(child) => {
                stack.push(child.as_ref());
                continue;
            }
            Clause::Answered(_) => continue,
            Clause::Equals(comparison)
            | Clause::NotEquals(comparison)
            | Clause::Contains(comparison) => (&comparison.decision, vec![&comparison.value]),
            Clause::In(membership) => (
                &membership.decision,
                membership.values.as_slice().iter().collect(),
            ),
        };
        if !identity_typed(decision) {
            continue;
        }
        for value in values {
            if let ConditionValue::Text(text) = value
                && let Ok(identity) = text.as_str().parse::<IdentityKey>()
            {
                found.push(identity);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::from_yaml;

    fn graph(nodes: &str) -> Graph {
        from_yaml(&format!("nodes:\n{nodes}")).unwrap()
    }

    /// E6: a comparison names an identity only when its decision is identity-typed; a text
    /// decision whose comparison spells a key names no one, so deleting that identity is not
    /// blocked by it.
    #[test]
    fn a_condition_names_an_identity_only_through_an_identity_decision() {
        let graph = graph(
            "- {key: n_who, id: who, kind: decision, title: Who, prompt: Who?, answer_type: identity}\n\
             - {key: n_note, id: note, kind: decision, title: Note, prompt: Note?, answer_type: text}\n\
             - {key: n_a, id: a, kind: action, title: A, relevant_when: {equals: {decision: n_who, value: who_alice}}}\n\
             - {key: n_b, id: b, kind: action, title: B, relevant_when: {equals: {decision: n_note, value: who_bob}}}\n",
        );
        assert_eq!(
            graph.condition_identity_mentions(),
            [(
                "n_a".parse::<NodeKey>().unwrap(),
                "who_alice".parse::<IdentityKey>().unwrap()
            )]
        );
        assert_eq!(
            graph.identity_references(),
            BTreeSet::from(["who_alice".parse::<IdentityKey>().unwrap()])
        );
    }
}
