//! References (PRD Invariants: graph, references resolve and are of the right kind;
//! conditions; stage bounds; participations and roles; kind-restricted flags). Cost at the
//! limits: one pass over nodes, each looking up its at most 64 edges, 16 condition clauses,
//! 2 x 64 rule sources, and 32 participations by key, O(n log n) overall.

use std::collections::BTreeMap;

use cairn_schema::{
    AnswerSpec, Clause, ConditionValue, Date, DateSource, EntityKey, KeyRefs, Node, NodeField,
    NodeKey, NodeKind, ParticipationSource, Payload, ResourceContent, RoleKey, Segment, Subject,
    Violation, ViolationCode,
};

use super::{GraphCheck, at_node, missing_kind_code, missing_node_code, missing_role_code};

type GraphNode = Node<KeyRefs>;

/// Runs the reference checks.
pub(super) fn check(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    for node in check.document.nodes.values() {
        requires(check, node, out);
        condition(check, node, out);
        date_rules(check, node, out);
        stage_bounds(check, node, out);
        answer_spec(check, node, out);
        participations(check, node, out);
        resources(check, node, out);
    }
    one_per_graph(check, out);
    default_owner(check, out);
}

/// Looks up a node a reference names, reporting it when it does not resolve.
fn resolve<'a>(
    check: &GraphCheck<'a>,
    from: &GraphNode,
    target: &NodeKey,
    field: Option<NodeField>,
    out: &mut Vec<Violation>,
) -> Option<&'a GraphNode> {
    let found = check.document.nodes.get(target);
    if found.is_none() {
        let mut missing = at_node(
            check.tree,
            &from.key,
            missing_node_code(check.document, target),
            format!("the reference to {target} does not resolve"),
        );
        missing.at.field = field;
        missing.related.push(Subject::Node(target.clone()));
        out.push(missing);
    }
    found
}

fn related_to(mut found: Violation, field: Option<NodeField>, other: &NodeKey) -> Violation {
    found.at.field = field;
    found.related.push(Subject::Node(other.clone()));
    found
}

/// A3: an explicit edge connects two nodes that containment does not already relate; an
/// edge that only repeats what the node's own condition gates on is rejected (Invariants).
fn requires(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let tree = check.tree;
    let gated_on: Vec<&NodeKey> = node
        .relevant_when
        .as_ref()
        .map(cairn_schema::Condition::decisions)
        .unwrap_or_default();
    for requirement in node.requires.iter() {
        if resolve(check, node, requirement, None, out).is_none() {
            continue;
        }
        let (code, message) = if *requirement == node.key {
            (ViolationCode::DependencyCycle, "the node requires itself")
        } else if tree.is_ancestor(requirement, &node.key)
            || tree.is_ancestor(&node.key, requirement)
        {
            (
                ViolationCode::EdgeToAncestorOrDescendant,
                "an explicit edge cannot join a node to its ancestor or descendant (A3)",
            )
        } else if gated_on.contains(&requirement) {
            (
                ViolationCode::RequiresDuplicatesCondition,
                "the edge repeats the gate the node's own condition already puts on the decision",
            )
        } else {
            continue;
        };
        out.push(related_to(
            at_node(tree, &node.key, code, message),
            None,
            requirement,
        ));
    }
}

/// A5 and Invariants: a condition names decisions, of an answer type its clauses can compare,
/// outside the node's own subtree.
fn condition(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let Some(condition) = &node.relevant_when else {
        return;
    };
    let field = Some(NodeField::RelevantWhen);
    let mut stack = vec![condition.root()];
    while let Some(clause) = stack.pop() {
        let (decision, comparable) = match clause {
            Clause::All(children) | Clause::Any(children) => {
                stack.extend(children);
                continue;
            }
            Clause::Not(child) => {
                stack.push(child.as_ref());
                continue;
            }
            Clause::Answered(decision) => (decision, Comparable::Any),
            Clause::Equals(comparison) | Clause::NotEquals(comparison) => (
                &comparison.decision,
                Comparable::Equal(vec![&comparison.value]),
            ),
            Clause::In(membership) => (
                &membership.decision,
                Comparable::Equal(membership.values.as_slice().iter().collect()),
            ),
            Clause::Contains(comparison) => {
                (&comparison.decision, Comparable::Member(&comparison.value))
            }
        };
        let Some(target) = resolve(check, node, decision, field, out) else {
            continue;
        };
        let problem = if *decision == node.key || check.tree.is_ancestor(&node.key, decision) {
            Some((
                ViolationCode::ConditionOnOwnSubtree,
                "a node's condition cannot reference a decision inside its own subtree",
            ))
        } else {
            match &target.payload {
                Payload::Decision(spec) if comparable.fits(&spec.answer) => None,
                Payload::Decision(_) => Some((
                    ViolationCode::ConditionAnswerTypeMismatch,
                    "the clause compares a value the decision's answer type cannot hold",
                )),
                Payload::Deliverable(_)
                | Payload::Action(_)
                | Payload::Milestone(_)
                | Payload::Group(_) => Some((
                    ViolationCode::WrongReferenceKind,
                    "a condition references a decision",
                )),
            }
        };
        if let Some((code, message)) = problem {
            out.push(related_to(
                at_node(check.tree, &node.key, code, message),
                field,
                decision,
            ));
        }
    }
}

