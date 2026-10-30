//! The code step of migration 27: a stage is an ordinary group. The four fields a group had
//! for its bounds are gone (the statements after this step rebuild the nodes table without
//! their columns), and what they said is said by the ordinary fields:
//!
//! - `opens_at: M` becomes a requirement on `M`, unless `gates` was false, which a group no
//!   longer can be: the opening is dropped;
//! - `closes_at: M` becomes `due_by: {before: M}`, unless `closes` was false (dropped). A
//!   group that already has a rule takes `M` as one more source when its offset is zero, needs
//!   nothing when it is already `before` a source `M`, and otherwise cannot say both: the
//!   migration refuses, listing every such group, and writes nothing.
//!
//! Both are the same constraints the fields made, so every derived value is unchanged.
//!
//! The same mapping is made over the stored history, so it still reads back: a node an event
//! writes, and a stage field an event writes on its own (kept per graph in order, since
//! moving an opening means removing the old requirement; a closing edit on a group with a
//! rule of its own rewrites that rule with the closing merged in). A graph copied whole
//! (a draft published, a draft opened from a version) carries what is known of its groups
//! to the copy. A proposal, in the table and in the events that wrote it, has its added and
//! replaced nodes converted the same way and loses every other mutation and review item that
//! holds a removed field or anything a stage bound said (a violation of that rule, a chain
//! through a stage's bound); an open one that lost a mutation is discarded, with a reason,
//! since what is left of it is not what its author meant (a lost review item does not count:
//! it is derived from the mutations). A rerun changes nothing.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde_json::{Map, Value, json};

use cairn_schema::limits::EDGE_COUNT_PER_NODE_MAX;

use crate::sql::{SqlError, execute, int, rows, text};
use crate::stored_json::failed;

const FIELDS: [&str; 4] = ["opens_at", "closes_at", "gates", "closes"];

/// What a discarded open proposal says.
const DISCARDED: &str = "Discarded: a stage's opening and closing are now an ordinary \
requirement and due-by, so the stage edits this held no longer exist. Draft it again.";

/// A group's former bounds.
#[derive(Clone, Debug, Default)]
struct Bounds {
    opens_at: Option<String>,
    closes_at: Option<String>,
    gates: bool,
    closes: bool,
}

impl Bounds {
    fn new() -> Self {
        Self {
            gates: true,
            closes: true,
            ..Self::default()
        }
    }

    /// The requirement the opening stands for.
    fn requirement(&self) -> Option<&str> {
        self.opens_at.as_deref().filter(|_| self.gates)
    }

    /// The deadline the closing stands for.
    fn deadline(&self) -> Option<&str> {
        self.closes_at.as_deref().filter(|_| self.closes)
    }
}

/// A date rule with the deadline `milestone` also bounding it: `Ok(None)` when it already
/// does, `Err` when the rule says something else that cannot hold both.
fn with_deadline(rule: Option<&Value>, milestone: &str) -> Result<Option<Value>, String> {
    let Some(rule) = rule else {
        return Ok(Some(json!({"before": milestone})));
    };
    let direction = ["before", "after"]
        .into_iter()
        .find(|direction| rule.get(direction).is_some());
    let Some(direction) = direction else {
        return Err("its due-by names no sources".to_owned());
    };
    let mut sources: Vec<String> = match &rule[direction] {
        Value::String(source) => vec![source.clone()],
        Value::Array(sources) => sources
            .iter()
            .filter_map(|source| source.as_str().map(str::to_owned))
            .collect(),
        _ => return Err("its due-by names no sources".to_owned()),
    };
    let offset = rule.get("offset").and_then(Value::as_u64).unwrap_or(0);
    let names = sources.iter().any(|source| source == milestone);
    if names && (direction == "before" || offset == 0) {
        return Ok(None);
    }
    if offset != 0 {
        return Err(format!(
            "its own due-by is {direction} its sources by {offset} days, which cannot also \
             bound it by the milestone it closed at"
        ));
    }
    if sources.len() >= EDGE_COUNT_PER_NODE_MAX as usize {
        return Err("its due-by already has the most sources a rule can".to_owned());
    }
    sources.push(milestone.to_owned());
    sources.sort();
    let mut merged = rule.clone();
    merged[direction] = json!(sources);
    Ok(Some(merged))
}

