//! The shipped instructions against the tool surface (I4): every identifier the guide and its
//! workflows name is a tool, or a field or value some tool takes or answers.
#![cfg(test)]

use std::collections::BTreeSet;

use cairn_mcp::instructions::{self, SKILL};
use cairn_mcp::{ToolSet, limits};
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
        definitions
            .iter()
            .map(|t| (t.name.to_owned(), t.description)),
    );
    texts.extend(
        instructions::workflows()
            .into_iter()
            .map(|w| (w.name.to_owned(), w.text)),
    );
    for (file, text) in texts {
        for identifier in identifiers(text) {
            assert!(
                known.contains(&identifier),
                "{file} names `{identifier}`, which no tool has"
            );
        }
    }
}

/// The served guide stays within the size a client shows of server instructions: whatever
/// governs one tool is on that tool.
#[test]
fn the_guide_stays_within_its_size() {
    let bytes = u32::try_from(instructions::guide().len()).unwrap();
    assert!(
        bytes <= limits::GUIDE_BYTES_MAX,
        "the guide is {bytes} bytes, past {}",
        limits::GUIDE_BYTES_MAX
    );
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
            Some((state, cells.next()?.trim_end_matches(" |").to_owned()))
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
