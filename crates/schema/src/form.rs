//! What keeps a node form's values from being sent (PRD: A1a, A4, A9, G5; PRACTICES,
//! Explicit limits): each field's value is read through the same types the patch reader
//! builds a node from ([`Title`], [`Markdown`], [`Slug`], [`Weight`], [`Days`], [`Choices`],
//! [`Checklist`]), so the form and the reader share one rule set. A failure comes back at its
//! field in words a person reads, not as a location in a patch.

use serde::{Deserialize, Serialize};

use crate::collections::{
    BoundedVec, ChecklistItemCountPerNode, ChoiceCountPerDecision, CollectionError,
    EdgeCountPerNode, OneOrMany,
};
use crate::field::NodeField;
use crate::id::{RouteId, Slug};
use crate::limits::Limit;
use crate::node::{Checklist, ChecklistItem, Choice, Choices};
use crate::number::{Days, NumberError, Weight};
use crate::text::{Markdown, Title, Url};

/// A choice as the form holds it: a bare id, or an id with a label.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ChoiceValue {
    /// Just the id.
    Id(String),
    /// The id and a label.
    Labeled {
        /// The id.
        id: String,
        /// The label.
        title: String,
    },
}

/// A checklist item as the form holds it.
#[derive(Clone, Debug, Deserialize)]
pub struct ChecklistValue {
    /// The id.
    pub id: String,
    /// What the item says.
    pub title: String,
}

/// The values of a node form as typed, for the fields the node has: a field the node does
/// not have is left out and is not checked. Numbers are the text in their boxes; a text
/// field that may be blank (`description`, `help`) is blank when it is empty or spaces.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeFormValues {
    /// The id.
    pub id: Option<String>,
    /// The title.
    pub title: Option<String>,
    /// The description.
    pub description: Option<String>,
    /// The weight; empty is the kind's default.
    pub weight: Option<String>,
    /// The estimate in days; empty is none.
    pub estimate: Option<String>,
    /// A decision's prompt.
    pub prompt: Option<String>,
    /// A decision's help.
    pub help: Option<String>,
    /// A choice decision's choices.
    pub choices: Option<Vec<ChoiceValue>>,
    /// A deliverable's or action's checklist.
    pub checklist: Option<Vec<ChecklistValue>>,
    /// The `due_by` rule, when the node has one.
    pub due_by: Option<DateRuleValue>,
    /// The `not_before` rule, when the node has one.
    pub not_before: Option<DateRuleValue>,
}

/// A date rule as its editor holds it: what it measures from, and the offset in days.
#[derive(Clone, Debug, Deserialize)]
pub struct DateRuleValue {
    /// The sources, as the editor lists them.
    pub sources: Vec<String>,
    /// The offset in days; none when the box holds no number.
    pub offset: Option<f64>,
}

/// Why one field of a node form cannot be sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FieldProblem {
    /// The field.
    pub field: NodeField,
    /// What is wrong, in words for the person filling in the form.
    pub message: String,
}

/// PRD: A1a, A4, A9, G5. Every field of `values` that cannot be sent, one problem each, in the
/// order the form lists the fields; empty when all can.
#[must_use]
pub fn node_form_problems(values: &NodeFormValues) -> Vec<FieldProblem> {
    let mut problems = Vec::new();
    let mut note = |field: NodeField, problem: Option<String>| {
        if let Some(message) = problem {
            problems.push(FieldProblem { field, message });
        }
    };
    if let Some(id) = &values.id {
        note(
            NodeField::Id,
            id.parse::<Slug>().err().map(|e| e.words("An id")),
        );
    }
    if let Some(title) = &values.title {
        note(
            NodeField::Title,
            title.parse::<Title>().err().map(|e| e.words("The title")),
        );
    }
    if let Some(description) = &values.description {
        note(
            NodeField::Description,
            optional_markdown(description, "The description"),
        );
    }
    if let Some(weight) = &values.weight {
        note(
            NodeField::Weight,
            whole_number(
                weight,
                "A weight",
                |n| Weight::try_from(n).map(|_| ()),
                Limit::Weight,
            ),
        );
    }
    if let Some(estimate) = &values.estimate {
        note(
            NodeField::Estimate,
            whole_number(
                estimate,
                "An estimate in days",
                |n| Days::try_from(n).map(|_| ()),
                Limit::OffsetDays,
            ),
        );
    }
    if let Some(prompt) = &values.prompt {
        note(
            NodeField::Prompt,
            prompt
                .parse::<Markdown>()
                .err()
                .map(|e| e.words("The prompt")),
        );
    }
    if let Some(help) = &values.help {
        note(NodeField::Help, optional_markdown(help, "The help"));
    }
    if let Some(choices) = &values.choices {
        note(NodeField::Choices, choices_problem(choices));
    }
    if let Some(checklist) = &values.checklist {
        note(NodeField::Checklist, checklist_problem(checklist));
    }
    problems.extend(date_rule_problems(values));
    problems
}