/// The nodes table: each group's requirement and due-by, or every group that cannot be
/// converted.
fn nodes(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT graph_id, key, opens_at, closes_at, gates, closes, due_by FROM nodes \
                  WHERE kind = 'group' AND (opens_at IS NOT NULL OR closes_at IS NOT NULL) \
                  ORDER BY graph_id, key";
    let mut refused = Vec::new();
    for row in rows(connection, select, Vec::new())? {
        let (graph, key) = (row.text(0).map_err(failed)?, row.text(1).map_err(failed)?);
        let bounds = Bounds {
            opens_at: row.opt_text(2).map_err(failed)?,
            closes_at: row.opt_text(3).map_err(failed)?,
            gates: row.opt_flag(4).map_err(failed)?.unwrap_or(true),
            closes: row.opt_flag(5).map_err(failed)?.unwrap_or(true),
        };
        if let Some(milestone) = bounds.requirement()
            && let Err(reason) = require(connection, &graph, &key, milestone)?
        {
            refused.push(format!("{graph} {key}: {reason}"));
        }
        if let Some(milestone) = bounds.deadline() {
            let rule: Option<Value> = row
                .opt_text(6)
                .map_err(failed)?
                .map(|stored| serde_json::from_str(&stored))
                .transpose()
                .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
            match with_deadline(rule.as_ref(), milestone) {
                Ok(Some(merged)) => {
                    execute(
                        connection,
                        "UPDATE nodes SET due_by = ?1 WHERE graph_id = ?2 AND key = ?3",
                        vec![text(&merged.to_string()), text(&graph), text(&key)],
                    )?;
                }
                Ok(None) => {}
                Err(reason) => refused.push(format!("{graph} {key}: {reason}")),
            }
        }
    }
    if refused.is_empty() {
        Ok(())
    } else {
        Err(SqlError::Other(format!(
            "a stage cannot become an ordinary group where {}",
            refused.join("; ")
        )))
    }
}

/// Adds the requirement `node` -> `milestone`, unless it is there; `Err` when either end
/// would pass the explicit edges a node can have.
fn require(
    connection: &Connection,
    graph: &str,
    node: &str,
    milestone: &str,
) -> Result<Result<(), String>, SqlError> {
    let present = rows(
        connection,
        "SELECT 1 FROM edges WHERE graph_id = ?1 AND node = ?2 AND requires = ?3",
        vec![text(graph), text(node), text(milestone)],
    )?;
    if !present.is_empty() {
        return Ok(Ok(()));
    }
    for end in [node, milestone] {
        let held = rows(
            connection,
            "SELECT COUNT(*) FROM edges WHERE graph_id = ?1 AND (node = ?2 OR requires = ?2)",
            vec![text(graph), text(end)],
        )?;
        let held = held
            .first()
            .map_or(Ok(0), |row| row.int(0))
            .map_err(failed)?;
        if held >= i64::from(EDGE_COUNT_PER_NODE_MAX) {
            return Ok(Err(format!(
                "{end} already has the most requirements a node can, and the opening \
                 would be one more"
            )));
        }
    }
    execute(
        connection,
        "INSERT INTO edges (graph_id, node, requires) VALUES (?1, ?2, ?3)",
        vec![text(graph), text(node), text(milestone)],
    )?;
    Ok(Ok(()))
}

/// What moving a node's bounds did.
#[derive(PartialEq)]
enum Moved {
    /// The node had none.
    Nothing,
    /// Its bounds are now a requirement and a due-by.
    All,
    /// Its own rule cannot take the closing, which is left out.
    WithoutClosing,
}

