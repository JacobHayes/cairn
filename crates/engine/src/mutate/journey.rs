//! Journey state (B2, B5, B6, B10, D1, D1a, D4, E3, F1, F2, F5, G1): transitions by each kind's
//! table, answers, recorded dates, direct role fills, pins, snoozes, overrides, atomic
//! placeholders, notes and links, provenance, and local-edit markers.

use cairn_schema::{
    AnswerValue, GraphKey, GraphRecord, Limit, LocalEdit, Markdown, Mutation, NodeKey, NodeKind,
    NodeState, OverrideKind, Overrides, Payload, RecordedEnd, RoleKey, SignedDays, State, Subject,
    Transition, ViolationCode, Write,
};

use super::Session;
use crate::transition::{self, Move};
use crate::validate::state::{feeding_decisions, filling_decisions};

/// Applies a journey state mutation.
pub(super) fn apply(session: &mut Session<'_>, mutation: &Mutation) -> Vec<Write> {
    match mutation {
        Mutation::Transition { node, transition } => move_node(session, node, transition),
        Mutation::Answer {
            decision,
            value,
            rationale,
        } => answer(session, decision, value, rationale.as_ref()),
        Mutation::SetRecordedDate { node, end, date } => recorded_date(session, node, *end, *date),
        Mutation::FillRole { role, entities } => {
            if !direct_fill(session, role) {
                return Vec::new();
            }
            vec![session.put(GraphRecord::RoleFill {
                role: role.clone(),
                entities: entities.clone(),
            })]
        }
        Mutation::ClearRoleFill { role } => clear_fill(session, role),
        Mutation::SetPin { node, date } => match direct_pin(session, node, false) {
            Ok(_) => vec![session.put(GraphRecord::Pin {
                node: node.clone(),
                date: *date,
            })],
            Err(Rejected) => Vec::new(),
        },
        Mutation::ShiftPin { node, offset_days } => shift_pin(session, node, *offset_days),
        Mutation::ClearPin { node } => match direct_pin(session, node, true) {
            Ok(_) => vec![session.remove(GraphKey::Pin(node.clone()))],
            Err(Rejected) => Vec::new(),
        },
        Mutation::Snooze { node, until } => snooze(session, node, until),
        Mutation::Unsnooze { node } => match stored(session, node) {
            Some(_) if snoozed(session, node) => {
                vec![session.remove(GraphKey::Snooze(node.clone()))]
            }
            Some(_) => {
                session.reject(
                    ViolationCode::UnresolvedReference,
                    Some(node),
                    "the node is not snoozed",
                );
                Vec::new()
            }
            None => Vec::new(),
        },
        Mutation::ApplyOverride { node, applied } => apply_override(session, node, applied),
        Mutation::RemoveOverride { node, kind } => remove_override(session, node, *kind),
        Mutation::SetAtomic { node, atomic } => set_atomic(session, node, *atomic),
        other => super::annotations::apply(session, other),
    }
}

/// The node's stored state, or a violation when it has none.
pub(super) fn stored(session: &mut Session<'_>, node: &NodeKey) -> Option<NodeState> {
    if !session.require(node) {
        return None;
    }
    let found = session
        .journey()
        .and_then(|journey| journey.graph.state.nodes.get(node))
        .cloned();
    if found.is_none() {
        session.reject(
            ViolationCode::StateNotOnKind,
            Some(node),
            "the node has no stored state",
        );
    }
    found
}

fn snoozed(session: &Session<'_>, node: &NodeKey) -> bool {
    session
        .journey()
        .is_some_and(|journey| journey.graph.state.snoozes.contains_key(node))
}

fn kind_of(session: &Session<'_>, node: &NodeKey) -> Option<NodeKind> {
    session
        .graph()
        .and_then(|graph| graph.nodes.get(node))
        .map(cairn_schema::Node::kind)
}

