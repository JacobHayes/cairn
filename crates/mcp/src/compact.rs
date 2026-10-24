//! The listing's compaction (ARCHITECTURE, MCP endpoint): one pass over a tool's generated
//! input schema that drops what an agent cannot act on, so the tool list an agent loads
//! costs little. The server enforces everything dropped here and a refusal names the one
//! broken, so the agent loses nothing it could have acted on.
//!
//! Doc comments and the OpenAPI document are untouched: they are read elsewhere, and this
//! pass works on a copy made for the list.
//!
//! What goes: the `$schema` and `title` keys and the arguments type's own doc line; `null`
//! as an alternative on a property that may be left out (absent and `null` mean the same
//! there; a required property that takes `null` keeps it, since `null` is then a value,
//! such as clearing a field); null and empty defaults; integer formats; length, count and
//! uniqueness bounds; the zero of a count; the descriptions of shared types, but for their union variants'
//! first sentence. Shared scalar types (ids, keys, text, revisions) are written where they
//! are used, keeping an id's `pattern`, and unions of constants become an `enum`.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// Keywords that bound a value without changing its shape.
const BOUNDS: &[&str] = &[
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "uniqueItems",
];

/// The keywords of a shared scalar type that are kept where it is written out.
const SCALAR_KEYWORDS: &[&str] = &["type", "pattern", "format", "minimum", "maximum"];

const DEFINITIONS: &str = "$defs";
const REFERENCE_PREFIX: &str = "#/$defs/";

/// Where in the schema a value sits.
#[derive(Clone, Copy, Default)]
struct Place {
    /// Within the shared definitions.
    in_definition: bool,
    /// A property that may be left out.
    optional: bool,
    /// A variant of a union.
    variant: bool,
}

/// `schema`, a tool's arguments, compacted. `said_on_purpose` names the shared definitions
/// whose description is the tool's own words (a stand-in for a type another tool spells
/// out), which stay as written.
#[must_use]
pub(crate) fn compact(
    mut schema: Map<String, Value>,
    said_on_purpose: &[&str],
) -> Map<String, Value> {
    schema.remove("description");
    let definitions = schema.remove(DEFINITIONS);
    let Value::Object(mut root) = walk(Value::Object(schema), Place::default()) else {
        unreachable!("an object stays an object");
    };
    if let Some(Value::Object(definitions)) = definitions {
        let definitions = definitions
            .into_iter()
            .map(|(name, definition)| {
                let place = Place {
                    in_definition: !said_on_purpose.contains(&name.as_str()),
                    ..Place::default()
                };
                (name, walk(definition, place))
            })
            .collect();
        root.insert(DEFINITIONS.to_owned(), Value::Object(definitions));
    }
    let mut root = inline_scalars(root);
    crate::toolset::prune_definitions(&mut root);
    if root
        .get(DEFINITIONS)
        .is_some_and(|d| d.as_object().is_some_and(Map::is_empty))
    {
        root.remove(DEFINITIONS);
    }
    root
}

/// The value of `keyword` in a schema, compacted.
fn walk_keyword(keyword: &str, value: Value, place: Place) -> Value {
    let inner = Place {
        optional: false,
        variant: false,
        ..place
    };
    match (keyword, value) {
        ("anyOf" | "oneOf", Value::Array(variants)) => Value::Array(
            variants
                .into_iter()
                .map(|variant| {
                    walk(
                        variant,
                        Place {
                            variant: true,
                            ..inner
                        },
                    )
                })
                .collect(),
        ),
        ("allOf" | "prefixItems", Value::Array(schemas)) => {
            Value::Array(schemas.into_iter().map(|s| walk(s, inner)).collect())
        }
        ("items" | "additionalProperties" | "not" | "if" | "then" | "else", value) => {
            walk(value, inner)
        }
        (_, value) => value,
    }
}