/// A whole group node as stored in a write or a mutation: its bounds moved into `requires`
/// and `due_by`.
fn rewrite_node(node: &mut Map<String, Value>) -> Moved {
    if !FIELDS.iter().any(|field| node.contains_key(*field)) {
        return Moved::Nothing;
    }
    let bounds = Bounds {
        opens_at: node
            .remove("opens_at")
            .and_then(|value| value.as_str().map(str::to_owned)),
        closes_at: node
            .remove("closes_at")
            .and_then(|value| value.as_str().map(str::to_owned)),
        gates: node
            .remove("gates")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        closes: node
            .remove("closes")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
    };
    if let Some(milestone) = bounds.requirement() {
        let requires = node.entry("requires").or_insert_with(|| json!([]));
        if let Value::Array(requires) = requires
            && !requires.iter().any(|found| found == milestone)
        {
            requires.push(json!(milestone));
            requires.sort_by_key(Value::to_string);
        }
    }
    if let Some(milestone) = bounds.deadline() {
        match with_deadline(node.get("due_by"), milestone) {
            Ok(Some(rule)) => {
                node.insert("due_by".to_owned(), rule);
            }
            Ok(None) => {}
            Err(_) => return Moved::WithoutClosing,
        }
    }
    Moved::All
}

/// The stage state of the groups one graph's events have written so far, with the rule each
/// has of its own, so a field written on its own can be read as the requirement or due-by it
/// moves.
#[derive(Clone, Default)]
struct Groups(BTreeMap<String, (Bounds, Option<Value>)>);

/// One write of an event rewritten: the writes it becomes, or none when it is left as it is.
fn rewrite_write(groups: &mut BTreeMap<String, Groups>, write: &Value) -> Option<Vec<Value>> {
    if let Some(copy) = write.get("copy_graph") {
        let (from, to) = (copy.get("from")?.to_string(), copy.get("to")?.to_string());
        match groups.get(&from).cloned() {
            Some(known) => groups.insert(to, known),
            None => groups.remove(&to),
        };
        return None;
    }
    if let Some(graph) = write.pointer("/remove/graph") {
        groups.remove(&graph.to_string());
        return None;
    }
    let record = write.pointer("/put/graph/record")?;
    let graph = write.pointer("/put/graph/graph")?;
    let known = groups.entry(graph.to_string()).or_default();
    if let Some(Value::Object(node)) = record.get("node") {
        return node_write(known, write, node);
    }
    if let Some(Value::Object(field)) = record.get("node_field") {
        return field_write(known, graph, field);
    }
    None
}

/// A put of a whole node: remembered, and written with its bounds moved.
fn node_write(known: &mut Groups, write: &Value, node: &Map<String, Value>) -> Option<Vec<Value>> {
    let key = node.get("key")?.as_str()?.to_owned();
    let text = |field: &str| node.get(field).and_then(Value::as_str).map(str::to_owned);
    let flag = |field: &str| node.get(field).and_then(Value::as_bool).unwrap_or(true);
    let bounds = Bounds {
        opens_at: text("opens_at"),
        closes_at: text("closes_at"),
        gates: flag("gates"),
        closes: flag("closes"),
    };
    known.0.insert(key, (bounds, node.get("due_by").cloned()));
    let mut node = node.clone();
    if rewrite_node(&mut node) == Moved::Nothing {
        return None;
    }
    let mut changed = write.clone();
    *changed.pointer_mut("/put/graph/record/node")? = Value::Object(node);
    Some(vec![changed])
}

