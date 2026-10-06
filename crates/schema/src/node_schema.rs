//! The JSON Schema form of the node kind restrictions (A1a), built from the same table the
//! parser checks ([`KindField::allowed_on`], [`KindField::allowed_for_answer`]), so the
//! published schema and the parser cannot disagree about which field belongs where.

use schemars::Schema;
use serde_json::{Map, Value, json};

use crate::node::{AnswerType, KindField, NodeKind};

fn forbidden(fields: impl Iterator<Item = KindField>) -> Value {
    let properties: Map<String, Value> = fields
        .map(|field| (field.name().to_owned(), Value::Bool(false)))
        .collect();
    Value::Object(properties)
}

/// Adds an `allOf` of `if`/`then` rules: per kind, the fields it may not carry; for a
/// decision, its required fields; per answer type, the decision fields it may not carry and
/// the ones it requires.
pub(crate) fn restrict_by_kind(schema: &mut Schema) {
    let mut rules = Vec::new();
    for kind in NodeKind::ALL {
        let not_allowed = KindField::ALL
            .into_iter()
            .filter(|field| !field.allowed_on(kind));
        rules.push(json!({
            "if": { "properties": { "kind": { "const": kind.name() } } },
            "then": { "properties": forbidden(not_allowed) },
        }));
    }
    rules.push(json!({
        "if": { "properties": { "kind": { "const": NodeKind::Decision.name() } } },
        "then": { "required": [KindField::Prompt.name(), KindField::AnswerType.name()] },
    }));
    for answer in AnswerType::ALL {
        let not_allowed = KindField::ALL
            .into_iter()
            .filter(|field| !field.allowed_for_answer(answer));
        let mut then = json!({ "properties": forbidden(not_allowed) });
        if matches!(answer, AnswerType::SingleChoice | AnswerType::MultiChoice) {
            then["required"] = json!([KindField::Choices.name()]);
        }
        rules.push(json!({
            "if": {
                "properties": { "answer_type": { "const": answer.name() } },
                "required": [KindField::AnswerType.name()],
            },
            "then": then,
        }));
    }
    schema.insert("allOf".to_owned(), Value::Array(rules));
}
