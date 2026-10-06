//! The generated JSON Schema describes the format the parser reads: documents the parser
//! accepts validate, and the kind restrictions and unknown fields it rejects fail
//! validation too.

#![cfg(test)]

use cairn_schema::{RouteFile, from_yaml, json_schema, to_json};
use serde_json::{Value, json};

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(&json_schema::route_file()).unwrap();
    jsonschema::draft202012::new(&schema).unwrap()
}

/// A route file the parser accepts, as JSON.
fn parsed_as_json(yaml: &str) -> Value {
    let file: RouteFile = from_yaml(yaml).unwrap();
    serde_json::from_str(&to_json(&file).unwrap()).unwrap()
}

const VALID: &str = include_str!("data/every_field.yaml");

#[test]
fn a_parsed_route_file_validates() {
    let validator = validator();
    let document = parsed_as_json(VALID);
    let errors: Vec<String> = validator
        .iter_errors(&document)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn what_the_parser_rejects_the_schema_rejects() {
    let validator = validator();
    let base = parsed_as_json(VALID);
    let long_title = "t".repeat(257);
    let changes: Vec<(&str, Value)> = vec![
        (
            "an estimate on a milestone",
            json!({"id": "m", "kind": "milestone", "title": "M", "estimate": 2}),
        ),
        (
            "a null estimate on a group",
            json!({"id": "g", "kind": "group", "title": "G", "estimate": null}),
        ),
        (
            "a null estimate on a deliverable",
            json!({"id": "d", "kind": "deliverable", "title": "D", "estimate": null}),
        ),
        (
            "choices on a text decision",
            json!({"id": "d", "kind": "decision", "title": "D", "prompt": "Q?", "answer_type": "text", "choices": ["a"]}),
        ),
        (
            "an empty choice list",
            json!({"id": "d", "kind": "decision", "title": "D", "prompt": "Q?", "answer_type": "single_choice", "choices": []}),
        ),
        (
            "a decision without a prompt",
            json!({"id": "d", "kind": "decision", "title": "D", "answer_type": "text"}),
        ),
        (
            "an empty date rule",
            json!({"id": "a", "kind": "action", "title": "A", "due_by": {}}),
        ),
        (
            "a date rule both before and after",
            json!({"id": "a", "kind": "action", "title": "A", "due_by": {"before": "x", "after": "y"}}),
        ),
        (
            "a resource with no content",
            json!({"id": "a", "kind": "action", "title": "A", "resources": [{}]}),
        ),
        (
            "a resource with two contents",
            json!({"id": "a", "kind": "action", "title": "A", "resources": [{"tip": "x", "example": "https://example.org"}]}),
        ),
        (
            "a title past its limit",
            json!({"id": "a", "kind": "action", "title": long_title}),
        ),
        (
            "an unknown field",
            json!({"id": "a", "kind": "action", "title": "A", "colour": "red"}),
        ),
    ];
    for (name, node) in changes {
        let mut document = base.clone();
        document["nodes"].as_array_mut().unwrap().push(node.clone());
        assert!(!validator.is_valid(&document), "the schema accepted {name}");
        let parsed = serde_json::from_value::<RouteFile>(document);
        assert!(parsed.is_err(), "the parser accepted {name}");
    }
}

#[test]
fn a_kind_written_twice_is_rejected_not_overwritten() {
    let node = r#"{"id":"a","kind":"action","title":"A","participations":{"owner":["e_one"],"owner":["e_two"]}}"#;
    assert!(cairn_schema::from_json::<cairn_schema::Node<cairn_schema::FileRefs>>(node).is_err());
}