/// A put of one field of a node: a stage field becomes the requirement or due-by it moves,
/// and a due-by is remembered.
fn field_write(
    known: &mut Groups,
    graph: &Value,
    field: &Map<String, Value>,
) -> Option<Vec<Value>> {
    let key = field.get("node")?.as_str()?.to_owned();
    let value = field.get("value")?.as_object()?;
    let (name, written) = value.iter().next()?;
    let (bounds, own_rule) = known
        .0
        .entry(key.clone())
        .or_insert_with(|| (Bounds::new(), None));
    if name == "due_by" {
        *own_rule = (!written.is_null()).then(|| written.clone());
        // A closing still bounds the group: the rule written takes it as one more source.
        let merged = with_deadline(own_rule.as_ref(), bounds.deadline()?).ok()??;
        return Some(vec![due_by_write(graph, &key, &merged)]);
    }
    if !FIELDS.contains(&name.as_str()) {
        return None;
    }
    let (was_requirement, was_deadline) = (
        bounds.requirement().map(str::to_owned),
        bounds.deadline().map(str::to_owned),
    );
    match name.as_str() {
        "opens_at" => bounds.opens_at = written.as_str().map(str::to_owned),
        "closes_at" => bounds.closes_at = written.as_str().map(str::to_owned),
        "gates" => bounds.gates = written.as_bool().unwrap_or(true),
        _ => bounds.closes = written.as_bool().unwrap_or(true),
    }
    let (now_requirement, now_deadline) = (
        bounds.requirement().map(str::to_owned),
        bounds.deadline().map(str::to_owned),
    );
    let mut writes = Vec::new();
    if was_requirement != now_requirement {
        let edge = |milestone: &str| json!({"node": key, "requires": milestone});
        if let Some(old) = &was_requirement {
            writes.push(
                json!({"remove": {"in_graph": {"graph": graph, "key": {"edge": edge(old)}}}}),
            );
        }
        if let Some(new) = &now_requirement {
            writes.push(json!({"put": {"graph": {"graph": graph, "record": {"edge": edge(new)}}}}));
        }
    }
    if was_deadline != now_deadline {
        let own = own_rule.clone();
        let rule = now_deadline
            .and_then(|milestone| with_deadline(own.as_ref(), &milestone).ok().flatten())
            .or(own)
            .unwrap_or(Value::Null);
        writes.push(due_by_write(graph, &key, &rule));
    }
    Some(writes)
}

/// A put of a group's due-by rule on its own.
fn due_by_write(graph: &Value, key: &str, rule: &Value) -> Value {
    json!({"put": {"graph": {"graph": graph, "record": {
        "node_field": {"node": key, "value": {"due_by": rule}}}}}})
}

/// An event's delta, rewritten write by write; whether it changed.
fn rewrite_delta(groups: &mut BTreeMap<String, Groups>, delta: &mut Value) -> bool {
    let Value::Array(writes) = delta else {
        return false;
    };
    let mut changed = false;
    let mut converted = Vec::with_capacity(writes.len());
    for write in writes.iter() {
        match rewrite_write(groups, write) {
            Some(replacements) => {
                changed = true;
                converted.extend(replacements);
            }
            None => converted.push(write.clone()),
        }
    }
    for write in &mut converted {
        if let Some(draft) = write.pointer_mut("/put/proposal/draft") {
            let done = strip(draft);
            changed |= done.converted || done.dropped || done.items_dropped;
        }
    }
    *writes = converted;
    changed
}

/// Whether anything in `value` is of the removed fields: one of their names as a key, or as
/// the value of a `field` or `kind` entry (a field being cleared or named); the violation
/// code of a stage bound that named no milestone; a constraint source that is a stage's
/// bound. Text a person wrote is never read.
fn mentions(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            FIELDS.contains(&key.as_str())
                || value.as_str().is_some_and(|text| match key.as_str() {
                    "field" | "kind" => FIELDS.contains(&text),
                    "code" => text == "stage_bound_not_milestone",
                    "source" => text.starts_with("stage_close:") || text.contains(":opening:"),
                    _ => false,
                })
                || mentions(value)
        }),
        Value::Array(items) => items.iter().any(mentions),
        _ => false,
    }
}

