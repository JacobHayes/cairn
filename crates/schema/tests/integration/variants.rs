//! Every enum that lists its variants by hand lists them all: a variant added to the enum
//! and left out of its list would be skipped silently by whatever walks the list.

#![cfg(test)]

use std::collections::BTreeSet;

use cairn_schema::{
    AnswerType, Days, DisplayState, JourneyStatus, KeyRefs, KindColumns, KindField, LevelDisplay,
    NodeField, NodeKind, State,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// Every string a schema names as a constant, at any depth.
fn constants(schema: &Value, into: &mut BTreeSet<String>) {
    match schema {
        Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("const", Value::String(text)) => {
                        into.insert(text.clone());
                    }
                    ("enum", Value::Array(items)) => {
                        into.extend(items.iter().filter_map(Value::as_str).map(str::to_owned));
                    }
                    _ => constants(value, into),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| constants(item, into)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn every_variant_is_listed<T: JsonSchema + Serialize>(all: &[T]) {
    let mut named = BTreeSet::new();
    constants(&schemars::schema_for!(T).to_value(), &mut named);
    let listed: BTreeSet<String> = all
        .iter()
        .map(|variant| match serde_json::to_value(variant) {
            Ok(Value::String(text)) => text,
            other => panic!("a unit variant serializes as text: {other:?}"),
        })
        .collect();
    assert_eq!(listed, named, "{}", T::schema_name());
}

#[test]
fn every_enum_lists_all_its_variants() {
    every_variant_is_listed(&NodeField::ALL);
    every_variant_is_listed(&NodeKind::ALL);
    every_variant_is_listed(&AnswerType::ALL);
    every_variant_is_listed(&State::ALL);
    every_variant_is_listed(&DisplayState::ALL);
    every_variant_is_listed(&JourneyStatus::ALL);
    every_variant_is_listed(&LevelDisplay::ALL);
}

/// `KindColumns::present` reads each column once, so a column added needs a `KindField`,
/// and this list then needs the field too.
#[test]
fn the_kind_fields_are_the_kind_columns() {
    let node = || -> cairn_schema::NodeKey { "n_one".parse().unwrap() };
    let columns = KindColumns::<KeyRefs> {
        estimate: Some(Days::try_from(1).unwrap()),
        placeholder: Some(true),
        requires_artifact: Some(true),
        requires_note: Some(true),
        is_final: Some(true),
        auto_done: Some(true),
        opens_at: Some(node()),
        closes_at: Some(node()),
        gates: Some(true),
        closes: Some(true),
        prompt: Some("Which?".parse().unwrap()),
        answer_type: Some(AnswerType::Text),
        choices: Some(serde_json::from_value(serde_json::json!(["a"])).unwrap()),
        fills_role: Some("r_lead".parse().unwrap()),
        feeds_milestone: Some(node()),
        help: Some("Help.".parse().unwrap()),
    };
    assert_eq!(columns.present(), KindField::ALL);
}
