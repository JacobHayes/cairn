//! The code step of migration 25: a journey keeps only the route version it was made from. The
//! local-edit markers, the tombstones and each node's provenance are no longer stored (the
//! statements after this step drop their tables and column), and upgrade and re-link are gone,
//! with the conflict, kept-edit and orphan items a proposal could carry. SQL cannot tell stored
//! JSON from text a person wrote, so each value is walked:
//!
//! - a node state (a `state` that is one of the state names beside a `provenance` that is one
//!   of the provenance names) loses its `provenance`;
//! - in an event's delta, the writes of a local-edit marker or a tombstone go;
//! - a proposal that holds an upgrade, a re-link, a provenance or marker change, or a conflict,
//!   kept-edit or orphan item can no longer be read or applied, so it goes, and so does its
//!   record in an event's delta.
//!
//! The events that only recorded a provenance or a marker are removed, and an upgrade or a
//! re-link is now a journey edit in the log. A rerun changes nothing.

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, rows, text};
use crate::stored_json::{column, failed};

const STATES: [&str; 8] = [
    "todo", "active", "done", "skipped", "open", "decided", "pending", "derived",
];
const PROVENANCES: [&str; 4] = ["from_route", "from_segment", "local", "orphaned"];
const MUTATIONS: [&str; 4] = ["upgrade", "relink", "set_provenance", "mark_local_edit"];
const ITEMS: [&str; 3] = ["conflict", "kept_local_edit", "orphan"];
const RECORDS: [&str; 2] = ["local_edit", "tombstone"];

fn names(object: &serde_json::Map<String, Value>, key: &str, names: &[&str]) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|found| names.contains(&found))
}

/// Whether a proposal (as stored in its table or as a record in an event) holds anything gone.
fn proposal_is_gone(mutations: &Value, items: &Value) -> bool {
    let any = |list: &Value, key: &str, gone: &[&str]| {
        list.as_array().is_some_and(|list| {
            list.iter().any(|entry| {
                entry
                    .as_object()
                    .is_some_and(|entry| names(entry, key, gone))
            })
        })
    };
    any(mutations, "op", &MUTATIONS) || any(items, "item", &ITEMS)
}

/// The record a write puts or removes inside a graph, by the name of its kind.
fn graph_record_kind(write: &Value) -> Option<&str> {
    write
        .get("put")?
        .get("graph")?
        .get("record")?
        .as_object()?
        .keys()
        .next()
        .map(String::as_str)
}

fn removed_key_kind(write: &Value) -> Option<&str> {
    write
        .get("remove")?
        .get("in_graph")?
        .get("key")?
        .as_object()?
        .keys()
        .next()
        .map(String::as_str)
}

fn write_is_gone(write: &Value) -> bool {
    if let Some(kind) = graph_record_kind(write).or_else(|| removed_key_kind(write)) {
        return RECORDS.contains(&kind);
    }
    write
        .get("put")
        .and_then(|put| put.get("proposal"))
        .and_then(|proposal| proposal.get("draft"))
        .and_then(Value::as_object)
        .is_some_and(|draft| {
            proposal_is_gone(
                draft.get("mutations").unwrap_or(&Value::Null),
                draft.get("items").unwrap_or(&Value::Null),
            )
        })
}

/// Drops, anywhere in `value`, each node state's `provenance`; whether it changed anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            if names(object, "state", &STATES) && names(object, "provenance", &PROVENANCES) {
                changed |= object.remove("provenance").is_some();
            }
            for nested in object.values_mut() {
                changed |= rewrite(nested);
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

/// An event's delta without the writes that are gone, and the rest rewritten.
fn rewrite_delta(delta: &mut Value) -> bool {
    let mut changed = false;
    if let Value::Array(writes) = delta {
        let before = writes.len();
        writes.retain(|write| !write_is_gone(write));
        changed = writes.len() != before;
    }
    rewrite(delta) | changed
}

/// Runs the step over every stored JSON value that can hold a node state, a write, or a
/// proposal, and removes the proposals that hold anything gone.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT id, mutations, items FROM proposals";
    for row in rows(connection, select, Vec::new())? {
        let parse = |index| -> Result<Value, SqlError> {
            serde_json::from_str(&row.text(index).map_err(failed)?)
                .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))
        };
        if proposal_is_gone(&parse(1)?, &parse(2)?) {
            execute(
                connection,
                "DELETE FROM proposals WHERE id = ?1",
                vec![text(&row.text(0).map_err(failed)?)],
            )?;
        }
    }
    column(
        connection,
        "proposals",
        "id",
        &["mutations", "items"],
        rewrite,
    )?;
    column(connection, "events", "seq", &["delta"], rewrite_delta)?;
    column(
        connection,
        "patch_receipts",
        "patch_id",
        &["proposals", "deployment_touched"],
        rewrite,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_state_loses_its_provenance_and_a_rerun_changes_nothing() {
        let mut value = serde_json::json!({
            "put": [{"state": "todo", "provenance": "from_route", "atomic": true}],
        });
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!({"put": [{"state": "todo", "atomic": true}]})
        );
        assert!(!rewrite(&mut value));
    }

    #[test]
    fn an_object_that_only_looks_like_a_node_state_is_left_alone() {
        let lookalike = serde_json::json!({"state": "todo", "provenance": "mine"});
        let mut value = lookalike.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, lookalike);
    }

    #[test]
    fn marker_and_tombstone_writes_leave_an_events_delta() {
        let node = serde_json::json!({"graph": {"graph": {"journey": "j_one"}, "record": {
            "node_state": {"node": "n_a", "state": {"state": "todo", "provenance": "local", "atomic": false}}}}});
        let marker = serde_json::json!({"graph": {"graph": {"journey": "j_one"}, "record": {
            "local_edit": {"node": "n_a", "edit": "shape"}}}});
        let tombstone = serde_json::json!({"graph": {"graph": {"journey": "j_one"}, "record": {
            "tombstone": "n_b"}}});
        let unmarked = serde_json::json!({"in_graph": {"graph": {"journey": "j_one"}, "key": {
            "local_edit": {"node": "n_a", "edit": "shape"}}}});
        let mut value = serde_json::json!([
            {"put": node}, {"put": marker}, {"put": tombstone}, {"remove": unmarked}]);
        assert!(rewrite_delta(&mut value));
        let kept = serde_json::json!({"graph": {"graph": {"journey": "j_one"}, "record": {
            "node_state": {"node": "n_a", "state": {"state": "todo", "atomic": false}}}}});
        assert_eq!(value, serde_json::json!([{"put": kept}]));
        assert!(!rewrite_delta(&mut value));
    }

    #[test]
    fn a_proposal_holding_an_upgrade_or_a_conflict_is_gone() {
        let upgrade = serde_json::json!([{"op": "upgrade", "to": 2}]);
        let conflict = serde_json::json!([{"item": "conflict"}]);
        let plain = serde_json::json!([{"op": "transition"}]);
        let exclusion = serde_json::json!([{"item": "exclusion"}]);
        assert!(proposal_is_gone(&upgrade, &exclusion));
        assert!(proposal_is_gone(&plain, &conflict));
        assert!(!proposal_is_gone(&plain, &exclusion));
    }
}
