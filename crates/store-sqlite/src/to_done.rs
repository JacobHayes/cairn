//! The code step of migration 0014: the JSON a finished milestone's old words live in. SQL
//! cannot tell a field name or a state from text a person wrote, so each stored value is
//! walked and only the positions that hold one of those names are changed: a transition's name
//! in a mutation, a milestone's flag in a node, in a single-field edit and in a field name, and
//! a node's stored state. Text a person wrote (a title, a note, a text answer, a choice id) is
//! never touched, and neither is the one map keyed by what a person chose, a conflict
//! resolution's `map_choices.map`. A value already in the new words is left as it is, so a
//! rerun changes nothing.
//!
//! The rewritten columns: `proposals.mutations` and `.items`, `events.subject` and `.delta`,
//! and `patch_receipts.proposals` and `.deployment_touched`. The assistant's
//! `conversation_messages` (a transcript of what was said) and the receipts' content hashes
//! stay as they are: a patch resent from before the upgrade carries a transition that no
//! longer parses, so it is refused as invalid and never mistaken for another.

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, rows, text};

/// Renames the key `from` of `object` to `to`, keeping its value.
fn rename_key(object: &mut serde_json::Map<String, Value>, from: &str, to: &str) -> bool {
    match object.remove(from) {
        Some(value) => {
            object.insert(to.to_owned(), value);
            true
        }
        None => false,
    }
}

/// Whether `object` holds `key` with the string `expected`.
fn holds(object: &serde_json::Map<String, Value>, key: &str, expected: &str) -> bool {
    object.get(key).and_then(Value::as_str) == Some(expected)
}

/// Rewrites, anywhere in `value`, each position that holds an old name; whether it changed
/// anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            // A mutation: `{op: transition, transition: reach}` is now a completion.
            if holds(object, "op", "transition") && holds(object, "transition", "reach") {
                object.insert(
                    "transition".to_owned(),
                    Value::String("complete".to_owned()),
                );
                changed = true;
            }
            // A milestone as authored or stored: `{kind: milestone, auto_reach: ...}`.
            if holds(object, "kind", "milestone") {
                changed |= rename_key(object, "auto_reach", "auto_done");
            }
            // A single-field edit: `{auto_reach: true}`.
            if object.len() == 1 && object.get("auto_reach").is_some_and(Value::is_boolean) {
                changed |= rename_key(object, "auto_reach", "auto_done");
            }
            // A field's name: a local edit, a violation's location, a record key.
            if holds(object, "field", "auto_reach") {
                object.insert("field".to_owned(), Value::String("auto_done".to_owned()));
                changed = true;
            }
            // A node's stored state.
            if object.contains_key("provenance") && holds(object, "state", "reached") {
                object.insert("state".to_owned(), Value::String("done".to_owned()));
                changed = true;
            }
            for (key, nested) in object.iter_mut() {
                if key != "map" {
                    changed |= rewrite(nested);
                }
            }
            changed
        }
        Value::Array(items) => {
            let mut changed = false;
            for item in items {
                changed |= rewrite(item);
            }
            changed
        }
        _ => false,
    }
}

/// Rewrites the JSON `columns` of every row of `table`, found by `key`.
fn column(
    connection: &Connection,
    table: &str,
    key: &str,
    columns: &[&str],
) -> Result<(), SqlError> {
    for name in columns {
        let select = format!("SELECT {key}, {name} FROM {table}");
        let update = format!("UPDATE {table} SET {name} = ?1 WHERE {key} = ?2");
        for row in rows(connection, &select, Vec::new())? {
            let stored = row.text(1).map_err(failed)?;
            let mut value: Value = serde_json::from_str(&stored)
                .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
            if rewrite(&mut value) {
                let at = if table == "events" {
                    crate::sql::int(row.int(0).map_err(failed)?)
                } else {
                    text(&row.text(0).map_err(failed)?)
                };
                execute(connection, &update, vec![text(&value.to_string()), at])?;
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "passed to `map_err`, which hands over the error"
)]
fn failed(error: cairn_store::StoreError) -> SqlError {
    SqlError::Other(format!("{error:?}"))
}

/// Runs the step over every stored JSON value that can hold an old name.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    column(connection, "proposals", "id", &["mutations", "items"])?;
    column(connection, "events", "seq", &["subject", "delta"])?;
    column(
        connection,
        "patch_receipts",
        "patch_id",
        &["proposals", "deployment_touched"],
    )
}
