//! The mutation vocabulary as a type sketch: `apply_patch`'s mutation schema written as
//! compact type signatures, one line per `op`, then each shared type once. An agent reads
//! the sketch for about a quarter of the tokens of the JSON Schema it is generated from.
//!
//! The renderer supports exactly the constructs Cairn's schemas use and refuses any other
//! with [`Unsupported`], so a new construct is a failing listing rather than a silently
//! wrong sketch. Requiredness and nullability are told apart: `field: T` is required,
//! `field?: T` may be left out, and `T | null` takes `null` as a value (a required field
//! that clears something). An optional field's `null` means the same as leaving it out, so
//! it is not written.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use serde_json::{Map, Value};

/// A schema construct Cairn's schemas do not use, and the sketch does not render.
#[derive(Debug)]
pub(crate) struct Unsupported(String);

impl fmt::Display for Unsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported schema construct: {}", self.0)
    }
}

/// The groups of a vocabulary, by first op and header, and which of its ops are listed.
pub(crate) type Listing<'a> = (&'a [(&'a str, &'a str)], &'a dyn Fn(&str) -> bool);

/// The sketch, the op names it covers in the schema's order, and every field name any of
/// those ops takes.
#[derive(Debug)]
pub(crate) struct MutationSketch {
    pub text: String,
    pub ops: Vec<String>,
    pub fields: BTreeSet<String>,
}

/// A rendering, or the construct that stopped it.
type Rendered<T = String> = Result<T, Unsupported>;

fn unsupported<T>(what: impl fmt::Display) -> Rendered<T> {
    Err(Unsupported(what.to_string()))
}

/// A comment longer than this many bytes is cut at its first clause break at least
/// [`MIN_COMMENT`] bytes in.
const LONG_COMMENT: usize = 90;
const MIN_COMMENT: usize = 20;

/// Keywords that bound or annotate a value without changing its shape: the server enforces
/// them and a refusal names the one broken.
const IGNORED: &[&str] = &[
    "description",
    "title",
    "default",
    "$schema",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "minProperties",
    "maxProperties",
    "minimum",
    "maximum",
    "uniqueItems",
    "pattern",
    "format",
    "x-sketch",
];

/// Keywords the renderer reads.
const READ: &[&str] = &[
    "$ref",
    "anyOf",
    "oneOf",
    "allOf",
    "enum",
    "const",
    "type",
    "items",
    "properties",
    "required",
    "additionalProperties",
    "patternProperties",
];

/// The mutation vocabulary's purposes, each named by the op that starts it in the schema's
/// order and the header the sketch writes above its ops, so an agent reading for one job
/// finds its ops together.
pub(crate) const GROUPS: &[(&str, &str)] = &[
    ("create_journey", "The journey"),
    ("create_route", "Routes and their drafts"),
    ("add_node", "Nodes and edges"),
    ("add_role", "Roles and participation kinds"),
    ("set_participation", "Who does a node"),
    ("add_resource", "A node's resources"),
    ("insert_segment", "Segments"),
    (
        "transition",
        "What happened: progress, answers, dates, snoozes, overrides, ticks",
    ),
    ("add_annotation", "Notes and links"),
    (
        "create_identity",
        "Identities (create_identity also rides with a journey write)",
    ),
    ("delete_journey", "Deleting for good (the deployment)"),
    ("create_proposal", "Proposals"),
];

/// The sketch of the mutation schema `mutation` (a union of objects each tagged by an `op`
/// constant), named `root` among the shared types `definitions` it reaches: the ops `listed`
/// keeps, under the header of the group (of `groups`, by first op) each falls in, and only
/// the shared types those ops reach.
///
/// # Errors
///
/// [`Unsupported`] on a schema construct Cairn's schemas do not use, or a vocabulary that
/// does not open with a group's first op.
pub(crate) fn mutation_sketch(
    definitions: &Map<String, Value>,
    root: &str,
    mutation: &Value,
    (groups, listed): Listing<'_>,
) -> Rendered<MutationSketch> {
    let mut sketch = Sketch {
        definitions,
        root,
        named: BTreeMap::new(),
        order: Vec::new(),
        key_prefixes: BTreeMap::new(),
    };
    let mut lines = Vec::new();
    let mut ops = Vec::new();
    let mut fields = BTreeSet::new();
    let mut group: Option<(&str, bool)> = None;
    for variant in array(mutation, "oneOf")? {
        let op = variant
            .pointer("/properties/op/const")
            .and_then(Value::as_str)
            .map_or_else(
                || unsupported(format_args!("a variant without an op: {variant}")),
                Ok,
            )?;
        if let Some((_, header)) = groups.iter().find(|(first, _)| *first == op) {
            group = Some((header, false));
        }
        let Some((header, written)) = group.as_mut() else {
            return unsupported(format_args!("`{op}` comes before any group's first op"));
        };
        if !listed(op) {
            continue;
        }
        if !*written {
            lines.push(format!("// {header}"));
            *written = true;
        }
        let (op, line) = sketch.variant(variant)?;
        if let Some(Value::Object(properties)) = variant.get("properties") {
            fields.extend(properties.keys().cloned());
        }
        ops.push(op);
        lines.push(line);
    }
    for name in &sketch.order {
        if let Some(Some(body)) = sketch.named.get(name) {
            lines.push(format!("type {name} = {body}"));
        }
    }
    if !sketch.key_prefixes.is_empty() {
        let keys: Vec<String> = sketch
            .key_prefixes
            .iter()
            .map(|(name, prefix)| format!("{name} {prefix}…"))
            .collect();
        lines.push(format!("Keys: {}", keys.join(", ")));
    }
    Ok(MutationSketch {
        text: lines.join("\n"),
        ops,
        fields,
    })
}

