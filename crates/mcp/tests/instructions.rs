//! The shipped instructions against the tool surface (I4) and the tool surface against I2:
//! every identifier the guide and its workflows name is a tool, or a field or value some
//! tool takes or answers, and every capability I2 lists has a tool.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_mcp::ToolSet;
use cairn_mcp::instructions::{self, SKILL};
use cairn_store::MemoryStore;
use serde_json::Value;

/// I4: the instructions name only tools that exist. An identifier written as code (`name`)
/// must be a tool's name, or a field or value in some tool's arguments or output, so a
/// renamed or removed tool or field fails here rather than misleading an agent.
#[test]
fn the_instructions_name_only_tools_and_fields_that_exist() {
    let definitions = ToolSet::<MemoryStore>::definitions();
    let mut known: BTreeSet<String> = definitions.iter().map(|t| t.name.to_owned()).collect();
    for tool in &definitions {
        vocabulary(&Value::Object(tool.input_schema.clone()), &mut known);
        vocabulary(&Value::Object(tool.output_schema.clone()), &mut known);
    }
    let mut texts = vec![("SKILL.md".to_owned(), SKILL)];
    texts.extend(
        instructions::workflows()
            .into_iter()
            .map(|w| (w.name.to_owned(), w.text)),
    );
    let mut named_tools = BTreeSet::new();
    for (file, text) in texts {
        for identifier in identifiers(text) {
            assert!(
                known.contains(&identifier),
                "{file} names `{identifier}`, which no tool has"
            );
            if definitions.iter().any(|tool| tool.name == identifier) {
                named_tools.insert(identifier);
            }
        }
    }
    let every: BTreeSet<String> = definitions.iter().map(|t| t.name.to_owned()).collect();
    assert_eq!(named_tools, every, "the guide names every tool");
}

/// I2: each capability the PRD lists as the minimum tool surface, and the tools that cover
/// it; every tool named is one the set has.
#[test]
fn every_i2_capability_has_a_tool() {
    let coverage: &[(&str, &[&str])] = &[
        ("list/get routes", &["list_routes", "get_route"]),
        ("list/get journeys", &["list_journeys", "get_snapshot"]),
        (
            "create a journey (from route or empty)",
            &["create_journey"],
        ),
        (
            "create/edit a route draft and publish it",
            &[
                "open_draft",
                "apply_patch",
                "create_proposal",
                "publish_draft",
            ],
        ),
        ("get the graph at an aggregation level", &["get_level"]),
        (
            "get the ranked frontier",
            &["list_frontier", "get_snapshot"],
        ),
        (
            "decisions needed, needs breakdown, unassigned, active, blocked, stale, overdue, shortfall, mine",
            &["list_frontier"],
        ),
        (
            "get node detail with priority and date explanations",
            &["get_node"],
        ),
        ("answer decisions", &["answer_decision"]),
        ("transition nodes", &["transition_node"]),
        ("snooze/unsnooze", &["snooze", "unsnooze"]),
        ("assign participations", &["assign"]),
        ("apply a patch", &["apply_patch"]),
        (
            "draft/get/edit proposals",
            &[
                "create_proposal",
                "get_proposal",
                "edit_proposal",
                "apply_proposal",
            ],
        ),
        (
            "pin/unpin dates and resolve conflicts",
            &["set_date", "resolve_date_conflict"],
        ),
        (
            "force-include, keep under skipped ancestors, and bypass guards",
            &["override"],
        ),
        ("manage entities (create, link, merge)", &["manage_entity"]),
        ("route import/export", &["import_route", "export_route"]),
        ("save as route", &["save_as_route"]),
        ("re-link", &["relink"]),
        ("upgrade", &["upgrade"]),
        ("search", &["search"]),
    ];
    let tools: BTreeSet<&str> = ToolSet::<MemoryStore>::definitions()
        .iter()
        .map(|t| t.name)
        .collect();
    for (capability, covered_by) in coverage {
        assert!(!covered_by.is_empty(), "{capability}");
        for tool in *covered_by {
            assert!(tools.contains(tool), "{capability}: no tool {tool}");
        }
    }
}

/// Every property name, enum value, and constant in a JSON Schema.
fn vocabulary(schema: &Value, into: &mut BTreeSet<String>) {
    let mut stack = vec![schema];
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(object) => {
                if let Some(Value::Object(properties)) = object.get("properties") {
                    into.extend(properties.keys().cloned());
                }
                for key in ["enum", "const"] {
                    let values = object.get(key).into_iter().flat_map(|found| match found {
                        Value::Array(items) => items.iter().collect(),
                        other => vec![other],
                    });
                    into.extend(values.filter_map(Value::as_str).map(str::to_owned));
                }
                stack.extend(object.values());
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
}

/// The identifiers a Markdown text writes as inline code (`snake_case`), outside fenced
/// blocks: words of lowercase letters joined by underscores.
fn identifiers(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let spans = line.split('`').skip(1).step_by(2);
        let words = spans.filter(|span| {
            let parts: Vec<&str> = span.split('_').collect();
            parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase()))
        });
        found.extend(words.map(str::to_owned));
    }
    found
}
