//! What keeps a node form's values from being sent (PRD: A1a, A4, A9, G5; PRACTICES,
//! Explicit limits): each field's value is read through the same types the patch reader
//! builds a node from ([`Title`], [`Markdown`], [`Slug`], [`Weight`], [`Days`], [`Choices`],
//! [`Checklist`]), so the form and the reader share one rule set. A failure comes back at its
//! field in words a person reads, not as a location in a patch.

use serde::{Deserialize, Serialize};

use crate::collections::{
    BoundedVec, ChecklistItemCountPerNode, ChoiceCountPerDecision, CollectionError,
};
use crate::field::NodeField;
use crate::id::Slug;
use crate::limits::Limit;
use crate::node::{Checklist, ChecklistItem, Choice, Choices};
use crate::number::{Days, NumberError, Weight};
use crate::text::{Markdown, Title};

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
    problems
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
    // As the number box reads it: `1.0` and `1e2` are whole numbers.
    let whole = text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.fract() == 0.0)
        .and_then(|number| number.to_string().parse::<u32>().ok());
    let ok = whole.is_some_and(|n| check(n).is_ok());
    (!ok).then(|| format!("{what} is a whole number from 0 to {}.", limit.max()))
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