/// A description's first sentence, on one line.
fn first_sentence(description: &str) -> String {
    let flat = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let end = [". ", "; ", ": "]
        .iter()
        .filter_map(|stop| flat.find(stop))
        .min()
        .unwrap_or(flat.len());
    let mut sentence = &flat[..end];
    if sentence.len() > LONG_COMMENT {
        // A long sentence says its point before the first clause break.
        let cut = [", ", " ("]
            .iter()
            .filter_map(|stop| {
                let tail = sentence.get(MIN_COMMENT..)?;
                tail.find(stop).map(|at| at + MIN_COMMENT)
            })
            .min();
        if let Some(cut) = cut {
            sentence = &sentence[..cut];
        }
    }
    sentence.trim_end_matches(['.', ';', ':']).to_owned()
}

fn array<'a>(value: &'a Value, keyword: &str) -> Rendered<&'a [Value]> {
    match value.get(keyword) {
        Some(Value::Array(items)) => Ok(items),
        _ => unsupported(format_args!("`{keyword}` is not an array in {value}")),
    }
}

/// The key prefix a pattern such as `^n_[a-z0-9]...` names, if it names one.
fn key_prefix(pattern: &str) -> Option<&str> {
    let rest = pattern.strip_prefix('^')?;
    let letters = rest.find(|c: char| !c.is_ascii_lowercase())?;
    (letters > 0 && rest[letters..].starts_with('_')).then(|| &rest[..=letters])
}

/// Whether `text` has a ` | ` outside any bracket.
fn is_union(text: &str) -> bool {
    let mut depth = 0_i32;
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'{' | b'[' | b'(' => depth += 1,
            b'}' | b']' | b')' => depth -= 1,
            b'|' if depth == 0 && index > 0 && bytes[index - 1] == b' ' => return true,
            _ => {}
        }
    }
    false
}

/// Fails on a keyword the renderer neither reads nor knows to be an annotation.
fn check_keywords(schema: &Value, map: &Map<String, Value>) -> Rendered<()> {
    match map
        .keys()
        .find(|key| !IGNORED.contains(&key.as_str()) && !READ.contains(&key.as_str()))
    {
        Some(key) => unsupported(format_args!("keyword `{key}` in {schema}")),
        None => Ok(()),
    }
}

struct Sketch<'a> {
    definitions: &'a Map<String, Value>,
    /// The name of the schema the sketch is of, which others may refer back to.
    root: &'a str,
    /// Shared types rendered so far; `None` while one is being rendered (recursion).
    named: BTreeMap<String, Option<String>>,
    /// The shared types, in the order they finished rendering.
    order: Vec<String>,
    /// Keyed-id types and the prefix their values start with.
    key_prefixes: BTreeMap<String, String>,
}

