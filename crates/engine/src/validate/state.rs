//! Journey state (PRD Invariants: journey; D1; E3; B6; B10): every node has a stored state its
//! kind's machine has, answers match their decisions, direct role fills and pins never stand
//! in for a filling or feeding decision, and every record of state hangs off something that
//! exists. A route version or draft has no state at all. Entity references are checked with
//! the deployment by the apply pipeline. Each node, and the journey itself, holds at most its
//! limits of notes and links. Cost at the limits: one pass over each state map, each entry
//! one lookup by key.

use std::collections::BTreeMap;

use cairn_schema::{
    AnswerSpec, AnswerValue, Choices, JourneyState, KeyRefs, Node, NodeKey, Payload, RoleKey,
    SnoozeTarget, State, Subject, Violation, ViolationCode,
};

use super::{GraphCheck, at_node, missing_node_code, missing_role_code, violation};

/// Runs the state checks.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let state = &check.document.state;
    if !check.journey {
        if !state.is_empty() {
            out.push(violation(
                ViolationCode::StateNotOnKind,
                "a route version or draft holds no journey state",
            ));
        }
        return;
    }
    node_states(check, out);
    answers(check, out);
    role_fills(check, out);
    pins_and_snoozes(check, out);
    attached_records(check, out);
    annotation_counts(check, out);
}

/// D1: every node has a state its kind's machine has; B10: `atomic` only on placeholders.
fn node_states(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    for node in document.nodes.values() {
        let message = match document.state.nodes.get(&node.key) {
            None => "the node has no stored state (B1)",
            Some(stored) if !stored.state.legal_for(node.kind()) => {
                "the stored state is not one the node's kind has (D1)"
            }
            Some(stored) if stored.atomic && !is_placeholder(node) => {
                "only a placeholder is marked atomic (B10)"
            }
            Some(stored) if stored.skip_reason.is_some() != (stored.state == State::Skipped) => {
                "a skipped node keeps the reason it was skipped for, and only a skipped node has one (D1)"
            }
            Some(_) => continue,
        };
        out.push(at_node(
            check.tree,
            &node.key,
            ViolationCode::StateNotOnKind,
            message,
        ));
    }
    for key in document.state.nodes.keys() {
        if document.nodes.get(key).is_none() {
            out.push(state_without_node(check, key));
        }
    }
}

fn is_placeholder(node: &Node<KeyRefs>) -> bool {
    match &node.payload {
        Payload::Deliverable(deliverable) => deliverable.placeholder,
        Payload::Action(action) => action.placeholder,
        Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
    }
}

fn state_without_node(check: &GraphCheck<'_>, key: &NodeKey) -> Violation {
    let mut found = violation(
        missing_node_code(check.document, key),
        format!("journey state is recorded for {key}, which is not in the graph"),
    );
    found.at.subject = Some(Subject::Node(key.clone()));
    found
}

/// Invariants: answers match their decision's answer type; D1: a decision is decided exactly
/// when it holds an answer.
fn answers(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let state = &document.state;
    for (key, answer) in &state.answers {
        let Some(node) = document.nodes.get(key) else {
            out.push(state_without_node(check, key));
            continue;
        };
        let (code, message) = match &node.payload {
            Payload::Decision(decision) if !answer_fits(&decision.answer, answer) => (
                ViolationCode::AnswerTypeMismatch,
                "the answer does not match the decision's answer type or choices",
            ),
            Payload::Decision(_) => continue,
            Payload::Deliverable(_)
            | Payload::Action(_)
            | Payload::Milestone(_)
            | Payload::Group(_) => (
                ViolationCode::WrongReferenceKind,
                "only a decision is answered",
            ),
        };
        out.push(at_node(check.tree, key, code, message));
    }
    for node in document.nodes.values() {
        let decided = state
            .nodes
            .get(&node.key)
            .is_some_and(|stored| stored.state == State::Decided);
        let decision = matches!(node.payload, Payload::Decision(_));
        if decision && decided != state.answers.contains_key(&node.key) {
            out.push(at_node(
                check.tree,
                &node.key,
                ViolationCode::StateNotOnKind,
                "a decision is decided exactly when it holds an answer (D1)",
            ));
        }
    }
}

/// A4: an answer of the decision's type, naming only its choices.
#[must_use]
pub(crate) fn answer_fits(spec: &AnswerSpec<KeyRefs>, answer: &AnswerValue) -> bool {
    let has = |choices: &Choices, id: &cairn_schema::Slug| {
        choices.as_slice().iter().any(|choice| choice.id == *id)
    };
    match spec {
        AnswerSpec::SingleChoice(choices) => {
            matches!(answer, AnswerValue::SingleChoice(id) if has(choices, id))
        }
        AnswerSpec::MultiChoice(choices) => {
            matches!(answer, AnswerValue::MultiChoice(ids) if ids.iter().all(|id| has(choices, id)))
        }
        AnswerSpec::Boolean
        | AnswerSpec::Text
        | AnswerSpec::Date { .. }
        | AnswerSpec::Entity { .. }
        | AnswerSpec::EntityList { .. } => spec.answer_type() == answer.answer_type(),
    }
}