/// What stripping a draft did.
#[derive(Default)]
struct Stripped {
    /// An added or replaced node had its bounds moved.
    converted: bool,
    /// A mutation was removed: what the proposal's author meant is no longer all there.
    dropped: bool,
    /// A review item was removed: derived from the draft, so the draft means what it did.
    items_dropped: bool,
}

/// An added or replaced node in a mutation, its bounds moved into `requires` and `due_by`;
/// whether it was, or `Err` when its own rule cannot take the closing, so the mutation
/// cannot say what its author meant.
fn convert(mutation: &mut Value) -> Result<bool, ()> {
    let is_node = matches!(
        mutation.get("op").and_then(Value::as_str),
        Some("add_node" | "replace_node")
    );
    let Some(node) = mutation
        .get_mut("node")
        .and_then(Value::as_object_mut)
        .filter(|_| is_node)
    else {
        return Ok(false);
    };
    match rewrite_node(node) {
        Moved::Nothing => Ok(false),
        Moved::All => Ok(true),
        Moved::WithoutClosing => Err(()),
    }
}

/// Converts, in a proposal's draft, the nodes its mutations add or replace, then removes the
/// mutations and review items that still hold the removed fields.
fn strip(draft: &mut Value) -> Stripped {
    let mut done = Stripped::default();
    for list in ["mutations", "items"] {
        if let Some(Value::Array(entries)) = draft.get_mut(list) {
            let before = entries.len();
            if list == "mutations" {
                entries.retain_mut(|entry| match convert(entry) {
                    Ok(moved) => {
                        done.converted |= moved;
                        true
                    }
                    Err(()) => false,
                });
            }
            entries.retain(|entry| !mentions(entry));
            let lost = entries.len() != before;
            if list == "mutations" {
                done.dropped |= lost;
            } else {
                done.items_dropped |= lost;
            }
        }
    }
    done
}

/// The stored proposals: stripped, and an open one that lost anything discarded.
fn proposals(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT id, status, mutations, items FROM proposals";
    for row in rows(connection, select, Vec::new())? {
        let parse = |index| -> Result<Value, SqlError> {
            serde_json::from_str(&row.text(index).map_err(failed)?)
                .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))
        };
        let mut draft = json!({"mutations": parse(2)?, "items": parse(3)?});
        let done = strip(&mut draft);
        if !(done.converted || done.dropped || done.items_dropped) {
            continue;
        }
        let id = text(&row.text(0).map_err(failed)?);
        if done.dropped && row.text(1).map_err(failed)? == "open" {
            execute(
                connection,
                "UPDATE proposals SET status = 'discarded', description = ?1, \
                 mutations = '[]', items = '[]' WHERE id = ?2",
                vec![text(DISCARDED), id],
            )?;
        } else {
            execute(
                connection,
                "UPDATE proposals SET mutations = ?1, items = ?2 WHERE id = ?3",
                vec![
                    text(&draft["mutations"].to_string()),
                    text(&draft["items"].to_string()),
                    id,
                ],
            )?;
        }
    }
    Ok(())
}

/// The stored events, in order, graph by graph.
fn events(connection: &Connection) -> Result<(), SqlError> {
    let mut groups: BTreeMap<String, Groups> = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT seq, delta FROM events ORDER BY seq",
        Vec::new(),
    )? {
        let mut delta: Value = serde_json::from_str(&row.text(1).map_err(failed)?)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        if rewrite_delta(&mut groups, &mut delta) {
            execute(
                connection,
                "UPDATE events SET delta = ?1 WHERE seq = ?2",
                vec![text(&delta.to_string()), int(row.int(0).map_err(failed)?)],
            )?;
        }
    }
    Ok(())
}

/// Runs the step over the nodes table, the proposals, and the events (the proposals' own
/// records in events are stripped the same way).
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    nodes(connection)?;
    proposals(connection)?;
    events(connection)
}
