//! Three-valued condition evaluation (A5; PRD Gating: Kleene evaluation). Each referenced
//! decision contributes a [`Term`]: an answer that is in effect, unanswered, or unknown (a
//! decision still open and relevant). `answered` is true for an answer and false for
//! unanswered; the value operators (`equals`, `not_equals`, `in`, `contains`) are false when
//! unanswered; `all`, `any`, and `not` compose in Kleene's logic, so unknown survives only
//! where the open decision could still change the outcome.
//!
//! Evaluation is iterative, with an explicit stack bounded by the condition's own limits
//! (8 levels, 16 clauses: PRACTICES, No recursion), so one condition costs at most a few
//! dozen steps. Entity values compare through aliases (E6), so a merge can change what a
//! condition evaluates to.

use cairn_schema::{
    AnswerValue, Clause, Condition, ConditionValue, Deployment, EntityKey, KeyRefs, NodeKey,
};
use jiff::civil::Date;

use crate::entity;

/// A truth value in Kleene's three-valued logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Truth {
    /// Holds.
    True,
    /// Does not hold.
    False,
    /// Depends on a decision still open and relevant.
    Unknown,
}

impl Truth {
    fn from_bool(value: bool) -> Self {
        if value { Truth::True } else { Truth::False }
    }

    fn and(self, other: Truth) -> Truth {
        match (self, other) {
            (Truth::False, _) | (_, Truth::False) => Truth::False,
            (Truth::Unknown, _) | (_, Truth::Unknown) => Truth::Unknown,
            (Truth::True, Truth::True) => Truth::True,
        }
    }

    fn or(self, other: Truth) -> Truth {
        match (self, other) {
            (Truth::True, _) | (_, Truth::True) => Truth::True,
            (Truth::Unknown, _) | (_, Truth::Unknown) => Truth::Unknown,
            (Truth::False, Truth::False) => Truth::False,
        }
    }

    fn not(self) -> Truth {
        match self {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }
    }
}

/// What a referenced decision contributes to a condition (Gating).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Term<'a> {
    /// Decided and relevant: its answer is in effect.
    Answered(&'a AnswerValue),
    /// Skipped (stored or effectively), not relevant, or itself undecided.
    Unanswered,
    /// Open and relevant: the answer is still to come.
    Unknown,
}

/// A combination waiting for its operands' values.
enum Combine {
    All(usize),
    Any(usize),
    Not,
}

/// One step of the evaluation: a clause to visit, or a combination whose operands are on
/// the value stack.
enum Step<'c> {
    Visit(&'c Clause<KeyRefs>),
    Combine(Combine),
}

/// A5: evaluates a condition, each referenced decision contributing `term(decision)`.
///
/// # Panics
///
/// Never for a condition that passed parsing: every combination has its operands.
#[must_use]
pub(crate) fn evaluate<'a>(
    condition: &Condition<KeyRefs>,
    term: impl Fn(&NodeKey) -> Term<'a>,
    deployment: &Deployment,
) -> Truth {
    let mut steps = vec![Step::Visit(condition.root())];
    let mut values: Vec<Truth> = Vec::new();
    while let Some(step) = steps.pop() {
        match step {
            Step::Visit(Clause::All(children)) => {
                steps.push(Step::Combine(Combine::All(children.len())));
                steps.extend(children.iter().map(Step::Visit));
            }
            Step::Visit(Clause::Any(children)) => {
                steps.push(Step::Combine(Combine::Any(children.len())));
                steps.extend(children.iter().map(Step::Visit));
            }
            Step::Visit(Clause::Not(child)) => {
                steps.push(Step::Combine(Combine::Not));
                steps.push(Step::Visit(child));
            }
            Step::Visit(leaf) => values.push(leaf_truth(leaf, &term, deployment)),
            Step::Combine(combine) => {
                let (count, start, fold): (usize, Truth, fn(Truth, Truth) -> Truth) = match combine
                {
                    Combine::All(count) => (count, Truth::True, Truth::and),
                    Combine::Any(count) => (count, Truth::False, Truth::or),
                    Combine::Not => (1, Truth::True, |_, operand| operand.not()),
                };
                let split = values.len().checked_sub(count);
                assert!(split.is_some(), "a combination's operands were evaluated");
                let operands = values.split_off(split.unwrap_or_default());
                values.push(operands.into_iter().fold(start, fold));
            }
        }
    }
    assert_eq!(values.len(), 1, "a condition evaluates to one value");
    values.pop().unwrap_or(Truth::Unknown)
}

