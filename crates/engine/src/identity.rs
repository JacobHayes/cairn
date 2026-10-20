//! Identity references and their resolution through aliases (E6, H3): where a graph names
//! identities, and which identity a key names once merges are followed.

use cairn_schema::{
    AnswerSpec, AnswerValue, Clause, ConditionValue, Deployment, IdentityKey, NodeKey,
    ParticipationSource, Payload,
};

use crate::graph::Document;

/// Where a graph names an identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mention {
    /// An identity or identity-list answer to a decision.
    Answer(NodeKey),
    /// A direct role fill.
    RoleFill(cairn_schema::RoleKey),
    /// An explicit participation on a node.
    Participation(NodeKey),
}

/// Every identity a graph names, with where it names it.
#[must_use]
pub(crate) fn mentions(document: &Document) -> Vec<(Mention, &IdentityKey)> {
    let mut found = Vec::new();
    for (decision, answer) in &document.state.answers {
        match answer {
            AnswerValue::Identity(identity) => {
                found.push((Mention::Answer(decision.clone()), identity));
            }
            AnswerValue::IdentityList(identities) => {
                found.extend(
                    identities
                        .iter()
                        .map(|identity| (Mention::Answer(decision.clone()), identity)),
                );
            }
            AnswerValue::Boolean(_)
            | AnswerValue::SingleChoice(_)
            | AnswerValue::MultiChoice(_)
            | AnswerValue::Text(_)
            | AnswerValue::Date(_) => {}
        }
    }
    for (role, identities) in &document.state.role_fills {
        found.extend(
            identities
                .iter()
                .map(|identity| (Mention::RoleFill(role.clone()), identity)),
        );
    }
    for node in document.nodes.values() {
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

/// Every identity a node's condition compares an identity or identity-list decision with (E6: a
/// merge can change what such a condition evaluates to), by the node holding the condition.
/// Cost: one walk of each condition, at most 16 clauses.
#[must_use]
pub(crate) fn condition_mentions(document: &Document) -> Vec<(NodeKey, IdentityKey)> {
    let mut found = Vec::new();
    for node in document.nodes.values() {
        let Some(condition) = &node.relevant_when else {
            continue;
        };
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
            let identity_typed =
                document
                    .nodes
                    .get(decision)
                    .is_some_and(|target| match &target.payload {
                        Payload::Decision(spec) => matches!(
                            spec.answer,
                            AnswerSpec::Identity { .. } | AnswerSpec::IdentityList { .. }
                        ),
                        Payload::Deliverable(_)
                        | Payload::Action(_)
                        | Payload::Milestone(_)
                        | Payload::Group(_) => false,
                    });
            if !identity_typed {
                continue;
            }
            for value in values {
                if let ConditionValue::Text(text) = value
                    && let Ok(identity) = text.as_str().parse::<IdentityKey>()
                {
                    found.push((node.key.clone(), identity));
                }
            }
        }
    }
    found
}

/// E6: the identity a key names, following an alias left by a merge; `None` when it names no
/// identity. Aliases always point at an identity (the deployment stage checks), so one step is
/// the whole walk.
#[must_use]
pub(crate) fn resolve<'a>(
    deployment: &'a Deployment,
    key: &'a IdentityKey,
) -> Option<&'a IdentityKey> {
    if deployment.identities.get(key).is_some() {
        return Some(key);
    }
    let target = deployment.aliases.get(key)?;
    assert!(
        !deployment.aliases.contains_key(target) || deployment.identities.get(target).is_none()
    );
    deployment.identities.get(target).map(|_| target)
}