/// What a condition clause compares a decision's answer with.
enum Comparable<'a> {
    /// Answered or not: any answer type.
    Any,
    /// Equal to one of these values (`equals`, `not_equals`, `in`).
    Equal(Vec<&'a ConditionValue>),
    /// Holding this value (`contains`).
    Member(&'a ConditionValue),
}

impl Comparable<'_> {
    fn fits(&self, spec: &AnswerSpec<KeyRefs>) -> bool {
        match self {
            Comparable::Any => true,
            Comparable::Equal(values) => values.iter().all(|value| equal_fits(spec, value)),
            Comparable::Member(value) => member_fits(spec, value),
        }
    }
}

fn is_choice(choices: &cairn_schema::Choices, value: &ConditionValue) -> bool {
    match value {
        ConditionValue::Text(text) => choices
            .as_slice()
            .iter()
            .any(|choice| choice.id.as_str() == text.as_str()),
        ConditionValue::Boolean(_) => false,
    }
}

fn parses<T: std::str::FromStr>(value: &ConditionValue) -> bool {
    match value {
        ConditionValue::Text(text) => text.as_str().parse::<T>().is_ok(),
        ConditionValue::Boolean(_) => false,
    }
}

fn equal_fits(spec: &AnswerSpec<KeyRefs>, value: &ConditionValue) -> bool {
    match spec {
        AnswerSpec::Boolean => matches!(value, ConditionValue::Boolean(_)),
        AnswerSpec::SingleChoice(choices) => is_choice(choices, value),
        AnswerSpec::Text => matches!(value, ConditionValue::Text(_)),
        AnswerSpec::Date { .. } => parses::<Date>(value),
        AnswerSpec::Entity { .. } => parses::<EntityKey>(value),
        AnswerSpec::MultiChoice(_) | AnswerSpec::EntityList { .. } => false,
    }
}

fn member_fits(spec: &AnswerSpec<KeyRefs>, value: &ConditionValue) -> bool {
    match spec {
        AnswerSpec::MultiChoice(choices) => is_choice(choices, value),
        AnswerSpec::EntityList { .. } => parses::<EntityKey>(value),
        AnswerSpec::Text => matches!(value, ConditionValue::Text(_)),
        AnswerSpec::Boolean
        | AnswerSpec::SingleChoice(_)
        | AnswerSpec::Date { .. }
        | AnswerSpec::Entity { .. } => false,
    }
}

/// A8: a date rule's sources are milestones, date decisions, or `created_at`.
fn date_rules(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let rules = [
        (NodeField::DueBy, node.due_by.as_ref()),
        (NodeField::NotBefore, node.not_before.as_ref()),
    ];
    for (field, rule) in rules {
        let sources = rule.into_iter().flat_map(|rule| rule.sources.as_set());
        for source in sources {
            let DateSource::Node(source) = source else {
                continue;
            };
            let Some(target) = resolve(check, node, source, Some(field), out) else {
                continue;
            };
            let fits = match &target.payload {
                Payload::Milestone(_) => true,
                Payload::Decision(decision) => matches!(decision.answer, AnswerSpec::Date { .. }),
                Payload::Deliverable(_) | Payload::Action(_) | Payload::Group(_) => false,
            };
            if !fits {
                let found = at_node(
                    check.tree,
                    &node.key,
                    ViolationCode::WrongReferenceKind,
                    "a date rule measures from a milestone or a date decision (A8)",
                );
                out.push(related_to(found, Some(field), source));
            }
        }
    }
}

