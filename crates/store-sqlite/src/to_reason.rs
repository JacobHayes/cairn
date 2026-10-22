//! The code step of migration 19: the stored node state's `skip_reason` is now `reason`,
//! because a group marked done keeps one too. SQL cannot tell a node state in stored JSON
//! from text a person wrote, so each stored value is walked and only an object that is a
//! node state (a `state` that is one of the state names beside a `provenance` that is one of
//! the provenance names) is changed. The one map keyed and valued by what a person chose, a
//! conflict resolution's `map_choices.map`, is never entered. An object already holding
//! `reason` is left as it is, so a rerun changes nothing.
//!
//! The rewritten columns are the ones the earlier code steps rewrote: `proposals.mutations`
//! and `.items`, `events.subject` and `.delta`, and `patch_receipts.proposals` and
//! `.deployment_touched`.

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::SqlError;
use crate::stored_json::{column, rename_key};

const STATES: [&str; 8] = [
    "todo", "active", "done", "skipped", "open", "decided", "pending", "derived",
];
const PROVENANCES: [&str; 4] = ["from_route", "from_segment", "local", "orphaned"];

/// Whether `object` names one of `names` under `key`.
fn names(object: &serde_json::Map<String, Value>, key: &str, names: &[&str]) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|found| names.contains(&found))
}

/// Renames, anywhere in `value`, each node state's `skip_reason`; whether it changed anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            if names(object, "state", &STATES)
                && names(object, "provenance", &PROVENANCES)
                && !object.contains_key("reason")
            {
                changed |= rename_key(object, "skip_reason", "reason");
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

/// Runs the step over every stored JSON value that can hold a node state.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    column(
        connection,
        "proposals",
        "id",
        &["mutations", "items"],
        rewrite,
    )?;
    column(connection, "events", "seq", &["subject", "delta"], rewrite)?;
    column(
        connection,
        "patch_receipts",
        "patch_id",
        &["proposals", "deployment_touched"],
        rewrite,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_state_is_renamed_once_and_a_rerun_changes_nothing() {
        let mut value = serde_json::json!({
            "put": [{"state": "skipped", "provenance": "local", "skip_reason": "Needed."}],
        });
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!({
                "put": [{"state": "skipped", "provenance": "local", "reason": "Needed."}],
            })
        );
        let renamed = value.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, renamed);
    }

    #[test]
    fn an_object_that_only_looks_like_a_node_state_is_left_alone() {
        let lookalike =
            serde_json::json!({"state": "skipped", "provenance": "mine", "skip_reason": "x"});
        let mut value = lookalike.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, lookalike);
    }
}
