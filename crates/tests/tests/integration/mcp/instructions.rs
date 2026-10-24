//! The shipped instructions against the tool surface (I4) and the tool surface against I2:
//! every identifier the guide and its workflows name is a tool, or a field or value some
//! tool takes or answers, and every capability I2 lists has a tool.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_mcp::ToolSet;
use cairn_mcp::instructions::{self, SKILL};
use cairn_schema::{DisplayState, NodeKind};
use serde_json::Value;

/// I4: the instructions name only tools that exist. An identifier written as code (`name`)
/// must be a tool's name, or a field or value in some tool's arguments or output, so a
/// renamed or removed tool or field fails here rather than misleading an agent.
#[test]
fn the_instructions_name_only_tools_and_fields_that_exist() {
    let definitions = ToolSet::definitions();
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

/// D8: the guide's table of display states says each one by the words every screen does:
/// the state's word, then each kind's own word where it differs.
#[test]
fn the_guide_says_each_display_state_as_the_screens_do() {
    let said: Vec<(String, String)> = SKILL
        .lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split(" | ");
            let state = cells.next()?.strip_suffix('`')?.to_owned();
            serde_json::from_value::<DisplayState>(Value::String(state.clone())).ok()?;
            Some((state, cells.next()?.to_owned()))
        })
        .collect();
    let expected: Vec<(String, String)> = DisplayState::ALL
        .iter()
        .map(|state| {
            let name = serde_json::to_value(state)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            let kinds: Vec<String> = NodeKind::ALL
                .iter()
                .filter(|kind| state.word(Some(**kind)) != state.word(None))
                .map(|kind| format!("a {}: {}", kind.name(), state.word(Some(*kind))))
                .collect();
            let words = if kinds.is_empty() {
                state.word(None).to_owned()
            } else {
                format!("{} ({})", state.word(None), kinds.join("; "))
            };
            (name, words)
        })
        .collect();
    let (mut said, mut expected) = (said, expected);
    said.sort();
    expected.sort();
    assert_eq!(said, expected);
}

/// The clauses of the PRD's minimum tool surface (I2), as the PRD writes them: the text after
/// "Tool surface includes at least:" in the I2 requirement, split at its semicolons.
fn prd_tool_surface() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../PRD.md");
    let prd = std::fs::read_to_string(&path).expect("the PRD is in the repository");
    let line = prd
        .lines()
        .find(|line| line.starts_with("- **I2**"))
        .expect("the PRD has the I2 requirement");
    let (_, surface) = line
        .split_once("Tool surface includes at least: ")
        .expect("I2 lists the tool surface");
    surface
        .trim_end_matches('.')
        .split("; ")
        .map(str::to_owned)
        .collect()
}

/// Each clause of the PRD's tool surface, as the PRD words it, and the tools that cover it.
const COVERAGE: &[(&str, &[&str])] = &[
    (
        "list/get routes and journeys",
        &["list_routes", "get_route", "list_journeys", "get_snapshot"],
    ),
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
        "get the ranked frontier, decisions needed, needs breakdown, unassigned, active, blocked, stale, overdue, shortfall, and \"mine\"",
        &["list_nodes", "get_snapshot"],
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
        "draft, list, get and edit proposals",
        &[
            "create_proposal",
            "list_proposals",
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
    (
        "list identities by name or email, find the journeys that refer to an identity or that it owns, and manage identities (create, link, merge, delete)",
        &["list_identities", "list_journeys", "manage_identity"],
    ),
    ("route import/export", &["import_route", "export_route"]),
    ("save as route", &["save_as_route"]),
    ("re-link", &["relink"]),
    ("upgrade", &["upgrade"]),
    (
        "segments through the route tools (filtered by kind), insert a segment, upgrade an insertion, save as segment, and link an insertion",
        &[
            "list_routes",
            "create_proposal",
            "upgrade",
            "save_as_route",
            "relink",
        ],
    ),
    ("search", &["search"]),
];

/// I2: each clause of the PRD's minimum tool surface, and the tools that cover it. The PRD
/// owns the clauses: the table must hold exactly them, so a clause added, reworded, or
/// removed there fails here until its tools are named; every tool named must exist.
#[test]
fn every_clause_of_the_prd_tool_surface_has_a_tool() {
    let tools: BTreeSet<&str> = ToolSet::definitions().iter().map(|t| t.name).collect();
    for (clause, covered_by) in COVERAGE {
        assert!(!covered_by.is_empty(), "{clause}");
        for tool in *covered_by {
            assert!(tools.contains(tool), "{clause}: no tool {tool}");
        }
    }
    let table: BTreeSet<&str> = COVERAGE.iter().map(|(clause, _)| *clause).collect();
    let prd = prd_tool_surface();
    let prd: BTreeSet<&str> = prd.iter().map(String::as_str).collect();
    assert_eq!(
        table, prd,
        "the table covers the PRD's clauses, no more and no fewer"
    );
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
