//! The code step of migration 21: order is no longer a guard. Starting, finishing and
//! answering never wait for what a node waits on or for its own date, so the `deps_done` and
//! `date_arrived` guards are gone, and with them the `open_dependency` and `too_early`
//! failures a bypass recorded. Every stored bypass keeps only its evidence guards and their
//! failures.
//!
//! The overrides table's `bypass` column is rewritten: a bypass left with no guard is removed,
//! and a row left with no override is deleted. The JSON of `proposals.mutations` and `.items`
//! and `events.subject` and `.delta` follows, so replaying the log still gives the stored
//! state: every `guards` array loses the two names and every `failures` array the two
//! failures; a stored `bypass` left with no guard is removed from the overrides record that
//! holds it, and a write of an overrides record left empty becomes the removal of that
//! record; a `guard_bypass` mutation left with no guard is dropped from its list, since alone
//! it covers nothing and would refuse the proposal that holds it. The one map keyed by what a
//! person chose, a conflict resolution's `map_choices.map`, is never entered. A rerun changes
//! nothing. Receipts' content hashes stay: a patch resent from before the upgrade
//! names a guard that no longer parses, so it is refused as invalid.

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, rows, text};
use crate::stored_json::{column, failed};

const GUARDS: [&str; 2] = ["deps_done", "date_arrived"];
const FAILURES: [&str; 2] = ["open_dependency", "too_early"];

/// Drops the order guards and failures from one bypass-shaped object; whether it changed.
fn strip(object: &mut serde_json::Map<String, Value>) -> bool {
    let mut changed = false;
    if let Some(Value::Array(guards)) = object.get_mut("guards") {
        let before = guards.len();
        guards.retain(|guard| guard.as_str().is_none_or(|name| !GUARDS.contains(&name)));
        changed |= guards.len() != before;
    }
    if let Some(Value::Array(failures)) = object.get_mut("failures") {
        let before = failures.len();
        failures.retain(|failure| {
            failure
                .as_object()
                .is_none_or(|tagged| !FAILURES.iter().any(|name| tagged.contains_key(*name)))
        });
        changed |= failures.len() != before;
        if failures.is_empty() {
            object.remove("failures");
        }
    }
    changed
}

/// Whether a bypass names no guard.
fn empty(bypass: &Value) -> bool {
    bypass
        .get("guards")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

/// A `guard_bypass` mutation that names no guard.
fn bypasses_nothing(mutation: &Value) -> bool {
    mutation.get("op").and_then(Value::as_str) == Some("apply_override")
        && mutation
            .get("override")
            .and_then(|applied| applied.get("guard_bypass"))
            .is_some_and(empty)
}

/// The removal of the overrides record that a write of `object` puts, when that record is
/// empty: `{put: {graph: {graph, record: {overrides: {node, overrides: {}}}}}}`.
fn emptied(object: &serde_json::Map<String, Value>) -> Option<Value> {
    let graph = object.get("put")?.get("graph")?;
    let record = graph.get("record")?.get("overrides")?;
    let empty = record.get("overrides")?.as_object()?.is_empty();
    empty.then(|| {
        serde_json::json!({"remove": {"in_graph": {
            "graph": graph.get("graph").cloned().unwrap_or(Value::Null),
            "key": {"overrides": record.get("node").cloned().unwrap_or(Value::Null)},
        }}})
    })
}

/// Rewrites, anywhere in `value`, each order guard and failure; whether it changed anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            for (key, nested) in object.iter_mut() {
                if key != "map" {
                    changed |= rewrite(nested);
                }
            }
            changed |= strip(object);
            if object.get("bypass").is_some_and(empty) {
                object.remove("bypass");
                changed = true;
            }
            if changed && let Some(removal) = emptied(object) {
                *value = removal;
            }
            changed
        }
        Value::Array(items) => {
            let mut changed = false;
            for item in items.iter_mut() {
                changed |= rewrite(item);
            }
            let before = items.len();
            items.retain(|item| !bypasses_nothing(item));
            changed | (items.len() != before)
        }
        _ => false,
    }
}

/// Rewrites each stored bypass of the overrides table, removing one left with no guard and a
/// row left with no override.
fn overrides(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT graph_id, node, bypass FROM overrides WHERE bypass IS NOT NULL";
    for row in rows(connection, select, Vec::new())? {
        let (graph, node) = (row.text(0).map_err(failed)?, row.text(1).map_err(failed)?);
        let stored = row.text(2).map_err(failed)?;
        let mut bypass: Value = serde_json::from_str(&stored)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        let changed = bypass.as_object_mut().is_some_and(strip);
        if !changed {
            continue;
        }
        let at = vec![text(&graph), text(&node)];
        if empty(&bypass) {
            execute(
                connection,
                "UPDATE overrides SET bypass = NULL WHERE graph_id = ?1 AND node = ?2",
                at,
            )?;
        } else {
            let mut values = vec![text(&bypass.to_string())];
            values.extend(at);
            execute(
                connection,
                "UPDATE overrides SET bypass = ?1 WHERE graph_id = ?2 AND node = ?3",
                values,
            )?;
        }
    }
    execute(
        connection,
        "DELETE FROM overrides WHERE force_include IS NULL AND keep IS NULL AND bypass IS NULL",
        Vec::new(),
    )?;
    Ok(())
}

/// Runs the step over the overrides table and every stored JSON value that can hold a bypass.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    overrides(connection)?;
    column(
        connection,
        "proposals",
        "id",
        &["mutations", "items"],
        rewrite,
    )?;
    column(connection, "events", "seq", &["subject", "delta"], rewrite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_bypass_keeps_its_evidence_and_one_of_order_alone_goes() {
        let put = |node: &str, overrides: Value| {
            serde_json::json!({"put": {"graph": {"graph": {"journey": "j_one"},
                "record": {"overrides": {"node": node, "overrides": overrides}}}}})
        };
        let bypass = |guards: Value| {
            serde_json::json!({"op": "apply_override", "node": "n_a",
                "override": {"guard_bypass": {"guards": guards, "reason": "deps_done"}}})
        };
        let mut value = serde_json::json!([
            put(
                "n_a",
                serde_json::json!({"bypass": {
                    "guards": ["deps_done", "has_note"],
                    "failures": [{"open_dependency": "n_b"}, "missing_note", {"too_early": "2026-10-01"}],
                }})
            ),
            put(
                "n_c",
                serde_json::json!({"keep": "Kept.", "bypass": {
                    "guards": ["date_arrived"], "failures": [{"too_early": "2026-10-01"}],
                }})
            ),
            put(
                "n_d",
                serde_json::json!({"bypass": {"guards": ["deps_done"]}})
            ),
            bypass(serde_json::json!(["deps_done", "all_ticked"])),
            bypass(serde_json::json!(["deps_done"])),
        ]);
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!([
                put("n_a", serde_json::json!({"bypass": {
                    "guards": ["has_note"], "failures": ["missing_note"],
                }})),
                put("n_c", serde_json::json!({"keep": "Kept."})),
                {"remove": {"in_graph": {"graph": {"journey": "j_one"},
                    "key": {"overrides": "n_d"}}}},
                bypass(serde_json::json!(["all_ticked"])),
            ])
        );
        let removal: Result<cairn_schema::Write, _> = serde_json::from_value(value[2].clone());
        assert!(removal.is_ok(), "{removal:?}");
        let rewritten = value.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, rewritten);
    }

    #[test]
    fn a_map_between_choices_is_left_alone() {
        let chosen = serde_json::json!({"map_choices": {"map": {"guards": "deps_done"}}});
        let mut value = chosen.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, chosen);
    }
}