impl Sketch<'_> {
    /// One line of the sketch: an op, its fields, and its description's first sentence.
    fn variant(&mut self, variant: &Value) -> Rendered<(String, String)> {
        if let Value::Object(map) = variant {
            check_keywords(variant, map)?;
        }
        let op = variant
            .pointer("/properties/op/const")
            .and_then(Value::as_str)
            .map_or_else(
                || unsupported(format_args!("a variant without an op: {variant}")),
                Ok,
            )?;
        let mut body = variant.clone();
        if let Value::Object(body) = &mut body {
            if let Some(Value::Object(properties)) = body.get_mut("properties") {
                properties.remove("op");
            }
            if let Some(Value::Array(required)) = body.get_mut("required") {
                required.retain(|name| name != "op");
            }
        }
        let signature = self.object(&body)?;
        let comment = variant
            .get("description")
            .and_then(Value::as_str)
            .map(first_sentence)
            .filter(|sentence| !sentence.is_empty())
            .map(|sentence| format!("  // {sentence}"))
            .unwrap_or_default();
        Ok((op.to_owned(), format!("{op} {signature}{comment}")))
    }

    /// A schema as a type, `| null` when it accepts null.
    fn render(&mut self, schema: &Value) -> Rendered {
        let (inner, nullable) = split_null(schema)?;
        let text = self.inner(&inner)?;
        Ok(if nullable {
            format!("{text} | null")
        } else {
            text
        })
    }

    fn inner(&mut self, schema: &Value) -> Rendered {
        let Value::Object(map) = schema else {
            return unsupported(schema);
        };
        check_keywords(schema, map)?;
        if let Some(Value::String(target)) = map.get("$ref") {
            if let Some(other) = map
                .keys()
                .find(|key| *key != "$ref" && !IGNORED.contains(&key.as_str()))
            {
                return unsupported(format_args!("`{other}` beside a reference in {schema}"));
            }
            return self.reference(target);
        }
        if map.contains_key("anyOf") {
            return self.union(array(schema, "anyOf")?);
        }
        if map.contains_key("oneOf") && !map.contains_key("properties") {
            return self.union(array(schema, "oneOf")?);
        }
        if let Some(Value::Array(values)) = map.get("enum") {
            return Ok(values
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(" | "));
        }
        if let Some(value) = map.get("const") {
            return Ok(value.to_string());
        }
        match map.get("type").and_then(Value::as_str) {
            Some("array") => {
                let items = map.get("items").map_or_else(|| unsupported(schema), Ok)?;
                let item = self.render(items)?;
                Ok(if is_union(&item) {
                    format!("({item})[]")
                } else {
                    format!("{item}[]")
                })
            }
            Some("object") => self.object(schema),
            Some("integer") => Ok("int".to_owned()),
            Some("number") => Ok("number".to_owned()),
            Some("boolean") => Ok("bool".to_owned()),
            Some("string") => match map.get("format").and_then(Value::as_str) {
                None => Ok("string".to_owned()),
                Some("date") => Ok("date".to_owned()),
                Some(other) => unsupported(format_args!("string format `{other}`")),
            },
            _ => unsupported(schema),
        }
    }

    fn union(&mut self, members: &[Value]) -> Rendered {
        if members.is_empty() {
            return unsupported("an empty union");
        }
        let mut parts = Vec::new();
        for member in members {
            parts.push(format!("{}{}", self.render(member)?, note(member)));
        }
        Ok(parts.join(" | "))
    }

    /// A shared type: a scalar inline (a keyed id by its name), anything else by name, its
    /// definition rendered once.
    fn reference(&mut self, target: &str) -> Rendered {
        let Some(name) = target.strip_prefix("#/$defs/") else {
            return unsupported(format_args!("reference `{target}`"));
        };
        if name == self.root {
            // The sketch itself, spelled out once above.
            return Ok(name.to_owned());
        }
        let Some(definition) = self.definitions.get(name) else {
            return unsupported(format_args!(
                "a reference to the missing definition `{name}`"
            ));
        };
        let is_scalar = matches!(
            definition.get("type").and_then(Value::as_str),
            Some("string" | "integer" | "number" | "boolean")
        ) && !definition.as_object().is_some_and(|map| {
            map.contains_key("enum") || map.contains_key("oneOf") || map.contains_key("const")
        });
        if is_scalar {
            let prefix = definition
                .get("pattern")
                .and_then(Value::as_str)
                .and_then(key_prefix);
            if let Some(prefix) = prefix {
                if let Value::Object(map) = definition {
                    check_keywords(definition, map)?;
                }
                self.key_prefixes.insert(name.to_owned(), prefix.to_owned());
                return Ok(name.to_owned());
            }
            return self.inner(definition);
        }
        if !self.named.contains_key(name) {
            self.named.insert(name.to_owned(), None);
            let mut body = self.render(definition)?;
            if body == "{}"
                && let Some(note) = definition.get("description").and_then(Value::as_str)
            {
                // An opaque type says what it is, since its fields are not written.
                body = format!(
                    "{{}} /* {} */",
                    note.split_whitespace().collect::<Vec<_>>().join(" ")
                );
            }
            self.named.insert(name.to_owned(), Some(body));
            self.order.push(name.to_owned());
        }
        Ok(name.to_owned())
    }

    /// An object schema: its fields, a group of which one is required, and the per-kind
    /// rules it carries; or a map.
    fn object(&mut self, schema: &Value) -> Rendered {
        let Value::Object(map) = schema else {
            return unsupported(schema);
        };
        check_keywords(schema, map)?;
        let empty = Map::new();
        let properties = match map.get("properties") {
            None => &empty,
            Some(Value::Object(properties)) => properties,
            Some(other) => return unsupported(format_args!("`properties`: {other}")),
        };
        if properties.is_empty() {
            return self.map_type(map);
        }
        if map.get("additionalProperties") != Some(&Value::Bool(false)) {
            return unsupported(format_args!(
                "an object with fields that is not closed: {schema}"
            ));
        }
        let required: Vec<&str> = match map.get("required") {
            None => Vec::new(),
            Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).collect(),
            Some(other) => return unsupported(format_args!("`required`: {other}")),
        };
        let group = one_of_group(map)?;
        let mut fields = Vec::new();
        for (name, property) in properties {
            if group.contains(&name.as_str()) {
                continue;
            }
            if required.contains(&name.as_str()) {
                fields.push(format!(
                    "{name}: {}{}",
                    self.render(property)?,
                    note(property)
                ));
            } else {
                // Absent and null mean the same on an optional field.
                let (inner, _) = split_null(property)?;
                fields.push(format!(
                    "{name}?: {}{}",
                    self.inner(&inner)?,
                    note(property)
                ));
            }
        }
        if !group.is_empty() {
            let mut alternatives = Vec::new();
            for name in &group {
                let Some(property) = properties.get(*name) else {
                    return unsupported(format_args!("a group names the missing field `{name}`"));
                };
                alternatives.push(format!("{name}: {}", self.render(property)?));
            }
            fields.push(format!("one of {}", alternatives.join(" | ")));
        }
        let body = format!("{{{}}}", fields.join(", "));
        Ok(match rules(map, properties)? {
            Some(rules) => format!("{body} /* {rules} */"),
            None => body,
        })
    }

    /// An object with no fields: a map by pattern or by any key, or the empty object.
    fn map_type(&mut self, map: &Map<String, Value>) -> Rendered {
        match (
            map.get("additionalProperties"),
            map.get("patternProperties"),
        ) {
            (Some(values @ Value::Object(_)), None) => {
                Ok(format!("{{[key]: {}}}", self.render(values)?))
            }
            (Some(Value::Bool(false)) | None, Some(Value::Object(patterns)))
                if patterns.len() == 1 =>
            {
                let mut entries = patterns.iter();
                let (Some((pattern, value)), None) = (entries.next(), entries.next()) else {
                    return unsupported("pattern properties");
                };
                let Some(prefix) = key_prefix(pattern) else {
                    return unsupported(format_args!("a key pattern without a prefix: {pattern}"));
                };
                Ok(format!("{{[{prefix}key]: {}}}", self.render(value)?))
            }
            (None | Some(Value::Bool(false)), None) => Ok("{}".to_owned()),
            _ => unsupported(format_args!("a map: {}", Value::Object(map.clone()))),
        }
    }
}