/// D1: a transition by the node's own table, recording or clearing the dates it moves (F1,
/// F2); any transition lifts a snooze in the same event (B6); reopening a decision clears its
/// answer, and with it the role fill or pin the answer produced (E3). A container's skip is
/// this one event: the cascade is derived (D1a).
fn move_node(session: &mut Session<'_>, node: &NodeKey, transition: &Transition) -> Vec<Write> {
    let Some(stored) = stored(session, node) else {
        return Vec::new();
    };
    let Some(kind) = kind_of(session, node) else {
        unreachable!("the node exists")
    };
    let step = Move::of(transition);
    let Some(to) = transition::next(kind, stored.state, step) else {
        let message = format!(
            "a {} in {:?} cannot {step:?} (D1)",
            kind.name(),
            stored.state
        );
        session.reject(ViolationCode::IllegalTransition, Some(node), message);
        return Vec::new();
    };
    assert!(
        to.legal_for(kind) && stored.state.legal_for(kind),
        "D1: within the kind's machine"
    );
    let today = session.inputs.today;
    let mut next = NodeState {
        state: to,
        ..stored.clone()
    };
    match step {
        Move::Start => next.started_on = Some(today),
        Move::Stop => next.started_on = None,
        Move::Complete | Move::Reach => next.finished_on = Some(today),
        Move::Reopen => {
            next.started_on = None;
            next.finished_on = None;
            next.skip_reason = None;
        }
        Move::Skip | Move::Answer => {}
    }
    if let Transition::Skip { reason } = transition {
        // D1: the reason a skip requires is kept with the skip, and goes when it is reopened.
        next.skip_reason = Some(reason.clone());
    }
    assert_eq!(next.skip_reason.is_some(), next.state == State::Skipped);
    note_guarded(session, node, kind, stored.state, step);
    let answered = session
        .journey()
        .is_some_and(|journey| journey.graph.state.answers.contains_key(node));
    let mut writes = vec![session.put(GraphRecord::NodeState {
        node: node.clone(),
        state: next,
    })];
    if snoozed(session, node) {
        writes.push(session.remove(GraphKey::Snooze(node.clone())));
    }
    if step == Move::Reopen && answered {
        // Reopening clears the answer and its rationale together (B2).
        writes.push(session.remove(GraphKey::Answer(node.clone())));
    }
    if step == Move::Reopen {
        writes.extend(clear_bypass(session, node));
    }
    writes
}

/// D4: reopening a node clears its guard bypass, which accepted the failures of the
/// completion it covered and of no later one.
fn clear_bypass(session: &Session<'_>, node: &NodeKey) -> Option<Write> {
    let mut next = overrides(session, node);
    next.bypass.take()?;
    Some(if next.is_empty() {
        session.remove(GraphKey::Overrides(node.clone()))
    } else {
        session.put(GraphRecord::Overrides {
            node: node.clone(),
            overrides: next,
        })
    })
}

/// D4: remembers a guarded transition, for the guards stage and any bypass covering it.
fn note_guarded(
    session: &mut Session<'_>,
    node: &NodeKey,
    kind: NodeKind,
    from: State,
    step: Move,
) {
    if transition::is_guarded(kind, from, step) {
        session.guarded.insert(node.clone(), session.ordinal);
    }
    if step == Move::Complete {
        session.completed.insert(node.clone(), session.ordinal);
    }
}

/// B2, D1: answering an open decision decides it on today's date; revising a decided one
/// keeps that date. The answer's rationale replaces the previous one outright, so a
/// revision that gives none leaves the decision with none. The answer must match the decision's type (an invariant the state stage
/// checks on the graph the patch produces).
fn answer(
    session: &mut Session<'_>,
    decision: &NodeKey,
    value: &AnswerValue,
    rationale: Option<&Markdown>,
) -> Vec<Write> {
    let Some(stored) = stored(session, decision) else {
        return Vec::new();
    };
    let Some(kind) = kind_of(session, decision) else {
        unreachable!("the node exists")
    };
    if kind != NodeKind::Decision {
        session.reject(
            ViolationCode::WrongReferenceKind,
            Some(decision),
            "only a decision is answered",
        );
        return Vec::new();
    }
    let Some(to) = transition::answered(stored.state) else {
        let message = format!(
            "a decision in {:?} cannot be answered; reopen it first (D1)",
            stored.state
        );
        session.reject(ViolationCode::IllegalTransition, Some(decision), message);
        return Vec::new();
    };
    assert_eq!(kind, NodeKind::Decision);
    assert_eq!(to, State::Decided, "an answer decides (D1)");
    note_guarded(session, decision, kind, stored.state, Move::Answer);
    let mut writes = vec![session.put(GraphRecord::Answer {
        decision: decision.clone(),
        value: value.clone(),
        // The rationale belongs to this answer alone: none given is none kept (B2).
        rationale: rationale.cloned(),
    })];
    if stored.state != to {
        let next = NodeState {
            state: to,
            finished_on: Some(session.inputs.today),
            ..stored
        };
        writes.push(session.put(GraphRecord::NodeState {
            node: decision.clone(),
            state: next,
        }));
    }
    if snoozed(session, decision) {
        writes.push(session.remove(GraphKey::Snooze(decision.clone())));
    }
    writes
}