/// One schema, compacted; anything that is not a schema object stays as it is.
fn walk(schema: Value, place: Place) -> Value {
    let Value::Object(mut schema) = schema else {
        return schema;
    };
    schema.remove("$schema");
    schema.remove("title");
    for bound in BOUNDS {
        schema.remove(*bound);
    }
    if schema.get("default").is_some_and(is_empty) {
        schema.remove("default");
    }
    if schema
        .get("format")
        .and_then(Value::as_str)
        .is_some_and(|format| format.starts_with("uint") || format.starts_with("int"))
    {
        schema.remove("format");
    }
    // An unsigned count is not negative: its format said so, and was dropped.
    if schema
        .get("minimum")
        .is_some_and(|minimum| minimum.as_f64() == Some(0.0))
    {
        schema.remove("minimum");
    }
    if place.optional {
        drop_null(&mut schema);
    }
    // The required names are read before the schema's properties are walked.
    let required: BTreeSet<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|name| name.as_str().map(str::to_owned))
        .collect();
    let mut compacted = Map::new();
    for (keyword, value) in schema {
        let value = match (keyword.as_str(), value) {
            ("properties", Value::Object(properties)) => Value::Object(
                properties
                    .into_iter()
                    .map(|(name, property)| {
                        let place = Place {
                            optional: !required.contains(&name),
                            variant: false,
                            ..place
                        };
                        (name, walk(property, place))
                    })
                    .collect(),
            ),
            ("description", Value::String(description)) => {
                if place.in_definition && !place.variant {
                    continue;
                }
                let description = if place.in_definition {
                    first_sentence(&description)
                } else {
                    description
                };
                Value::String(description)
            }
            (keyword, value) => walk_keyword(keyword, value, place),
        };
        compacted.insert(keyword, value);
    }
    constants_to_enum(&mut compacted);
    Value::Object(compacted)
}

/// Whether a default says nothing: null, zero, false, or an empty string, list or object.
fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(fields) => fields.is_empty(),
        Value::Bool(flag) => !flag,
        Value::Number(number) => number.as_f64() == Some(0.0),
    }
}

/// Drops `null` as an alternative of a schema for a property that may be left out.
fn drop_null(schema: &mut Map<String, Value>) {
    let null = serde_json::json!({ "type": "null" });
    for keyword in ["anyOf", "oneOf"] {
        let Some(Value::Array(variants)) = schema.get(keyword) else {
            continue;
        };
        if !variants.contains(&null) {
            continue;
        }
        let mut rest: Vec<Value> = variants.iter().filter(|v| **v != null).cloned().collect();
        schema.remove(keyword);
        if rest.len() == 1 {
            if let Some(Value::Object(only)) = rest.pop() {
                for (key, value) in only {
                    schema.entry(key).or_insert(value);
                }
            }
        } else {
            schema.insert(keyword.to_owned(), Value::Array(rest));
        }
    }
    if let Some(Value::Array(types)) = schema.get_mut("type") {
        types.retain(|kind| kind != "null");
        if types.len() == 1 {
            let only = types.remove(0);
            schema.insert("type".to_owned(), only);
        }
    }
}

/// A union whose variants are constants becomes an `enum`: the names say what each means.
fn constants_to_enum(schema: &mut Map<String, Value>) {
    for keyword in ["anyOf", "oneOf"] {
        let Some(Value::Array(variants)) = schema.get(keyword) else {
            continue;
        };
        let bare = !variants.is_empty()
            && variants.iter().all(|variant| {
                variant.as_object().is_some_and(|variant| {
                    variant.contains_key("const")
                        && variant
                            .keys()
                            .all(|key| matches!(key.as_str(), "const" | "type" | "description"))
                })
            });
        if !bare {
            continue;
        }
        let constants: Vec<Value> = variants
            .iter()
            .filter_map(|variant| variant.get("const").cloned())
            .collect();
        let kinds: BTreeSet<&str> = variants
            .iter()
            .filter_map(|variant| variant.get("type").and_then(Value::as_str))
            .collect();
        let kind = match (kinds.len(), kinds.first()) {
            (1, Some(kind)) if variants.iter().all(|v| v.get("type").is_some()) => {
                Some((*kind).to_owned())
            }
            _ => None,
        };
        schema.remove(keyword);
        if let Some(kind) = kind {
            schema.entry("type").or_insert(Value::String(kind));
        }
        schema.insert("enum".to_owned(), Value::Array(constants));
    }
}

/// The first sentence of a description: up to the first full stop that ends a line or is
/// followed by a space, on one line.
fn first_sentence(description: &str) -> String {
    let paragraph = description.split("\n\n").next().unwrap_or_default();
    let line = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut inside_code = false;
    let mut end = line.len();
    let bytes = line.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'`' => inside_code = !inside_code,
            b'.' if !inside_code && bytes.get(index + 1).is_none_or(|next| *next == b' ') => {
                end = index + 1;
                break;
            }
            _ => {}
        }
    }
    line[..end].to_owned()
}