/// The problems with the node's date rules, in the order the form lists them.
fn date_rule_problems(values: &NodeFormValues) -> Vec<FieldProblem> {
    [
        (NodeField::DueBy, &values.due_by),
        (NodeField::NotBefore, &values.not_before),
    ]
    .into_iter()
    .filter_map(|(field, rule)| {
        let message = date_rule_problem(rule.as_ref()?)?;
        Some(FieldProblem { field, message })
    })
    .collect()
}

/// What a loose value is read as: the types the patch reader builds the document's values from.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    /// An id ([`Slug`]).
    Id,
    /// A route's id ([`RouteId`]): a slug that is not the one the app keeps for itself.
    RouteId,
    /// A title, name or label ([`Title`]).
    Title,
    /// Markdown free text ([`Markdown`]).
    Markdown,
    /// A link's target ([`Url`]).
    Url,
}

/// One value of a form that is not a node's: what it is read as and what the form calls it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueCheck {
    /// Names the value in the answer; the form's own name for the field.
    pub field: String,
    /// What the value is read as.
    pub kind: ValueKind,
    /// A phrase that starts a sentence and names the value, such as "The name".
    pub what: String,
    /// The value as typed.
    pub value: String,
    /// Whether a blank value is left out rather than refused.
    #[serde(default)]
    pub optional: bool,
}

/// Why one value of a form cannot be sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ValueProblem {
    /// The `field` of the check.
    pub field: String,
    /// What is wrong, in words for the person filling in the form.
    pub message: String,
}

/// PRD: A1a, A6, A7, A9, E6. Every value of `checks` that cannot be sent, one problem
/// each, in the order given, read through the types the patch reader builds the same values from.
#[must_use]
pub fn value_problems(checks: &[ValueCheck]) -> Vec<ValueProblem> {
    checks
        .iter()
        .filter(|check| !(check.optional && check.value.trim().is_empty()))
        .filter_map(|check| {
            let what = check.what.as_str();
            let value = check.value.as_str();
            let message = match check.kind {
                ValueKind::Id => value.parse::<Slug>().err().map(|e| e.words(what)),
                ValueKind::RouteId => value.parse::<RouteId>().err().map(|e| e.words(what)),
                ValueKind::Title => value.parse::<Title>().err().map(|e| e.words(what)),
                ValueKind::Markdown => value.parse::<Markdown>().err().map(|e| e.words(what)),
                ValueKind::Url => value.parse::<Url>().err().map(|e| e.words(what)),
            };
            message.map(|message| ValueProblem {
                field: check.field.clone(),
                message,
            })
        })
        .collect()
}

/// A date rule that measures from nothing, from too many sources, or by an offset out of range.
fn date_rule_problem(rule: &DateRuleValue) -> Option<String> {
    if let Err(error) =
        OneOrMany::<&str, EdgeCountPerNode>::new(rule.sources.iter().map(String::as_str))
    {
        return Some(match error {
            CollectionError::Empty => "A date rule measures from at least one source.".to_owned(),
            other => other.words("A date rule's sources"),
        });
    }
    let ok = rule
        .offset
        .and_then(whole)
        .is_some_and(|days| Days::try_from(days).is_ok());
    (!ok).then(|| range_words("An offset in days", Limit::OffsetDays))
}

/// The whole number a float holds, as the number box reads `1.0` and `1e2`.
fn whole(number: f64) -> Option<u32> {
    (number.fract() == 0.0)
        .then(|| number.to_string().parse::<u32>().ok())
        .flatten()
}

fn range_words(what: &str, limit: Limit) -> String {
    format!("{what} is a whole number from 0 to {}.", limit.max())
}

/// Free text that may be left blank.
fn optional_markdown(text: &str, what: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    text.parse::<Markdown>().err().map(|e| e.words(what))
}