fn leaf_truth<'a>(
    clause: &Clause<KeyRefs>,
    term: &impl Fn(&NodeKey) -> Term<'a>,
    deployment: &Deployment,
) -> Truth {
    match clause {
        Clause::Answered(decision) => match term(decision) {
            Term::Answered(_) => Truth::True,
            Term::Unanswered => Truth::False,
            Term::Unknown => Truth::Unknown,
        },
        Clause::Equals(comparison) => value_test(term(&comparison.decision), |answer| {
            equal(answer, &comparison.value, deployment)
        }),
        Clause::NotEquals(comparison) => value_test(term(&comparison.decision), |answer| {
            !equal(answer, &comparison.value, deployment)
        }),
        Clause::In(membership) => value_test(term(&membership.decision), |answer| {
            membership
                .values
                .as_slice()
                .iter()
                .any(|value| equal(answer, value, deployment))
        }),
        Clause::Contains(comparison) => value_test(term(&comparison.decision), |answer| {
            contains(answer, &comparison.value, deployment)
        }),
        Clause::All(_) | Clause::Any(_) | Clause::Not(_) => {
            unreachable!("combinations are visited, not tested")
        }
    }
}

/// A value operator: false when unanswered (Gating), unknown while the answer is to come.
fn value_test(term: Term<'_>, test: impl Fn(&AnswerValue) -> bool) -> Truth {
    match term {
        Term::Answered(answer) => Truth::from_bool(test(answer)),
        Term::Unanswered => Truth::False,
        Term::Unknown => Truth::Unknown,
    }
}

/// The entity a key names once aliases are followed (E6); an unknown key stands for itself.
fn canonical<'a>(deployment: &'a Deployment, key: &'a EntityKey) -> &'a EntityKey {
    entity::resolve(deployment, key).unwrap_or(key)
}

fn same_entity(deployment: &Deployment, answer: &EntityKey, value: &ConditionValue) -> bool {
    match value {
        ConditionValue::Text(text) => text
            .as_str()
            .parse::<EntityKey>()
            .is_ok_and(|key| canonical(deployment, &key) == canonical(deployment, answer)),
        ConditionValue::Boolean(_) => false,
    }
}

/// `equals` for each answer type a clause may compare (validation holds the types; a value
/// the answer cannot hold is simply unequal).
fn equal(answer: &AnswerValue, value: &ConditionValue, deployment: &Deployment) -> bool {
    match (answer, value) {
        (AnswerValue::Boolean(answer), ConditionValue::Boolean(value)) => answer == value,
        (AnswerValue::SingleChoice(choice), ConditionValue::Text(text)) => {
            choice.as_str() == text.as_str()
        }
        (AnswerValue::Text(answer), ConditionValue::Text(text)) => answer.as_str() == text.as_str(),
        (AnswerValue::Date(date), ConditionValue::Text(text)) => text
            .as_str()
            .parse::<Date>()
            .is_ok_and(|value| value == *date),
        (AnswerValue::Entity(entity), value) => same_entity(deployment, entity, value),
        (
            AnswerValue::Boolean(_)
            | AnswerValue::SingleChoice(_)
            | AnswerValue::Text(_)
            | AnswerValue::Date(_)
            | AnswerValue::MultiChoice(_)
            | AnswerValue::EntityList(_),
            _,
        ) => false,
    }
}