/// Writes each shared scalar type where it is used.
fn inline_scalars(root: Map<String, Value>) -> Map<String, Value> {
    let Some(Value::Object(definitions)) = root.get(DEFINITIONS) else {
        return root;
    };
    let scalars: Map<String, Value> = definitions
        .iter()
        .filter_map(|(name, definition)| {
            let definition = definition.as_object()?;
            let plain = matches!(
                definition.get("type").and_then(Value::as_str),
                Some("string" | "integer" | "number" | "boolean")
            ) && definition
                .keys()
                .all(|key| SCALAR_KEYWORDS.contains(&key.as_str()) || key == "description");
            plain.then(|| {
                let kept: Map<String, Value> = definition
                    .iter()
                    .filter(|(key, _)| SCALAR_KEYWORDS.contains(&key.as_str()))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                (name.clone(), Value::Object(kept))
            })
        })
        .collect();
    let mut value = Value::Object(root);
    inline(&mut value, &scalars);
    let Value::Object(root) = value else {
        unreachable!("an object stays an object");
    };
    root
}

fn inline(value: &mut Value, scalars: &Map<String, Value>) {
    match value {
        Value::Object(object) => {
            let scalar = object
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|target| target.strip_prefix(REFERENCE_PREFIX))
                .and_then(|name| scalars.get(name))
                .and_then(Value::as_object);
            if let Some(scalar) = scalar {
                object.remove("$ref");
                for (key, written) in scalar {
                    object.entry(key.clone()).or_insert_with(|| written.clone());
                }
            }
            for child in object.values_mut() {
                inline(child, scalars);
            }
        }
        Value::Array(items) => {
            for item in items {
                inline(item, scalars);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::compact;

    fn compacted(schema: &Value) -> Value {
        let Value::Object(schema) = schema.clone() else {
            unreachable!("a test schema is an object");
        };
        Value::Object(compact(schema, &[]))
    }

    /// A property that may be left out loses its `null`; a required one keeps it, since
    /// `null` is then a value (clearing a field); a property named `title` is not the
    /// keyword.
    #[test]
    fn null_goes_only_where_absent_means_the_same() {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Arguments",
            "description": "The arguments type's own doc line.",
            "type": "object",
            "properties": {
                "title": { "type": ["string", "null"], "description": "kept" },
                "cleared": { "anyOf": [{ "type": "string" }, { "type": "null" }] },
                "optional": { "anyOf": [{ "type": "string" }, { "type": "null" }] },
            },
            "required": ["cleared"],
        });
        assert_eq!(
            compacted(&schema),
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "kept" },
                    "cleared": { "anyOf": [{ "type": "string" }, { "type": "null" }] },
                    "optional": { "type": "string" },
                },
                "required": ["cleared"],
            })
        );
    }

    /// Defaults that say nothing, integer formats, and bounds go; a bound on a value goes
    /// no further than lengths, counts and uniqueness.
    #[test]
    fn what_the_server_enforces_goes() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": { "type": "integer", "format": "uint32", "minimum": 0, "default": 0 },
                "b": { "type": "array", "items": { "type": "string", "maxLength": 9 },
                       "maxItems": 3, "uniqueItems": true, "default": [] },
                "c": { "type": "integer", "minimum": 1, "maximum": 365, "default": 7 },
            },
        });
        assert_eq!(
            compacted(&schema)["properties"],
            json!({
                "a": { "type": "integer" },
                "b": { "type": "array", "items": { "type": "string" } },
                "c": { "type": "integer", "minimum": 1, "maximum": 365, "default": 7 },
            })
        );
    }

    /// A shared scalar is written where it is used, keeping an id's pattern; a union of
    /// constants becomes an `enum`; a shared type keeps only its variants' first sentence.
    #[test]
    fn shared_types_are_written_out_or_trimmed() {
        let schema = json!({
            "type": "object",
            "properties": {
                "journey": { "$ref": "#/$defs/JourneyKey" },
                "kind": { "$ref": "#/$defs/Kind" },
                "change": { "$ref": "#/$defs/Change" },
            },
            "$defs": {
                "JourneyKey": { "type": "string", "pattern": "^j_", "maxLength": 64,
                                "description": "A journey." },
                "Kind": { "oneOf": [
                    { "type": "string", "const": "a", "description": "The first." },
                    { "type": "string", "const": "b", "description": "The second." },
                ] },
                "Change": { "description": "Dropped.", "oneOf": [
                    { "type": "object", "description": "Adds it. Then says more.",
                      "properties": { "add": { "type": "string", "description": "Dropped." } } },
                ] },
            },
        });
        assert_eq!(
            compacted(&schema),
            json!({
                "type": "object",
                "properties": {
                    "journey": { "type": "string", "pattern": "^j_" },
                    "kind": { "$ref": "#/$defs/Kind" },
                    "change": { "$ref": "#/$defs/Change" },
                },
                "$defs": {
                    "Kind": { "type": "string", "enum": ["a", "b"] },
                    "Change": { "oneOf": [
                        { "type": "object", "description": "Adds it.",
                          "properties": { "add": { "type": "string" } } },
                    ] },
                },
            })
        );
    }
}