/// The description of a field or union member that marks itself `x-sketch`, as a trailing
/// comment: the rule a caller would get wrong from the type alone.
fn note(schema: &Value) -> String {
    if schema.get("x-sketch") != Some(&Value::Bool(true)) {
        return String::new();
    }
    schema
        .get("description")
        .and_then(Value::as_str)
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
        .map_or_else(String::new, |text| format!(" /* {text} */"))
}

/// A schema without its `null` alternative, and whether it had one.
fn split_null(schema: &Value) -> Rendered<(Value, bool)> {
    let Value::Object(map) = schema else {
        return Ok((schema.clone(), false));
    };
    let is_null = |value: &Value| value == &serde_json::json!({ "type": "null" });
    if let Some(Value::Array(members)) = map.get("anyOf")
        && members.iter().any(is_null)
    {
        let mut rest: Vec<Value> = members
            .iter()
            .filter(|member| !is_null(member))
            .cloned()
            .collect();
        if let Some(other) = map
            .keys()
            .find(|key| *key != "anyOf" && !IGNORED.contains(&key.as_str()))
        {
            return unsupported(format_args!(
                "`{other}` beside a nullable union in {schema}"
            ));
        }
        return match rest.len() {
            0 => unsupported(format_args!("a union of nothing but null: {schema}")),
            1 => Ok((rest.remove(0), true)),
            _ => {
                let mut inner = map.clone();
                inner.insert("anyOf".to_owned(), Value::Array(rest));
                Ok((Value::Object(inner), true))
            }
        };
    }
    if let Some(Value::Array(types)) = map.get("type")
        && types.iter().any(|name| name == "null")
    {
        let rest: Vec<&Value> = types.iter().filter(|name| *name != "null").collect();
        let [only] = rest.as_slice() else {
            return unsupported(format_args!("a list of types: {schema}"));
        };
        let mut inner = map.clone();
        inner.insert("type".to_owned(), (*only).clone());
        return Ok((Value::Object(inner), true));
    }
    Ok((schema.clone(), false))
}

/// The fields of which exactly one is required: `oneOf: [{required: [a]}, {required: [b]}]`.
fn one_of_group(map: &Map<String, Value>) -> Rendered<Vec<&str>> {
    let Some(Value::Array(alternatives)) = map.get("oneOf") else {
        return Ok(Vec::new());
    };
    let mut names = Vec::new();
    for alternative in alternatives {
        let name = match alternative {
            Value::Object(only) if only.len() == 1 => match only.get("required") {
                Some(Value::Array(required)) if required.len() == 1 => required[0].as_str(),
                _ => None,
            },
            _ => None,
        };
        match name {
            Some(name) => names.push(name),
            None => return unsupported(format_args!("an alternative: {alternative}")),
        }
    }
    Ok(names)
}

/// What one `allOf` rule says: `if kind` or `if answer_type` is a constant, `then` bans
/// some fields (`false`) and needs others.
struct Rule {
    kind: Option<String>,
    answer_type: Option<String>,
    banned: Vec<String>,
    needs: Vec<String>,
}

