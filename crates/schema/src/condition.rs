//! Conditions (A5; ARCHITECTURE, Conditions): a structured predicate tree over decisions
//! with a fixed operator set and no parser. A condition reads as the PRD writes it:
//!
//! ```yaml
//! relevant_when:
//!   all:
//!     - equals: {decision: testing/who, value: partner}
//!     - not: {answered: setup/waiver}
//! ```
//!
//! Whether a referenced decision exists and has a compatible answer type is the engine's
//! check (Invariants); the tree's depth and clause count are checked here.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, CollectionError, EntityCountPerFill};
use crate::limits::Limit;
use crate::refs::References;
use crate::text::TextError;

/// The value a clause compares an answer with: a boolean, or text that the decision's
/// answer type gives meaning (a choice id, a date, an entity key, or plain text).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(untagged)]
pub enum ConditionValue {
    /// A boolean answer.
    Boolean(bool),
    /// Every other answer type, as text.
    Text(ConditionText),
}

/// The text form of a compared value: any text a text answer can hold (empty and multi-line
/// included, since an empty submission is an answer), at most the body limit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConditionText(String);

impl ConditionText {
    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for ConditionText {
    type Err = TextError;

    fn from_str(text: &str) -> Result<Self, TextError> {
        Limit::BodyBytes
            .check(text.len())
            .map_err(TextError::TooLong)?;
        Ok(Self(text.to_owned()))
    }
}

impl std::fmt::Display for ConditionText {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for ConditionText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ConditionText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for ConditionText {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ConditionText".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A compared value: a choice id, date, entity key, or text; at most body_bytes_max bytes.",
            "maxLength": Limit::BodyBytes.max(),
        })
    }
}

/// The values an `in` clause accepts.
pub type ConditionValues = BoundedVec<ConditionValue, EntityCountPerFill>;

/// `{decision, value}`: the operand of `equals`, `not_equals`, and `contains`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Comparison{R}")]
pub struct Comparison<R: References> {
    /// The decision whose answer is compared.
    pub decision: R::Node,
    /// The value it is compared with.
    pub value: ConditionValue,
}

/// `{decision, values}`: the operand of `in`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Membership{R}")]
pub struct Membership<R: References> {
    /// The decision whose answer is tested.
    pub decision: R::Node,
    /// The values it may equal.
    pub values: ConditionValues,
}

/// One node of a condition tree (A5's operators: equals, not equals, in, contains, is
/// answered, and, or, not).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields, bound = "")]
#[schemars(bound = "R: References", rename = "Clause{R}", transform = non_empty_combinations)]
pub enum Clause<R: References> {
    /// The answer equals the value.
    Equals(Comparison<R>),
    /// The answer does not equal the value.
    NotEquals(Comparison<R>),
    /// The answer is one of the values.
    In(Membership<R>),
    /// The answer (a list, or text) contains the value.
    Contains(Comparison<R>),
    /// The decision is answered.
    Answered(R::Node),
    /// Every clause holds.
    All(Vec<Clause<R>>),
    /// Some clause holds.
    Any(Vec<Clause<R>>),
    /// The clause does not hold.
    Not(Box<Clause<R>>),
}

/// The schema form of [`ConditionError::EmptyCombination`] and the clause limit: `all` and
/// `any` hold at least one clause and at most `condition_clause_count_max` (each child
/// holds at least one clause). The tree-wide clause count and the depth limit are the
/// parser's: JSON Schema cannot count across a tree, and the schema says so.
fn non_empty_combinations(schema: &mut schemars::Schema) {
    schema.insert(
        "description".to_owned(),
        serde_json::Value::String(format!(
            "A condition clause. The parser also holds the whole tree to {} levels and {} clauses, which JSON Schema cannot express.",
            Limit::ConditionDepth.max(),
            Limit::ConditionClauseCount.max()
        )),
    );
    let Some(serde_json::Value::Array(variants)) = schema.get_mut("oneOf") else {
        return;
    };
    for variant in variants {
        for operator in ["all", "any"] {
            if let Some(serde_json::Value::Object(operand)) =
                variant.pointer_mut(&format!("/properties/{operator}"))
            {
                operand.insert("minItems".to_owned(), 1.into());
                operand.insert(
                    "maxItems".to_owned(),
                    Limit::ConditionClauseCount.max().into(),
                );
            }
        }
    }
}

/// Why a condition tree was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConditionError {
    /// Deeper than `condition_depth_max` or more clauses than `condition_clause_count_max`.
    TooLarge(crate::limits::LimitExceeded),
    /// An `all` or `any` with no clauses, which has no meaning to evaluate.
    EmptyCombination,
}

impl fmt::Display for ConditionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionError::TooLarge(exceeded) => {
                write!(formatter, "condition is too large: {exceeded}")
            }
            ConditionError::EmptyCombination => {
                formatter.write_str("`all` or `any` with no clauses")
            }
        }
    }
}

impl std::error::Error for ConditionError {}

impl From<CollectionError> for ConditionError {
    fn from(error: CollectionError) -> Self {
        match error {
            CollectionError::TooMany(exceeded) => ConditionError::TooLarge(exceeded),
            CollectionError::Duplicate(_) | CollectionError::Empty => {
                ConditionError::EmptyCombination
            }
        }
    }
}

