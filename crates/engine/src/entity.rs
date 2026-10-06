//! Entity references and their resolution through aliases (E6, H3): where a graph names
//! entities, and which entity a key names once merges are followed.

use cairn_schema::{
    AnswerSpec, AnswerValue, Clause, ConditionValue, Deployment, EntityKey, NodeKey,
    ParticipationSource, Payload,
};

use crate::graph::Document;

/// Where a graph names an entity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mention {
    /// An entity or entity-list answer to a decision.
    Answer(NodeKey),
    /// A direct role fill.
    RoleFill(cairn_schema::RoleKey),
    /// An explicit participation on a node.
    Participation(NodeKey),
}

/// Every entity a graph names, with where it names it.
#[must_use]
pub(crate) fn mentions(document: &Document) -> Vec<(Mention, &EntityKey)> {
    let mut found = Vec::new();
    for (decision, answer) in &document.state.answers {
        match answer {
            AnswerValue::Entity(entity) => found.push((Mention::Answer(decision.clone()), entity)),
            AnswerValue::EntityList(entities) => {
                found.extend(
                    entities
                        .iter()
                        .map(|entity| (Mention::Answer(decision.clone()), entity)),
                );
            }
            AnswerValue::Boolean(_)
            | AnswerValue::SingleChoice(_)
            | AnswerValue::MultiChoice(_)
            | AnswerValue::Text(_)
            | AnswerValue::Date(_) => {}
        }
    }
    for (role, entities) in &document.state.role_fills {
        found.extend(
            entities
                .iter()
                .map(|entity| (Mention::RoleFill(role.clone()), entity)),
        );
    }
    for node in document.nodes.values() {
        for source in node.participations.as_map().values() {
            if let ParticipationSource::Entities(entities) = source {
                found.extend(
                    entities
                        .iter()
                        .map(|entity| (Mention::Participation(node.key.clone()), entity)),
                );
            }
        }
    }
    found
}

/// Every entity a node's condition compares an entity or entity-list decision with (E6: a
/// merge can change what such a condition evaluates to), by the node holding the condition.
/// Cost: one walk of each condition, at most 16 clauses.
#[must_use]
pub(crate) fn condition_mentions(document: &Document) -> Vec<(NodeKey, EntityKey)> {
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
            let entity_typed =
                document
                    .nodes
                    .get(decision)
                    .is_some_and(|target| match &target.payload {
                        Payload::Decision(spec) => matches!(
                            spec.answer,
                            AnswerSpec::Entity { .. } | AnswerSpec::EntityList { .. }
                        ),
                        Payload::Deliverable(_)
                        | Payload::Action(_)
                        | Payload::Milestone(_)
                        | Payload::Group(_) => false,
                    });
            if !entity_typed {
                continue;
            }
            for value in values {
                if let ConditionValue::Text(text) = value
                    && let Ok(entity) = text.as_str().parse::<EntityKey>()
                {
                    found.push((node.key.clone(), entity));
                }
            }
        }
    }
    found
}

/// E6: the entity a key names, following an alias left by a merge; `None` when it names no
/// entity. Aliases always point at an entity (the deployment stage checks), so one step is
/// the whole walk.
#[must_use]
pub(crate) fn resolve<'a>(deployment: &'a Deployment, key: &'a EntityKey) -> Option<&'a EntityKey> {
    if deployment.entities.get(key).is_some() {
        return Some(key);
    }
    let target = deployment.aliases.get(key)?;
    assert!(!deployment.aliases.contains_key(target) || deployment.entities.get(target).is_none());
    deployment.entities.get(target).map(|_| target)
}