/// A count of days or a weight typed as text: blank is none, anything else a whole number
/// within `limit`.
fn whole_number(
    text: &str,
    what: &str,
    check: impl Fn(u32) -> Result<(), NumberError>,
    limit: Limit,
) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    let ok = text
        .trim()
        .parse::<f64>()
        .ok()
        .and_then(whole)
        .is_some_and(|n| check(n).is_ok());
    (!ok).then(|| range_words(what, limit))
}

fn choices_problem(values: &[ChoiceValue]) -> Option<String> {
    let mut choices = Vec::with_capacity(values.len());
    for value in values {
        let (id, title) = match value {
            ChoiceValue::Id(id) => (id, None),
            ChoiceValue::Labeled { id, title } => (id, Some(title)),
        };
        let id = match id.parse::<Slug>() {
            Ok(id) => id,
            Err(error) => return Some(error.words("A choice's id")),
        };
        let title = match title.map(|title| title.parse::<Title>()).transpose() {
            Ok(title) => title,
            Err(error) => return Some(error.words("A choice's label")),
        };
        choices.push(Choice { id, title });
    }
    BoundedVec::<Choice, ChoiceCountPerDecision>::new(choices)
        .and_then(Choices::try_from)
        .err()
        .map(|error| match error {
            CollectionError::Empty => "Add at least one choice.".to_owned(),
            other => other.words("The choices"),
        })
}