/// `contains`: a multi-choice answer holds the choice, an entity list the entity, a text
/// answer the text.
fn contains(answer: &AnswerValue, value: &ConditionValue, deployment: &Deployment) -> bool {
    match (answer, value) {
        (AnswerValue::MultiChoice(choices), ConditionValue::Text(text)) => choices
            .iter()
            .any(|choice| choice.as_str() == text.as_str()),
        (AnswerValue::EntityList(entities), value) => entities
            .iter()
            .any(|entity| same_entity(deployment, entity, value)),
        (AnswerValue::Text(answer), ConditionValue::Text(text)) => {
            answer.as_str().contains(text.as_str())
        }
        (
            AnswerValue::Boolean(_)
            | AnswerValue::SingleChoice(_)
            | AnswerValue::Text(_)
            | AnswerValue::Date(_)
            | AnswerValue::Entity(_)
            | AnswerValue::MultiChoice(_),
            _,
        ) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_schema::from_json;

    fn condition(json: &str) -> Condition<KeyRefs> {
        from_json(json).unwrap()
    }

    fn key(text: &str) -> NodeKey {
        text.parse().unwrap()
    }

    /// Evaluates with `n_a` contributing `a` and every other decision unanswered.
    fn with(json: &str, a: Term<'_>) -> Truth {
        let term = |decision: &NodeKey| {
            if *decision == key("n_a") {
                a
            } else {
                Term::Unanswered
            }
        };
        evaluate(&condition(json), term, &Deployment::default())
    }

    #[test]
    fn kleene_tables_over_one_open_decision() {
        let yes = AnswerValue::Boolean(true);
        let eq = r#"{"equals": {"decision": "n_a", "value": true}}"#;
        let ne = r#"{"not_equals": {"decision": "n_a", "value": true}}"#;
        let not_eq = r#"{"not": {"equals": {"decision": "n_a", "value": true}}}"#;
        let answered = r#"{"answered": "n_a"}"#;
        let and_false = r#"{"all": [{"answered": "n_a"}, {"answered": "n_b"}]}"#;
        let or_true = r#"{"any": [{"answered": "n_a"}, {"not": {"answered": "n_b"}}]}"#;
        let cases: &[(&str, Term<'_>, Truth)] = &[
            (eq, Term::Answered(&yes), Truth::True),
            (eq, Term::Unanswered, Truth::False),
            (eq, Term::Unknown, Truth::Unknown),
            (ne, Term::Unanswered, Truth::False),
            (not_eq, Term::Unanswered, Truth::True),
            (not_eq, Term::Unknown, Truth::Unknown),
            (answered, Term::Unanswered, Truth::False),
            (answered, Term::Unknown, Truth::Unknown),
            (and_false, Term::Unknown, Truth::False),
            (or_true, Term::Unknown, Truth::True),
        ];
        for (json, term, expected) in cases {
            assert_eq!(with(json, *term), *expected, "{json} with {term:?}");
        }
    }

    #[test]
    fn each_answer_type_compares_its_values() {
        let entities: cairn_schema::EntitySet =
            cairn_schema::BoundedSet::new(["e_one".parse().unwrap()]).unwrap();
        let cases: &[(AnswerValue, &str, bool)] = &[
            (
                AnswerValue::SingleChoice("left".parse().unwrap()),
                r#"{"in": {"decision": "n_a", "values": ["right", "left"]}}"#,
                true,
            ),
            (
                AnswerValue::Date("2026-11-20".parse().unwrap()),
                r#"{"equals": {"decision": "n_a", "value": "2026-11-20"}}"#,
                true,
            ),
            (
                AnswerValue::Text("one two".to_owned().try_into().unwrap()),
                r#"{"contains": {"decision": "n_a", "value": "two"}}"#,
                true,
            ),
            (
                AnswerValue::EntityList(entities),
                r#"{"contains": {"decision": "n_a", "value": "e_two"}}"#,
                false,
            ),
        ];
        for (answer, json, expected) in cases {
            assert_eq!(
                with(json, Term::Answered(answer)),
                Truth::from_bool(*expected),
                "{json}"
            );
        }
    }

    #[test]
    fn entity_values_compare_through_aliases() {
        let mut deployment = Deployment::default();
        deployment
            .entities
            .put(from_json(r#"{"key": "e_kept", "name": "Kept"}"#).unwrap())
            .unwrap();
        deployment
            .aliases
            .insert("e_merged".parse().unwrap(), "e_kept".parse().unwrap());
        let answer = AnswerValue::Entity("e_merged".parse().unwrap());
        let json = r#"{"equals": {"decision": "n_a", "value": "e_kept"}}"#;
        let truth = evaluate(&condition(json), |_| Term::Answered(&answer), &deployment);
        assert_eq!(truth, Truth::True);
    }
}