/// The decision that fills each role (E3).
#[must_use]
pub(crate) fn filling_decisions(document: &cairn_schema::Graph) -> BTreeMap<&RoleKey, &NodeKey> {
    let mut fillers = BTreeMap::new();
    for node in document.nodes.values() {
        if let Payload::Decision(decision) = &node.payload {
            match &decision.answer {
                AnswerSpec::Entity {
                    fills_role: Some(role),
                }
                | AnswerSpec::EntityList {
                    fills_role: Some(role),
                } => {
                    fillers.insert(role, &node.key);
                }
                AnswerSpec::Entity { fills_role: None }
                | AnswerSpec::EntityList { fills_role: None }
                | AnswerSpec::Boolean
                | AnswerSpec::SingleChoice(_)
                | AnswerSpec::MultiChoice(_)
                | AnswerSpec::Text
                | AnswerSpec::Date { .. } => {}
            }
        }
    }
    fillers
}

/// The decision that feeds each milestone (E3).
#[must_use]
pub(crate) fn feeding_decisions(document: &cairn_schema::Graph) -> BTreeMap<&NodeKey, &NodeKey> {
    let mut feeders = BTreeMap::new();
    for node in document.nodes.values() {
        if let Payload::Decision(decision) = &node.payload
            && let AnswerSpec::Date {
                feeds_milestone: Some(milestone),
            } = &decision.answer
        {
            feeders.insert(milestone, &node.key);
        }
    }
    feeders
}

/// E3: a role with a filling decision is filled only through it; A6: a single role holds one
/// entity.
fn role_fills(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let fillers = filling_decisions(document);
    for (role, entities) in &document.state.role_fills {
        let (code, message) = match document.roles.get(role) {
            None => (
                missing_role_code(document, role),
                "a role fill names a role that does not exist",
            ),
            Some(_) if fillers.contains_key(role) => (
                ViolationCode::FilledThroughDecision,
                "the role has a filling decision and is filled only through it (E3)",
            ),
            Some(found) if !found.multi && entities.len() > 1 => (
                ViolationCode::FillsRoleCardinality,
                "a single-valued role is filled by one entity (A6)",
            ),
            Some(_) => continue,
        };
        let mut found = violation(code, message);
        found.at.subject = Some(Subject::Role(role.clone()));
        out.push(found);
    }
}

/// E3: a milestone fed by a decision is pinned only through it; B6: a snooze waits on another
/// node that exists.
fn pins_and_snoozes(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let state: &JourneyState = &document.state;
    let feeders = feeding_decisions(document);
    for key in state.pins.keys() {
        if document.nodes.get(key).is_none() {
            out.push(state_without_node(check, key));
        } else if let Some(decision) = feeders.get(key) {
            let mut found = at_node(
                check.tree,
                key,
                ViolationCode::PinnedThroughDecision,
                "the milestone is pinned through its feeding decision (E3)",
            );
            found.related.push(Subject::Node((*decision).clone()));
            out.push(found);
        }
    }
    for (key, until) in &state.snoozes {
        if document.nodes.get(key).is_none() {
            out.push(state_without_node(check, key));
            continue;
        }
        let SnoozeTarget::Node(target) = until else {
            continue;
        };
        let code = if target == key {
            ViolationCode::SnoozeOnSelf
        } else if document.nodes.get(target).is_none() {
            missing_node_code(document, target)
        } else {
            continue;
        };
        let mut found = at_node(check.tree, key, code, "a snooze waits on another node (B6)");
        found.related.push(Subject::Node(target.clone()));
        out.push(found);
    }
}

/// Overrides, local-edit markers, and notes hang off nodes that exist; a tombstone names a
/// removed node, never a live one (B4).
fn attached_records(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let document = check.document;
    let state = &document.state;
    let keys = state.overrides.keys().chain(state.local_edits.keys());
    let notes = state
        .annotations
        .values()
        .filter_map(|annotation| annotation.body.node.as_ref());
    for key in keys.chain(notes) {
        if document.nodes.get(key).is_none() {
            out.push(state_without_node(check, key));
        }
    }
    for key in &state.tombstones {
        if document.nodes.get(key).is_some() {
            out.push(at_node(
                check.tree,
                key,
                ViolationCode::RetiredKeyReused,
                "a tombstoned node cannot come back (B4)",
            ));
        }
    }
}

/// PRACTICES, Explicit limits: each node, and the journey itself, holds at most
/// `note_count_per_node_max` notes and `link_count_per_node_max` links (G1).
fn annotation_counts(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    for (holder, exceeded) in check.document.state.annotation_counts_exceeded() {
        let mut found = match holder {
            Some(node) => at_node(
                check.tree,
                node,
                ViolationCode::LimitExceeded,
                format!(
                    "{} on the node, past {}",
                    exceeded.count,
                    exceeded.limit.name()
                ),
            ),
            None => violation(
                ViolationCode::LimitExceeded,
                format!(
                    "{} on the journey, past {}",
                    exceeded.count,
                    exceeded.limit.name()
                ),
            ),
        };
        found.limit = Some(exceeded.limit);
        out.push(found);
    }
}