/// A `relevant_when` condition: a clause tree within `condition_depth_max` and
/// `condition_clause_count_max`, where clauses are the leaf predicates.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(try_from = "Clause<R>", into = "Clause<R>", bound = "")]
#[schemars(bound = "R: References", rename = "Condition{R}")]
pub struct Condition<R: References>(Clause<R>);

impl<R: References> Condition<R> {
    /// Checks the tree's size (A5; PRACTICES, Explicit limits).
    ///
    /// # Errors
    ///
    /// When the tree is too deep, has too many clauses, or holds an empty `all` or `any`.
    pub fn new(root: Clause<R>) -> Result<Self, ConditionError> {
        // Iterative walk with an explicit stack (PRACTICES, No recursion). Depth is checked
        // as each clause is reached, so the stack never holds more than the limit allows.
        let mut clause_count: usize = 0;
        let mut stack: Vec<(&Clause<R>, usize)> = vec![(&root, 1)];
        while let Some((clause, depth)) = stack.pop() {
            Limit::ConditionDepth
                .check(depth)
                .map_err(ConditionError::TooLarge)?;
            match clause {
                Clause::Equals(_)
                | Clause::NotEquals(_)
                | Clause::In(_)
                | Clause::Contains(_)
                | Clause::Answered(_) => {
                    clause_count += 1;
                    Limit::ConditionClauseCount
                        .check(clause_count)
                        .map_err(ConditionError::TooLarge)?;
                }
                Clause::All(children) | Clause::Any(children) => {
                    if children.is_empty() {
                        return Err(ConditionError::EmptyCombination);
                    }
                    stack.extend(children.iter().map(|child| (child, depth + 1)));
                }
                Clause::Not(child) => stack.push((child, depth + 1)),
            }
        }
        Ok(Self(root))
    }

    /// The root clause.
    #[must_use]
    pub fn root(&self) -> &Clause<R> {
        &self.0
    }

    /// Every decision the condition references, in tree order (each once per clause). The
    /// referenced set is static, which is what makes implicit gates possible.
    #[must_use]
    pub fn decisions(&self) -> Vec<&R::Node> {
        let mut decisions = Vec::new();
        let mut stack = vec![&self.0];
        while let Some(clause) = stack.pop() {
            match clause {
                Clause::Equals(comparison)
                | Clause::NotEquals(comparison)
                | Clause::Contains(comparison) => {
                    decisions.push(&comparison.decision);
                }
                Clause::In(membership) => decisions.push(&membership.decision),
                Clause::Answered(decision) => decisions.push(decision),
                Clause::All(children) | Clause::Any(children) => {
                    stack.extend(children.iter().rev());
                }
                Clause::Not(child) => stack.push(child),
            }
        }
        decisions
    }
}

impl<R: References> TryFrom<Clause<R>> for Condition<R> {
    type Error = ConditionError;

    fn try_from(root: Clause<R>) -> Result<Self, ConditionError> {
        Self::new(root)
    }
}

impl<R: References> From<Condition<R>> for Clause<R> {
    fn from(condition: Condition<R>) -> Self {
        condition.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::FileRefs;

    fn parse(yaml_like_json: serde_json::Value) -> Result<Condition<FileRefs>, String> {
        serde_json::from_value(yaml_like_json).map_err(|error| error.to_string())
    }

    fn answered() -> serde_json::Value {
        serde_json::json!({"answered": "setup/waiver"})
    }

    fn nested(depth: usize) -> serde_json::Value {
        let mut clause = answered();
        for _ in 1..depth {
            clause = serde_json::json!({ "not": clause });
        }
        clause
    }

    #[test]
    fn reads_the_architecture_example() {
        let condition = parse(serde_json::json!({"all": [
            {"equals": {"decision": "testing/who", "value": "partner"}},
            {"not": {"answered": "setup/waiver"}},
        ]}))
        .unwrap();
        let decisions: Vec<String> = condition
            .decisions()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(decisions, ["testing/who", "setup/waiver"]);
    }

    #[test]
    fn depth_at_and_past_its_limit() {
        assert!(parse(nested(8)).is_ok());
        let error = parse(nested(9)).unwrap_err();
        assert!(error.contains(Limit::ConditionDepth.name()), "{error}");
    }

    #[test]
    fn clauses_at_and_past_their_limit() {
        let at: Vec<_> = (0..16).map(|_| answered()).collect();
        assert!(parse(serde_json::json!({ "any": at })).is_ok());
        let past: Vec<_> = (0..17).map(|_| answered()).collect();
        let error = parse(serde_json::json!({ "any": past })).unwrap_err();
        assert!(
            error.contains(Limit::ConditionClauseCount.name()),
            "{error}"
        );
    }

    #[test]
    fn text_operands_may_be_empty_or_multiline() {
        for value in ["", "two\nlines"] {
            let condition = serde_json::json!({"equals": {"decision": "notes", "value": value}});
            assert!(parse(condition).is_ok(), "{value:?}");
        }
    }

    #[test]
    fn rejects_unknown_operators_and_empty_combinations() {
        for bad in [
            serde_json::json!({"greater_than": {"decision": "a", "value": 1}}),
            serde_json::json!({"all": []}),
            serde_json::json!({"equals": {"decision": "a", "value": "x", "extra": 1}}),
            serde_json::json!({"equals": {"decision": "a", "value": "x"}, "answered": "b"}),
        ] {
            assert!(parse(bad.clone()).is_err(), "{bad}");
        }
    }
}