fn checklist_problem(values: &[ChecklistValue]) -> Option<String> {
    let mut items = Vec::with_capacity(values.len());
    for (at, value) in values.iter().enumerate() {
        let number = at + 1;
        let id = match value.id.parse::<Slug>() {
            Ok(id) => id,
            Err(error) => return Some(error.words(&format!("Item {number}'s id"))),
        };
        let title = match value.title.parse::<Title>() {
            Ok(title) => title,
            Err(error) => return Some(error.words(&format!("Item {number}'s title"))),
        };
        items.push(ChecklistItem { id, title });
    }
    BoundedVec::<ChecklistItem, ChecklistItemCountPerNode>::new(items)
        .and_then(Checklist::try_from)
        .err()
        .map(|error| error.words("The checklist items"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(values: &str) -> Vec<(NodeField, String)> {
        let values: NodeFormValues = serde_json::from_str(values).unwrap();
        node_form_problems(&values)
            .into_iter()
            .map(|problem| (problem.field, problem.message))
            .collect()
    }

    fn fields(values: &str) -> Vec<NodeField> {
        problems(values)
            .into_iter()
            .map(|(field, _)| field)
            .collect()
    }

    #[test]
    fn values_within_every_rule_have_no_problem() {
        let within = serde_json::json!({
            "id": "work", "title": "Work", "description": "", "weight": "1000", "estimate": "365",
            "prompt": "Ready?", "help": "  ",
            "choices": ["yes", {"id": "later", "title": "Later"}],
            "checklist": [{"id": "a", "title": "A"}],
        });
        assert_eq!(problems(&within.to_string()), []);
        assert_eq!(problems("{}"), []);
        // The number box's own readings of a whole number.
        assert_eq!(fields(r#"{"weight": "1.0", "estimate": "1e2"}"#), []);
    }

    #[test]
    fn each_field_fails_at_its_own_rule() {
        let cases = [
            (r#"{"id": "Not a slug"}"#, NodeField::Id),
            (&format!(r#"{{"id": "{}"}}"#, "i".repeat(65)), NodeField::Id),
            (r#"{"title": "  "}"#, NodeField::Title),
            (r#"{"title": "two\nlines"}"#, NodeField::Title),
            (
                &format!(r#"{{"title": "{}"}}"#, "t".repeat(257)),
                NodeField::Title,
            ),
            (
                &format!(r#"{{"description": "{}"}}"#, "d".repeat(64 * 1024 + 1)),
                NodeField::Description,
            ),
            (r#"{"weight": "1001"}"#, NodeField::Weight),
            (r#"{"weight": "-1"}"#, NodeField::Weight),
            (r#"{"estimate": "366"}"#, NodeField::Estimate),
            (r#"{"estimate": "1.5"}"#, NodeField::Estimate),
            (r#"{"estimate": "abc"}"#, NodeField::Estimate),
            (r#"{"prompt": ""}"#, NodeField::Prompt),
            (
                &format!(r#"{{"help": "{}"}}"#, "h".repeat(64 * 1024 + 1)),
                NodeField::Help,
            ),
            (r#"{"choices": []}"#, NodeField::Choices),
            (r#"{"choices": ["Up"]}"#, NodeField::Choices),
            (r#"{"choices": ["a", "a"]}"#, NodeField::Choices),
            (
                r#"{"choices": [{"id": "a", "title": ""}]}"#,
                NodeField::Choices,
            ),
            (
                r#"{"checklist": [{"id": "a", "title": " "}]}"#,
                NodeField::Checklist,
            ),
            (
                r#"{"checklist": [{"id": "a", "title": "A"}, {"id": "a", "title": "B"}]}"#,
                NodeField::Checklist,
            ),
        ];
        for (values, field) in cases {
            assert_eq!(fields(values), [field], "{values}");
        }
    }

    #[test]
    fn date_rules_fail_without_a_source_or_past_the_offset() {
        let rule = |sources: usize, offset: &str| {
            let sources: Vec<String> = (0..sources).map(|at| format!("n_{at}")).collect();
            format!(r#"{{"due_by": {{"sources": {sources:?}, "offset": {offset}}}}}"#)
        };
        assert_eq!(fields(&rule(1, "365")), []);
        assert_eq!(fields(&rule(1, "0")), []);
        assert_eq!(fields(&rule(0, "0")), [NodeField::DueBy]);
        assert_eq!(fields(&rule(1, "366")), [NodeField::DueBy]);
        assert_eq!(fields(&rule(1, "-1")), [NodeField::DueBy]);
        assert_eq!(fields(&rule(1, "1.5")), [NodeField::DueBy]);
        assert_eq!(fields(&rule(1, "null")), [NodeField::DueBy]);
        assert_eq!(
            fields(&rule(1, "366").replace("due_by", "not_before")),
            [NodeField::NotBefore]
        );
        assert_eq!(fields(&rule(64, "0")), []);
        assert_eq!(fields(&rule(65, "0")), [NodeField::DueBy]);
        let twice = r#"{"due_by": {"sources": ["n_a", "n_a"], "offset": 0}}"#;
        assert_eq!(fields(twice), [NodeField::DueBy]);
    }

    fn value_fields(checks: &str) -> Vec<String> {
        let checks: Vec<ValueCheck> = serde_json::from_str(checks).unwrap();
        value_problems(&checks)
            .into_iter()
            .map(|problem| problem.field)
            .collect()
    }

    #[test]
    fn loose_values_fail_at_the_rule_of_their_kind() {
        let check = |kind: &str, value: &str, optional: bool| {
            serde_json::json!([{"field": "f", "kind": kind, "what": "It", "value": value, "optional": optional}])
                .to_string()
        };
        let (long_title, long_body) = ("t".repeat(257), "m".repeat(64 * 1024 + 1));
        let cases = [
            ("id", "ok-id", true),
            ("id", "Not a slug", false),
            ("route_id", "new", false),
            ("route_id", "a-route", true),
            ("title", "", false),
            ("title", &long_title, false),
            ("markdown", &long_body, false),
            ("url", "https://example.org", true),
            ("url", "no scheme", false),
        ];
        for (kind, value, ok) in cases {
            let failed = value_fields(&check(kind, value, false));
            assert_eq!(failed.is_empty(), ok, "{kind} {value:?}");
        }
        // A blank optional value is left out; a blank required one is refused.
        assert_eq!(
            value_fields(&check("title", "  ", true)),
            Vec::<String>::new()
        );
        assert_eq!(value_fields(&check("title", "  ", false)), ["f"]);
    }

    #[test]
    fn counts_fail_one_past_their_limit() {
        let choices = |count: u32| {
            let ids: Vec<String> = (0..count).map(|at| format!("c{at}")).collect();
            serde_json::json!({ "choices": ids }).to_string()
        };
        let items = |count: u32| {
            let items: Vec<_> = (0..count)
                .map(|at| serde_json::json!({"id": format!("i{at}"), "title": "Item"}))
                .collect();
            serde_json::json!({ "checklist": items }).to_string()
        };
        assert_eq!(fields(&choices(32)), []);
        assert_eq!(fields(&choices(33)), [NodeField::Choices]);
        assert_eq!(fields(&items(50)), []);
        assert_eq!(fields(&items(51)), [NodeField::Checklist]);
    }
}