/// F4: stage bounds are milestones outside the stage.
fn stage_bounds(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let Payload::Group(group) = &node.payload else {
        return;
    };
    let bounds = [
        (NodeField::OpensAt, group.opens_at.as_ref()),
        (NodeField::ClosesAt, group.closes_at.as_ref()),
    ];
    for (field, bound) in bounds {
        let Some(bound) = bound else {
            continue;
        };
        let Some(target) = resolve(check, node, bound, Some(field), out) else {
            continue;
        };
        let problem = if target.kind() != NodeKind::Milestone {
            Some((
                ViolationCode::StageBoundNotMilestone,
                "a stage bound is a milestone (F4)",
            ))
        } else if check.tree.is_ancestor(&node.key, bound) {
            Some((
                ViolationCode::EdgeToAncestorOrDescendant,
                "a stage bound sits outside the stage (F4)",
            ))
        } else {
            None
        };
        if let Some((code, message)) = problem {
            let found = at_node(check.tree, &node.key, code, message);
            out.push(related_to(found, Some(field), bound));
        }
    }
}

/// A6 and E3: `fills_role` names a role of matching cardinality; `feeds_milestone` names a
/// milestone.
fn answer_spec(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let Payload::Decision(decision) = &node.payload else {
        return;
    };
    match &decision.answer {
        AnswerSpec::Entity {
            fills_role: Some(role),
        } => fills_role(check, node, role, false, out),
        AnswerSpec::EntityList {
            fills_role: Some(role),
        } => fills_role(check, node, role, true, out),
        AnswerSpec::Date {
            feeds_milestone: Some(milestone),
        } => {
            let field = Some(NodeField::FeedsMilestone);
            let Some(target) = resolve(check, node, milestone, field, out) else {
                return;
            };
            if target.kind() != NodeKind::Milestone {
                let found = at_node(
                    check.tree,
                    &node.key,
                    ViolationCode::FeedsMilestoneNotMilestone,
                    "a date decision feeds a milestone (E3)",
                );
                out.push(related_to(found, field, milestone));
            }
        }
        AnswerSpec::Entity { fills_role: None }
        | AnswerSpec::EntityList { fills_role: None }
        | AnswerSpec::Date {
            feeds_milestone: None,
        }
        | AnswerSpec::Boolean
        | AnswerSpec::SingleChoice(_)
        | AnswerSpec::MultiChoice(_)
        | AnswerSpec::Text => {}
    }
}

fn fills_role(
    check: &GraphCheck<'_>,
    node: &GraphNode,
    role: &RoleKey,
    list: bool,
    out: &mut Vec<Violation>,
) {
    let (code, message) = match check.document.roles.get(role) {
        None => (
            missing_role_code(check.document, role),
            "the role this decision fills does not exist",
        ),
        Some(found) if found.multi != list => (
            ViolationCode::FillsRoleCardinality,
            "an entity decision fills a single role, an entity-list decision a multi role (A6)",
        ),
        Some(_) => return,
    };
    let mut found = at_node(check.tree, &node.key, code, message);
    found.at.field = Some(NodeField::FillsRole);
    found.related.push(Subject::Role(role.clone()));
    out.push(found);
}

/// A7 and E2: participations use declared kinds, and a single-valued kind holds one entity.
fn participations(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let document = check.document;
    for (kind, source) in node.participations.as_map() {
        let multi_kind = if *kind == cairn_schema::KindKey::owner() {
            false
        } else if let Some(declared) = document.participation_kinds.get(kind) {
            declared.multi
        } else {
            let mut found = at_node(
                check.tree,
                &node.key,
                missing_kind_code(document, kind),
                format!("the participation kind {kind} is not declared (A7)"),
            );
            found.related.push(Subject::Kind(kind.clone()));
            out.push(found);
            continue;
        };
        let (code, message, related) = match source {
            ParticipationSource::Role(role) => match document.roles.get(role) {
                None => (
                    missing_role_code(document, role),
                    "the role this participation names does not exist",
                    Subject::Role(role.clone()),
                ),
                Some(found) if found.multi && !multi_kind => (
                    ViolationCode::SingleKindOnMultiRole,
                    "a single-valued kind cannot take a multi-valued role (A7)",
                    Subject::Role(role.clone()),
                ),
                Some(_) => continue,
            },
            ParticipationSource::Entities(entities) if entities.len() > 1 && !multi_kind => (
                ViolationCode::SingleKindOnMultiRole,
                "a single-valued kind holds at most one entity (A7)",
                Subject::Kind(kind.clone()),
            ),
            ParticipationSource::Entities(_) => continue,
        };
        let mut found = at_node(check.tree, &node.key, code, message);
        found.related.push(related);
        out.push(found);
    }
}

