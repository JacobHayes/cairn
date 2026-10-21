//! No requirement id or document name reaches a reader. The ids (a letter and a number, such as `A2`, or
//! with a letter after, such as `D1a`) are how developers cite the PRD from code; the text
//! that people and agents read says the rule in words. This holds every description the
//! API document, the route-file schema and the MCP tools serve, and the agent guide, to
//! that, so a doc comment that becomes a description cannot bring an id along. The
//! violation messages are held to it where the engine tests apply patches
//! (`engine::support::apply`).
#![cfg(test)]

use std::path::Path;
use std::sync::LazyLock;

use serde_json::Value;

/// The generated patterns of what a reader cannot follow (`mise run gen` writes them beside
/// the engine's rule tables, from the repository's documents; the browser tests read the
/// same), each as a JSON Schema `pattern`, which is an ECMAScript regular expression as the
/// browser's are.
static CITATIONS: LazyLock<Vec<(&'static str, jsonschema::Validator)>> = LazyLock::new(|| {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/client/generated/rules.ts");
    let rules = std::fs::read_to_string(&path).expect("the generated rule tables read");
    [
        ("a requirement id", "REQUIREMENT_ID_PATTERN"),
        ("a document name", "DOCUMENT_NAME_PATTERN"),
    ]
    .into_iter()
    .map(|(what, name)| {
        let line = rules
            .lines()
            .find_map(|line| line.strip_prefix(&format!("export const {name} = ")))
            .unwrap_or_else(|| panic!("the rule tables export {name}"));
        let pattern: String = serde_json::from_str(line.trim_end_matches(';'))
            .unwrap_or_else(|error| panic!("{name} is a string: {error}"));
        let schema = serde_json::json!({ "type": "string", "pattern": pattern });
        (
            what,
            jsonschema::draft202012::new(&schema).expect("the pattern compiles"),
        )
    })
    .collect()
});

/// What `text` cites that a reader cannot follow: a requirement id or a document name, by
/// kind.
pub fn citations(text: &str) -> Vec<&'static str> {
    let text = Value::String(text.to_owned());
    CITATIONS
        .iter()
        .filter(|(_, pattern)| pattern.is_valid(&text))
        .map(|(what, _)| *what)
        .collect()
}

/// Every string a JSON document serves as prose, with the path to it. Schema keywords whose
/// values are identifiers (`$ref`, `pattern`, `enum`, `const`, `required`, property names)
/// are not prose.
fn prose(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::String(text) => out.push((path.to_owned(), text.clone())),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                prose(item, &format!("{path}/{index}"), out);
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                if matches!(
                    key.as_str(),
                    "$ref" | "pattern" | "enum" | "const" | "required" | "format" | "operationId"
                ) {
                    continue;
                }
                prose(item, &format!("{path}/{key}"), out);
            }
        }
        _ => {}
    }
}

/// Every prose string in `document` that cites what a reader cannot follow, with where it is
/// served from (`served`) and its path in the document.
fn cited(served: &str, document: &Value) -> Vec<String> {
    let mut texts = Vec::new();
    prose(document, "", &mut texts);
    texts
        .iter()
        .filter(|(_, text)| !citations(text).is_empty())
        .map(|(path, text)| format!("{served}{path}: {text}"))
        .collect()
}

fn assert_no_ids(found: &[String]) {
    assert!(
        found.is_empty(),
        "requirement ids or document names in served text:\n{}",
        found.join("\n")
    );
}

/// The generated patterns find a requirement id or a document name standing alone, and
/// nothing that only looks like one: code in backticks, a longer word, or another letter.
#[test]
fn the_detector_finds_citations_and_only_citations() {
    let cases = [
        ("a decision has no children (A2)", vec!["a requirement id"]),
        ("pinned by answering it (E3, F5)", vec!["a requirement id"]),
        ("D1a: the states", vec!["a requirement id"]),
        ("a node (PRD glossary, Pin)", vec!["a document name"]),
        (
            "see ARCHITECTURE, Terms, and C16",
            vec!["a requirement id", "a document name"],
        ),
        ("a route file at version 2", vec![]),
        ("the `A2` column and snake_b2 and ABC1 and A123", vec![]),
        ("range Z3 and a30", vec![]),
        ("a PRDX, a prd and an ARCHITECTURES", vec![]),
    ];
    for (text, found) in cases {
        assert_eq!(citations(text), found, "{text}");
    }
}

#[test]
fn the_api_document_describes_without_ids() {
    assert_no_ids(&cited("openapi", &cairn_api::openapi::document()));
}

#[test]
fn the_route_file_schema_describes_without_ids() {
    for (name, content) in cairn_schema::json_schema::generated_files() {
        let document: Value = serde_json::from_str(&content).expect("a generated schema is JSON");
        assert_no_ids(&cited(name, &document));
    }
}

/// Every tool's description and its whole input and output schemas, their definitions and
/// every property's description included, all reported together.
#[test]
fn the_mcp_tools_describe_without_ids() {
    let mut found = Vec::new();
    for tool in cairn_mcp::ToolSet::definitions() {
        let document = serde_json::json!({
            "description": tool.description,
            "input": tool.input_schema,
            "output": tool.output_schema,
        });
        found.extend(cited(tool.name, &document));
    }
    assert_no_ids(&found);
}

#[test]
fn the_agent_guide_and_workflows_say_rules_in_words() {
    let mut served = vec![(
        "guide".to_owned(),
        cairn_mcp::instructions::SKILL.to_owned(),
    )];
    for workflow in cairn_mcp::instructions::workflows() {
        served.push((workflow.name.to_owned(), workflow.text.to_owned()));
        served.push((
            format!("{} description", workflow.name),
            workflow.description.to_owned(),
        ));
    }
    for (name, text) in served {
        assert_eq!(citations(&text), [] as [&str; 0], "{name}");
    }
}