fn rule(rule: &Value) -> Rendered<Rule> {
    let only = |value: Option<&Value>, allowed: &[&str]| {
        value
            .and_then(Value::as_object)
            .is_some_and(|map| map.keys().all(|key| allowed.contains(&key.as_str())))
    };
    let keys_ok = rule.as_object().is_some_and(|map| {
        map.len() == 2
            && only(map.get("if"), &["properties", "required"])
            && only(map.get("then"), &["properties", "required"])
    });
    if !keys_ok {
        return unsupported(format_args!("a rule: {rule}"));
    }
    let condition = rule
        .pointer("/if/properties")
        .and_then(Value::as_object)
        .map_or_else(|| unsupported(format_args!("a rule: {rule}")), Ok)?;
    let Some(then) = rule.get("then") else {
        return unsupported(format_args!("a rule: {rule}"));
    };
    let mut banned = Vec::new();
    if let Some(Value::Object(properties)) = then.get("properties") {
        for (name, value) in properties {
            if value != &Value::Bool(false) {
                return unsupported(format_args!("a rule: {rule}"));
            }
            banned.push(name.clone());
        }
    }
    let needs = then
        .get("required")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let constant = |field: &str| {
        condition
            .get(field)
            .and_then(|value| value.get("const"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let found = Rule {
        kind: constant("kind"),
        answer_type: constant("answer_type"),
        banned,
        needs,
    };
    if condition.len() != 1 || found.kind.is_some() == found.answer_type.is_some() {
        return unsupported(format_args!("a rule: {rule}"));
    }
    Ok(found)
}

/// What an `allOf` of `if kind`/`if answer_type` rules says of an object's fields, as
/// what each kind and each answer type takes and needs.
fn rules(map: &Map<String, Value>, properties: &Map<String, Value>) -> Rendered<Option<String>> {
    let Some(Value::Array(all_of)) = map.get("allOf") else {
        return Ok(None);
    };
    let mut banned_by_kind: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut needs_by_kind: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut kind_order = Vec::new();
    let mut by_answer = Vec::new();
    for found in all_of.iter().map(rule) {
        let Rule {
            kind,
            answer_type,
            mut banned,
            needs,
        } = found?;
        if let Some(kind) = kind {
            if !kind_order.contains(&kind) {
                kind_order.push(kind.clone());
            }
            banned_by_kind
                .entry(kind.clone())
                .or_default()
                .extend(banned);
            needs_by_kind.entry(kind).or_default().extend(needs);
        } else if let Some(answer_type) = answer_type {
            let mut parts = Vec::new();
            if !needs.is_empty() {
                parts.push(format!("needs {}", needs.join(", ")));
            }
            if !banned.is_empty() {
                banned.sort();
                parts.push(format!("no {}", banned.join(", ")));
            }
            by_answer.push(format!("{answer_type}: {}", parts.join(", ")));
        }
    }
    let banned_for = |kind: &String, name: &str| banned_by_kind[kind].iter().any(|b| b == name);
    let kinds: Vec<String> = kind_order
        .iter()
        .map(|kind| {
            let own: Vec<&str> = properties
                .keys()
                .map(String::as_str)
                .filter(|name| !banned_for(kind, name))
                // Beyond the common ones: some kind bans it.
                .filter(|name| kind_order.iter().any(|other| banned_for(other, name)))
                .collect();
            let needs = &needs_by_kind[kind];
            let needs = if needs.is_empty() {
                String::new()
            } else {
                format!(" (needs {})", needs.join(", "))
            };
            format!("{kind}: {}{needs}", own.join(", "))
        })
        .collect();
    let mut text = format!(
        "fields beyond the common ones, by kind: {}",
        kinds.join("; ")
    );
    if !by_answer.is_empty() {
        let _ = write!(text, ". By answer_type: {}", by_answer.join("; "));
    }
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::collections::BTreeMap;

    use super::*;
    use crate::ToolSet;
    use crate::toolset::schema;

    /// Every op of the vocabulary, under its groups.
    fn every() -> Listing<'static> {
        (GROUPS, &|_| true)
    }

    /// A probe vocabulary's one group.
    fn probes() -> Listing<'static> {
        (&[("probe", "Probes")], &|_| true)
    }

    /// The full mutation schema and the definitions it reaches.
    fn full() -> (Map<String, Value>, Value) {
        let patch = schema::<cairn_schema::Patch>();
        let Some(Value::Object(definitions)) = patch.get("$defs") else {
            panic!("a patch has shared definitions");
        };
        let mut definitions = definitions.clone();
        // As the tool's schema has it: Cairn drafts review items, so they are cited.
        let (name, citation) = crate::toolset::REVIEW_ITEM;
        definitions.insert(
            name.to_owned(),
            json!({ "type": "object", "description": citation }),
        );
        let mutation = definitions["Mutation"].clone();
        (definitions, mutation)
    }

    /// The fields of an object type written `{a: T, b?: U}`, as (name, optional, type).
    fn fields(object: &str) -> Vec<(String, bool, String)> {
        let mut bare = String::new();
        let mut rest = object;
        while let Some(at) = rest.find(" /* ") {
            bare.push_str(&rest[..at]);
            let close = rest[at..].find(" */").expect("a closed note");
            rest = &rest[at + close + 3..];
        }
        bare.push_str(rest);
        let end = bare.rfind('}').expect("a closing brace");
        let inner = &bare[1..end];
        let (mut depth, mut start, mut parts) = (0_i32, 0, Vec::new());
        for (at, byte) in inner.bytes().enumerate() {
            match byte {
                b'{' | b'[' | b'(' => depth += 1,
                b'}' | b']' | b')' => depth -= 1,
                b',' if depth == 0 => {
                    parts.push(&inner[start..at]);
                    start = at + 2;
                }
                _ => {}
            }
        }
        if start < inner.len() {
            parts.push(&inner[start..]);
        }
        parts
            .into_iter()
            .map(|part| {
                let (name, kind) = part.split_once(": ").expect("a field has a type");
                match name.strip_suffix('?') {
                    Some(name) => (name.to_owned(), true, kind.to_owned()),
                    None => (name.to_owned(), false, kind.to_owned()),
                }
            })
            .collect()
    }

    /// Each field of `schema`, as the sketch writes `object`, is required, optional and
    /// nullable as the schema says (derived from the schema, not from wording).
    fn check_fields(schema: &Value, object: &str, skip: &[&str], what: &str) {
        let properties = schema["properties"].as_object().expect("properties");
        let required: Vec<&str> = schema["required"]
            .as_array()
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let written = fields(object);
        let expected: Vec<&String> = properties
            .keys()
            .filter(|name| !skip.contains(&name.as_str()))
            .collect();
        let mut names: Vec<&String> = written.iter().map(|(name, ..)| name).collect();
        names.sort();
        assert_eq!(names, expected, "{what}: the sketch names every field");
        for (name, optional, kind) in &written {
            let is_required = required.contains(&name.as_str());
            assert_eq!(
                *optional, !is_required,
                "{what}.{name}: required or optional"
            );
            // Independent of the renderer: the schema lists a null alternative.
            let nullable = properties[name]["anyOf"]
                .as_array()
                .is_some_and(|members| members.contains(&json!({ "type": "null" })));
            assert_eq!(
                kind.ends_with(" | null"),
                nullable && is_required,
                "{what}.{name}: null is a value only on a required field that takes it"
            );
        }
    }

    #[test]
    fn every_op_and_field_is_written_as_the_schema_says() {
        let (definitions, mutation) = full();
        let text = mutation_sketch(&definitions, "Mutation", &mutation, every())
            .expect("a sketch")
            .text;
        for variant in array(&mutation, "oneOf").expect("variants") {
            let op = variant["properties"]["op"]["const"].as_str().expect("op");
            let line = text
                .lines()
                .find(|line| line.starts_with(&format!("{op} {{")))
                .unwrap_or_else(|| panic!("{op} has a line"));
            let signature = line.split("  // ").next().expect("a signature");
            let signature = signature.strip_prefix(op).expect("op").trim_start();
            check_fields(variant, signature, &["op"], op);
        }
        // Every shared object type the sketch writes holds to the same rule.
        for (name, definition) in &definitions {
            let Some(line) = text
                .lines()
                .find(|l| l.starts_with(&format!("type {name} = {{")))
            else {
                continue;
            };
            if definition.get("properties").is_none() {
                continue;
            }
            let body = line.split(" = ").nth(1).expect("a body");
            // The trailing comment is the kind rules; field notes stay for `fields` to strip.
            let body = body
                .rsplit_once(" /* fields beyond")
                .map_or(body, |(fields, _)| fields);
            let group: Vec<&str> =
                one_of_group(definition.as_object().expect("an object")).expect("a group");
            let object = body.split(", one of ").next().expect("fields");
            let object = if object.ends_with('}') {
                object.to_owned()
            } else {
                format!("{object}}}")
            };
            check_fields(definition, &object, &group, name);
        }
    }

    #[test]
    fn null_is_a_value_only_where_a_required_field_takes_it() {
        let variant = json!({
            "oneOf": [{
                "type": "object",
                "additionalProperties": false,
                "required": ["op", "must", "clearable"],
                "properties": {
                    "op": { "const": "probe" },
                    "must": { "type": "string" },
                    "clearable": { "anyOf": [{ "type": "integer" }, { "type": "null" }] },
                    "left_out": { "type": "boolean" },
                    "left_out_or_null": { "anyOf": [{ "type": "boolean" }, { "type": "null" }] }
                }
            }]
        });
        let text = mutation_sketch(&Map::new(), "Mutation", &variant, probes())
            .expect("a sketch")
            .text;
        assert_eq!(
            text,
            "// Probes\nprobe {clearable: int | null, left_out?: bool, left_out_or_null?: bool, \
             must: string}"
        );
    }

    #[test]
    fn a_construct_cairn_does_not_use_stops_the_sketch() {
        let probe = |field: Value| {
            json!({ "oneOf": [{
                "type": "object", "additionalProperties": false, "required": ["op", "x"],
                "properties": { "op": { "const": "probe" }, "x": field }
            }] })
        };
        let unsupported = [
            json!({ "not": { "type": "string" } }),
            json!({ "type": "string", "format": "uuid" }),
            json!({ "type": "object", "properties": { "a": { "type": "string" } } }),
            json!({ "type": "object", "properties": { "a": { "type": "string" } },
                    "additionalProperties": true }),
            json!({ "type": "object", "additionalProperties": true }),
            json!({ "type": "array", "prefixItems": [{ "type": "string" }] }),
            json!({ "$ref": "#/$defs/Missing" }),
            json!({ "$ref": "https://example.com/schema" }),
            json!({ "anyOf": [{ "type": "null" }] }),
            json!({ "type": ["string", "integer"] }),
            json!({ "anyOf": [{ "type": "string" }, { "type": "null" }], "not": { "const": "x" } }),
            json!({ "$ref": "#/$defs/Id", "not": { "const": "x" } }),
            json!({ "$ref": "#/$defs/Other" }),
            json!({ "allOf": [{ "if": { "properties": { "kind": { "const": "x" } } }, "then": {},
                                "else": {} }],
                    "type": "object", "additionalProperties": false,
                    "properties": { "a": { "type": "string" } } }),
            json!({ "allOf": [{ "if": { "properties": { "mood": { "const": "x" } } }, "then": {} }],
                    "type": "object", "additionalProperties": false,
                    "properties": { "a": { "type": "string" } } }),
        ];
        let definitions = json!({
            "Id": { "type": "string", "pattern": "^n_[a-z]+$", "not": { "const": "n_x" } },
            "Other": { "type": "string", "not": { "const": "x" } },
        });
        let Value::Object(definitions) = definitions else {
            unreachable!("an object")
        };
        for field in unsupported {
            let schema = probe(field.clone());
            let outcome = mutation_sketch(&definitions, "Mutation", &schema, probes());
            assert!(outcome.is_err(), "{field} was rendered");
        }
        // The variant itself is held to the same rule.
        let variant = json!({ "oneOf": [{
            "type": "object", "additionalProperties": false, "not": { "required": ["x"] },
            "required": ["op"], "properties": { "op": { "const": "probe" } }
        }] });
        assert!(mutation_sketch(&definitions, "Mutation", &variant, probes()).is_err());
    }

    /// `apply_patch` lists the ops it takes, each with its line in the sketch under its
    /// group's header, in the schema's order; the types only the ops it leaves out reach are
    /// not written.
    #[test]
    fn apply_patch_lists_the_ops_it_takes_with_the_sketch() {
        let (_, mutation) = full();
        let ops: Vec<String> = array(&mutation, "oneOf")
            .expect("variants")
            .iter()
            .map(|variant| {
                variant["properties"]["op"]["const"]
                    .as_str()
                    .expect("op")
                    .to_owned()
            })
            .collect();
        let tools = ToolSet::definitions();
        let apply = tools
            .iter()
            .find(|tool| tool.name == "apply_patch")
            .expect("apply_patch");
        let listed = &apply.input_schema["$defs"]["Mutation"];
        let named: Vec<String> = listed["properties"]["op"]["enum"]
            .as_array()
            .expect("an op enum")
            .iter()
            .map(|op| op.as_str().expect("an op name").to_owned())
            .collect();
        let sketch = listed["description"].as_str().expect("the sketch");
        let lines: Vec<String> = sketch
            .lines()
            .filter_map(|line| line.split_once(" {").map(|(op, _)| op.to_owned()))
            .filter(|op| ops.contains(op))
            .collect();
        assert_eq!(named, lines, "each listed op has its line, in order");
        let in_order: Vec<&String> = ops.iter().filter(|op| named.contains(op)).collect();
        assert_eq!(named.iter().collect::<Vec<_>>(), in_order);
        assert!(!named.iter().any(|op| op == "apply_proposal"), "{named:?}");
        assert!(
            !sketch.contains("type ProposalDraft ="),
            "only the proposal ops reach a proposal's content"
        );
        // Each group's header comes once, before its first listed op.
        for (first, header) in GROUPS {
            let header = format!("// {header}");
            let written = sketch.lines().filter(|line| **line == header).count();
            assert!(written <= 1, "{header}");
            if named.iter().any(|op| op == first) {
                assert_eq!(written, 1, "{header}");
            }
        }
        assert!(
            listed.get("oneOf").is_none(),
            "the schema no longer spells the variants out"
        );
    }

    type Banned<'a> = BTreeMap<&'a str, Vec<&'a str>>;
    type Answers<'a> = BTreeMap<&'a str, (Vec<&'a str>, Vec<&'a str>)>;

    /// The node schema's rules read straight from it: per kind what is banned and needed,
    /// per answer type what is needed and banned.
    fn read_rules(node: &Value) -> (Banned<'_>, Banned<'_>, Answers<'_>) {
        let mut banned: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut needs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut answers: BTreeMap<&str, (Vec<&str>, Vec<&str>)> = BTreeMap::new();
        for rule in node["allOf"].as_array().expect("rules") {
            let condition = rule["if"]["properties"].as_object().expect("a condition");
            let listed = |key: &str| -> Vec<&str> {
                rule["then"][key]
                    .as_array()
                    .map(|v| v.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default()
            };
            let off: Vec<&str> = rule["then"]["properties"]
                .as_object()
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if let Some(kind) = condition.get("kind") {
                let kind = kind["const"].as_str().expect("a kind");
                banned.entry(kind).or_default().extend(off);
                needs.entry(kind).or_default().extend(listed("required"));
            } else {
                let answer = condition["answer_type"]["const"]
                    .as_str()
                    .expect("an answer type");
                let entry = answers.entry(answer).or_default();
                entry.0.extend(listed("required"));
                entry.1.extend(off);
            }
        }
        (banned, needs, answers)
    }

    /// What a schema's `if kind` and `if answer_type` rules say, read straight from it and
    /// compared with what the sketch writes of the node: the fields each kind takes and
    /// needs, and each answer type's needs and bans.
    #[test]
    fn the_nodes_rules_are_written_as_the_schema_has_them() {
        let (definitions, mutation) = full();
        let text = mutation_sketch(&definitions, "Mutation", &mutation, every())
            .expect("a sketch")
            .text;
        let node = &definitions["Node"];
        let comment = text
            .lines()
            .find(|line| line.starts_with("type Node = "))
            .and_then(|line| line.rsplit("/* ").next())
            .map(|rules| rules.trim_end_matches(" */"))
            .expect("the node's rules");
        let names: Vec<&String> = node["properties"]
            .as_object()
            .expect("fields")
            .keys()
            .collect();
        let (banned, needs, answers) = read_rules(node);
        let (kinds, by_answer) = comment
            .split_once(". By answer_type: ")
            .expect("both rules");
        let kinds = kinds.trim_start_matches("fields beyond the common ones, by kind: ");
        for (kind, off) in &banned {
            let clause = kinds
                .split("; ")
                .find(|clause| clause.starts_with(&format!("{kind}: ")))
                .unwrap_or_else(|| panic!("{kind} has a clause"));
            let (taken, wanted) = clause[kind.len() + 2..]
                .split_once(" (needs ")
                .map_or((&clause[kind.len() + 2..], ""), |(taken, wanted)| {
                    (taken, wanted.trim_end_matches(')'))
                });
            let mut taken: Vec<&str> = taken.split(", ").filter(|name| !name.is_empty()).collect();
            taken.sort_unstable();
            // Beyond the common ones: allowed for this kind, banned for another.
            let mut expected: Vec<&str> = names
                .iter()
                .map(|name| name.as_str())
                .filter(|name| !off.contains(name))
                .filter(|name| banned.values().any(|other| other.contains(name)))
                .collect();
            expected.sort_unstable();
            assert_eq!(taken, expected, "{kind}: the fields it takes");
            let wanted: Vec<&str> = wanted.split(", ").filter(|name| !name.is_empty()).collect();
            assert_eq!(wanted, needs[kind], "{kind}: the fields it needs");
        }
        for (answer, (wanted, off)) in &answers {
            let clause = by_answer
                .split("; ")
                .find(|clause| clause.starts_with(&format!("{answer}: ")))
                .unwrap_or_else(|| panic!("{answer} has a clause"));
            let body = &clause[answer.len() + 2..];
            let needed: Vec<&str> = body
                .split(", ")
                .filter_map(|part| part.strip_prefix("needs "))
                .collect();
            assert_eq!(needed, *wanted, "{answer}: needs");
            let mut forbidden: Vec<&str> = body
                .split("no ")
                .nth(1)
                .map(|b| b.split(", ").collect())
                .unwrap_or_default();
            forbidden.sort_unstable();
            let mut off = off.clone();
            off.sort_unstable();
            assert_eq!(forbidden, off, "{answer}: bans");
        }
    }

    /// A group of which exactly one is required, and a type cited without its fields, are
    /// written from the schema: the group's names as the schema lists them, and the
    /// description the cited type carries.
    #[test]
    fn groups_and_opaque_types_keep_what_the_schema_says() {
        let (definitions, mutation) = full();
        let text = mutation_sketch(&definitions, "Mutation", &mutation, every())
            .expect("a sketch")
            .text;
        let mut groups = 0;
        for (name, definition) in &definitions {
            let Some(line) = text
                .lines()
                .find(|l| l.starts_with(&format!("type {name} = ")))
            else {
                continue;
            };
            if definition.get("properties").is_none() {
                continue;
            }
            let group = one_of_group(definition.as_object().expect("an object")).expect("a group");
            if !group.is_empty() {
                groups += 1;
                let written = line.split(", one of ").nth(1).expect("a one-of group");
                for field in group {
                    assert!(written.contains(&format!("{field}: ")), "{name}.{field}");
                }
                assert!(
                    !line.contains(", one of ") || line.matches("one of").count() == 1,
                    "{name}"
                );
            }
        }
        assert!(
            groups >= 3,
            "the vocabulary has several groups of which one is required"
        );
        let review = &definitions["ReviewItem"];
        let line = text
            .lines()
            .find(|l| l.starts_with("type ReviewItem = "))
            .expect("review item");
        let described = review["description"]
            .as_str()
            .expect("described")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(line.contains(&described), "{line}");
    }
}