/// F1, F2: a recorded start or finish date is edited, where one is recorded.
fn recorded_date(
    session: &mut Session<'_>,
    node: &NodeKey,
    end: RecordedEnd,
    date: cairn_schema::Date,
) -> Vec<Write> {
    let Some(mut next) = stored(session, node) else {
        return Vec::new();
    };
    let slot = match end {
        RecordedEnd::Start => &mut next.started_on,
        RecordedEnd::Finish => &mut next.finished_on,
    };
    if slot.is_none() {
        session.reject(
            ViolationCode::IllegalTransition,
            Some(node),
            format!("the node has no recorded {end:?} date to edit"),
        );
        return Vec::new();
    }
    assert!(slot.is_some(), "only a recorded date is edited");
    *slot = Some(date);
    vec![session.put(GraphRecord::NodeState {
        node: node.clone(),
        state: next,
    })]
}

/// E3: a role is filled directly only when no decision fills it.
fn direct_fill(session: &mut Session<'_>, role: &RoleKey) -> bool {
    let Some(graph) = session.graph() else {
        unreachable!("a journey patch has a graph")
    };
    let problem = if graph.roles.get(role).is_none() {
        Some((
            ViolationCode::UnresolvedReference,
            "no such role".to_owned(),
        ))
    } else {
        filling_decisions(graph).get(role).map(|decision| {
            (
                ViolationCode::FilledThroughDecision,
                format!("the role is filled by answering {decision} (E3)"),
            )
        })
    };
    if let Some((code, message)) = problem {
        session.reject(code, None, message);
        if let Some(found) = session.violations.last_mut() {
            found.at.subject = Some(Subject::Role(role.clone()));
        }
        return false;
    }
    true
}

fn clear_fill(session: &mut Session<'_>, role: &RoleKey) -> Vec<Write> {
    let filled = session
        .journey()
        .is_some_and(|journey| journey.graph.state.role_fills.contains_key(role));
    if !direct_fill(session, role) {
        return Vec::new();
    }
    if !filled {
        session.reject(
            ViolationCode::UnresolvedReference,
            None,
            format!("role {role} is not filled directly"),
        );
        return Vec::new();
    }
    vec![session.remove(GraphKey::RoleFill(role.clone()))]
}

/// A mutation found unable to apply (its violation is recorded).
struct Rejected;

/// E3, F5: a node's pin, edited directly unless a decision feeds it. With `existing`, the
/// node must hold a pin; returns the pin it holds.
fn direct_pin(
    session: &mut Session<'_>,
    node: &NodeKey,
    existing: bool,
) -> Result<Option<cairn_schema::Date>, Rejected> {
    if !session.require(node) {
        return Err(Rejected);
    }
    let Some(graph) = session.graph() else {
        unreachable!("a journey patch has a graph")
    };
    if let Some(decision) = feeding_decisions(graph).get(node) {
        let message = format!("the milestone is pinned by answering {decision} (E3, F5)");
        session.reject(ViolationCode::PinnedThroughDecision, Some(node), message);
        return Err(Rejected);
    }
    let pin = graph.state.pins.get(node).copied();
    if existing && pin.is_none() {
        session.reject(
            ViolationCode::UnresolvedReference,
            Some(node),
            "the node has no pin",
        );
        return Err(Rejected);
    }
    Ok(pin)
}

/// F5 `shift`: a pin moved by whole days.
fn shift_pin(session: &mut Session<'_>, node: &NodeKey, offset_days: SignedDays) -> Vec<Write> {
    let Ok(Some(pin)) = direct_pin(session, node, true) else {
        return Vec::new();
    };
    let span = jiff::Span::new().days(i64::from(offset_days.get()));
    let Ok(date) = pin.checked_add(span) else {
        session.reject(
            ViolationCode::LimitExceeded,
            Some(node),
            "the shifted date is out of range",
        );
        if let Some(found) = session.violations.last_mut() {
            found.limit = Some(Limit::OffsetDays);
        }
        return Vec::new();
    };
    vec![session.put(GraphRecord::Pin {
        node: node.clone(),
        date,
    })]
}

