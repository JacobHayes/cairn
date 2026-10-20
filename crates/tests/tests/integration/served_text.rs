//! No requirement id or document name reaches a reader. The ids (a letter and a number, such as `A2`, or
//! with a letter after, such as `D1a`) are how developers cite the PRD from code; the text
//! that people and agents read says the rule in words. This holds every description the
//! API document, the route-file schema and the MCP tools serve, and the agent guide, to
//! that, so a doc comment that becomes a description cannot bring an id along. The
//! violation messages are held to it where the engine tests apply patches
//! (`engine::support::apply`).
#![cfg(test)]

use serde_json::Value;

/// The requirement ids in `text`: a letter from A to J, one or two digits and an optional
/// lowercase letter, standing alone as a word.
pub fn requirement_ids(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'`';
    let mut found = Vec::new();
    for start in 0..bytes.len() {
        if !(b'A'..=b'J').contains(&bytes[start]) || (start > 0 && word(bytes[start - 1])) {
            continue;
        }
        let digits = bytes[start + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if !(1..=2).contains(&digits) {
            continue;
        }
        let mut end = start + 1 + digits;
        if bytes.get(end).is_some_and(u8::is_ascii_lowercase) {
            end += 1;
        }
        if bytes.get(end).is_none_or(|byte| !word(*byte)) {
            found.push(&text[start..end]);
        }
    }
    found
}

/// The names of the repository's documents in `text` (the PRD and the others), which a reader
/// of served text cannot open.
pub fn document_names(text: &str) -> Vec<&str> {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|word| matches!(*word, "PRD" | "ARCHITECTURE" | "PRACTICES" | "DESIGN"))
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

fn assert_no_ids(served: &str, document: &Value) {
    let mut texts = Vec::new();
    prose(document, "", &mut texts);
    let found: Vec<String> = texts
        .iter()
        .filter(|(_, text)| !requirement_ids(text).is_empty() || !document_names(text).is_empty())
        .map(|(path, text)| format!("{served}{path}: {text}"))
        .collect();
    assert!(
        found.is_empty(),
        "requirement ids or document names in served text:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_detector_finds_ids_and_only_ids() {
    let cases = [
        ("a decision has no children (A2)", vec!["A2"]),
        ("pinned by answering it (E3, F5)", vec!["E3", "F5"]),
        ("D1a: the states", vec!["D1a"]),
        ("a route file at version 2", vec![]),
        ("the `A2` column and snake_b2 and ABC1", vec![]),
        ("range K3 and a30", vec![]),
    ];
    for (text, ids) in cases {
        assert_eq!(requirement_ids(text), ids, "{text}");
    }
    assert_eq!(
        document_names("a node (PRD glossary, Pin; ARCHITECTURE, Terms)"),
        ["PRD", "ARCHITECTURE"]
    );
    assert_eq!(
        document_names("a PRDX, a prd and an ARCHITECTURES"),
        [] as [&str; 0]
    );
}

#[test]
fn the_api_document_describes_without_ids() {
    assert_no_ids("openapi", &cairn_api::openapi::document());
}

#[test]
fn the_route_file_schema_describes_without_ids() {
    for (name, content) in cairn_schema::json_schema::generated_files() {
        let document: Value = serde_json::from_str(&content).expect("a generated schema is JSON");
        assert_no_ids(name, &document);
    }
}

#[test]
fn the_mcp_tools_describe_without_ids() {
    for tool in cairn_mcp::ToolSet::definitions() {
        let document = serde_json::json!({
            "description": tool.description,
            "input": tool.input_schema,
            "output": tool.output_schema,
        });
        assert_no_ids(tool.name, &document);
    }
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
        assert!(
            requirement_ids(&text).is_empty(),
            "{name}: {:?}",
            requirement_ids(&text)
        );
    }
}