/// A10: a message draft's placeholders name roles and decisions that exist.
fn resources(check: &GraphCheck<'_>, node: &GraphNode, out: &mut Vec<Violation>) {
    let document = check.document;
    for resource in &node.resources {
        let ResourceContent::MessageDraft(template) = &resource.content else {
            continue;
        };
        for segment in template.segments() {
            match segment {
                Segment::RoleName(role) if document.roles.get(role).is_none() => {
                    let mut found = at_node(
                        check.tree,
                        &node.key,
                        missing_role_code(document, role),
                        "a message draft names a role that does not exist",
                    );
                    found.related.push(Subject::Role(role.clone()));
                    out.push(found);
                }
                Segment::Answer(decision) => {
                    let target = resolve(check, node, decision, None, out);
                    if target.is_some_and(|target| target.kind() != NodeKind::Decision) {
                        let found = at_node(
                            check.tree,
                            &node.key,
                            ViolationCode::WrongReferenceKind,
                            "a message draft's answer placeholder names a decision",
                        );
                        out.push(related_to(found, None, decision));
                    }
                }
                Segment::Text(_) | Segment::Journey(_) | Segment::RoleName(_) => {}
            }
        }
    }
}

/// Invariants: at most one `final` milestone per graph, one filling decision per role, and
/// one feeding decision per milestone.
fn one_per_graph(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let mut finals: Vec<&NodeKey> = Vec::new();
    let mut fillers: BTreeMap<&RoleKey, Vec<&NodeKey>> = BTreeMap::new();
    let mut feeders: BTreeMap<&NodeKey, Vec<&NodeKey>> = BTreeMap::new();
    for node in check.document.nodes.values() {
        match &node.payload {
            Payload::Milestone(milestone) if milestone.is_final => finals.push(&node.key),
            Payload::Decision(decision) => match &decision.answer {
                AnswerSpec::Entity {
                    fills_role: Some(role),
                }
                | AnswerSpec::EntityList {
                    fills_role: Some(role),
                } => {
                    fillers.entry(role).or_default().push(&node.key);
                }
                AnswerSpec::Date {
                    feeds_milestone: Some(milestone),
                } => {
                    feeders.entry(milestone).or_default().push(&node.key);
                }
                AnswerSpec::Entity { fills_role: None }
                | AnswerSpec::EntityList { fills_role: None }
                | AnswerSpec::Date {
                    feeds_milestone: None,
                }
                | AnswerSpec::Boolean
                | AnswerSpec::SingleChoice(_)
                | AnswerSpec::MultiChoice(_)
                | AnswerSpec::Text => {}
            },
            Payload::Milestone(_)
            | Payload::Deliverable(_)
            | Payload::Action(_)
            | Payload::Group(_) => {}
        }
    }
    let groups = std::iter::once((ViolationCode::SeveralFinalMilestones, finals))
        .chain(
            fillers
                .into_values()
                .map(|keys| (ViolationCode::SeveralFillingDecisions, keys)),
        )
        .chain(
            feeders
                .into_values()
                .map(|keys| (ViolationCode::SeveralFeedingDecisions, keys)),
        );
    for (code, keys) in groups {
        let Some((first, rest)) = keys.split_first() else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let mut found = at_node(
            check.tree,
            first,
            code,
            "at most one per graph (Invariants)",
        );
        found.related = rest
            .iter()
            .map(|key| Subject::Node((*key).clone()))
            .collect();
        out.push(found);
    }
}

/// A6: `default_owner` names an existing single-valued role (owner is single).
fn default_owner(check: &GraphCheck<'_>, out: &mut Vec<Violation>) {
    let Some(role) = &check.document.default_owner else {
        return;
    };
    let (code, message) = match check.document.roles.get(role) {
        None => (
            missing_role_code(check.document, role),
            "the default owner role does not exist",
        ),
        Some(found) if found.multi => (
            ViolationCode::SingleKindOnMultiRole,
            "the default owner is single-valued and cannot take a multi-valued role (A6, A7)",
        ),
        Some(_) => return,
    };
    let mut found = super::violation(code, message);
    found.at.subject = Some(Subject::Role(role.clone()));
    out.push(found);
}