/// B6: only a node that can be acted on is snoozed, never on itself. Its kind and state are
/// checked here; whether it is in scope is checked on the graph the patch produces (the
/// derived stage), and a blocked node may be snoozed (Gating).
fn snooze(
    session: &mut Session<'_>,
    node: &NodeKey,
    until: &cairn_schema::SnoozeTarget,
) -> Vec<Write> {
    let Some(stored) = stored(session, node) else {
        return Vec::new();
    };
    let kind = kind_of(session, node);
    let (code, message) = if kind == Some(NodeKind::Group) || stored.state.is_terminal() {
        (
            ViolationCode::SnoozeNotActionable,
            "only an actionable node is snoozed (B6)",
        )
    } else if *until == cairn_schema::SnoozeTarget::Node(node.clone()) {
        (
            ViolationCode::SnoozeOnSelf,
            "a node cannot be snoozed until itself (B6)",
        )
    } else {
        // B6: whether the node is relevant is checked on the graph the patch produces.
        session.snoozed.insert(node.clone(), session.ordinal);
        return vec![session.put(GraphRecord::Snooze {
            node: node.clone(),
            until: until.clone(),
        })];
    };
    session.reject(code, Some(node), message);
    Vec::new()
}

fn overrides(session: &Session<'_>, node: &NodeKey) -> Overrides {
    session
        .journey()
        .and_then(|journey| journey.graph.state.overrides.get(node).cloned())
        .unwrap_or_default()
}

/// B5, D1a, D4: an override with its reason. A guard bypass covers a guarded transition in
/// the same patch; the guards stage records the failures it bypassed.
fn apply_override(
    session: &mut Session<'_>,
    node: &NodeKey,
    applied: &cairn_schema::Override,
) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let mut next = overrides(session, node);
    match applied {
        cairn_schema::Override::ForceInclude { reason } => {
            next.force_include = Some(reason.clone());
        }
        cairn_schema::Override::Keep { reason } => next.keep = Some(reason.clone()),
        cairn_schema::Override::GuardBypass { guards, reason } => {
            next.bypass = Some(cairn_schema::Bypass {
                guards: guards.clone(),
                reason: reason.clone(),
                failures: std::collections::BTreeSet::new(),
            });
            session.bypassed.insert(node.clone(), session.ordinal);
        }
    }
    assert!(!next.is_empty(), "an applied override is stored");
    vec![session.put(GraphRecord::Overrides {
        node: node.clone(),
        overrides: next,
    })]
}

fn remove_override(session: &mut Session<'_>, node: &NodeKey, kind: OverrideKind) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let mut next = overrides(session, node);
    let removed = match kind {
        OverrideKind::ForceInclude => next.force_include.take().is_some(),
        OverrideKind::Keep => next.keep.take().is_some(),
        OverrideKind::GuardBypass => next.bypass.take().is_some(),
    };
    if !removed {
        session.reject(
            ViolationCode::UnresolvedReference,
            Some(node),
            format!("the node has no {kind:?} override"),
        );
        return Vec::new();
    }
    vec![if next.is_empty() {
        session.remove(GraphKey::Overrides(node.clone()))
    } else {
        session.put(GraphRecord::Overrides {
            node: node.clone(),
            overrides: next,
        })
    }]
}

/// B10: a placeholder marked atomic completes without children.
fn set_atomic(session: &mut Session<'_>, node: &NodeKey, atomic: bool) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let placeholder = session
        .node(node)
        .is_some_and(|found| match &found.payload {
            Payload::Deliverable(deliverable) => deliverable.placeholder,
            Payload::Action(action) => action.placeholder,
            Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => false,
        });
    if !placeholder {
        session.reject(
            ViolationCode::FieldNotOnKind,
            Some(node),
            "only a placeholder is marked atomic (B10)",
        );
        return Vec::new();
    }
    let Some(next) = stored(session, node) else {
        return Vec::new();
    };
    assert!(placeholder, "B10: atomic only on placeholders");
    vec![session.put(GraphRecord::NodeState {
        node: node.clone(),
        state: NodeState { atomic, ..next },
    })]
}

/// The marker a reset to the route clears (B4), or an explicit marker.
pub(super) fn local_edit(
    session: &mut Session<'_>,
    node: &NodeKey,
    edit: &LocalEdit,
    marked: bool,
) -> Vec<Write> {
    if !session.require(node) {
        return Vec::new();
    }
    let present = session.journey().is_some_and(|journey| {
        let edits = journey.graph.state.local_edits.get(node);
        edits.is_some_and(|edits| edits.contains(edit))
    });
    if !marked && !present {
        session.reject(
            ViolationCode::UnresolvedReference,
            Some(node),
            "the node has no such local edit",
        );
        return Vec::new();
    }
    vec![if marked {
        session.put(GraphRecord::LocalEdit {
            node: node.clone(),
            edit: edit.clone(),
        })
    } else {
        session.remove(GraphKey::LocalEdit {
            node: node.clone(),
            edit: edit.clone(),
        })
    }]
}
