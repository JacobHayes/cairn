//! The code step of migration 23: a guard bypass is no longer an override applied beside a
//! completion; the completion itself goes ahead anyway (the `complete_anyway` transition,
//! carrying the optional reason). A stored bypass no longer lists its guards: the failures it
//! recorded say which guards it bypassed, so every stored bypass loses its `guards` list.
//!
//! In every stored list of mutations (`proposals.mutations` and `.items`, and the proposal
//! records inside `events.delta`), an applied
//! `guard_bypass` override is removed and the completion of the same node in the list becomes
//! `complete_anyway` with the bypass's reason; a bypass with no completion beside it covered
//! nothing and is dropped, as is the removal of a bypass. The log keeps its meaning with a new
//! encoding: an event that applied a bypass (`guard_bypassed`) is now an `override_applied`
//! event, whose write of the node's overrides still holds the bypass. A rerun changes
//! nothing.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, rows, text};
use crate::stored_json::{column, failed};

fn op(mutation: &Value) -> Option<&str> {
    mutation.get("op").and_then(Value::as_str)
}

fn node(mutation: &Value) -> Option<&str> {
    mutation.get("node").and_then(Value::as_str)
}

/// The reason (or none) of an applied `guard_bypass` override, by its node.
fn bypass(mutation: &Value) -> Option<(String, Option<Value>)> {
    if op(mutation) != Some("apply_override") {
        return None;
    }
    let applied = mutation.get("override")?.get("guard_bypass")?;
    Some((node(mutation)?.to_owned(), applied.get("reason").cloned()))
}

fn bypass_removal(mutation: &Value) -> bool {
    op(mutation) == Some("remove_override")
        && mutation.get("kind").and_then(Value::as_str) == Some("guard_bypass")
}

fn completion(mutation: &Value) -> bool {
    op(mutation) == Some("transition")
        && mutation.get("transition").and_then(Value::as_str) == Some("complete")
}

/// Rewrites, anywhere in `value`, each list of mutations holding a guard bypass; whether it
/// changed anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            if let Some(Value::Object(stored)) = object.get_mut("bypass") {
                changed |= stored.remove("guards").is_some();
            }
            // A violation no longer says which guard a bypass would accept it past; its
            // failures do.
            if object.contains_key("code") && object.contains_key("message") {
                changed |= object.remove("bypassable").is_some();
            }
            for nested in object.values_mut() {
                changed |= rewrite(nested);
            }
            changed
        }
        Value::Array(items) => {
            let mut changed = false;
            for nested in items.iter_mut() {
                changed |= rewrite(nested);
            }
            let reasons: BTreeMap<String, Option<Value>> =
                items.iter().filter_map(bypass).collect();
            let before = items.len();
            items.retain(|item| bypass(item).is_none() && !bypass_removal(item));
            changed |= items.len() != before;
            for item in items.iter_mut().filter(|item| completion(item)) {
                let Some(reason) = node(item).and_then(|found| reasons.get(found)) else {
                    continue;
                };
                let mut ahead = serde_json::Map::new();
                if let Some(reason) = reason {
                    ahead.insert("reason".to_owned(), reason.clone());
                }
                item["transition"] = serde_json::json!({ "complete_anyway": ahead });
                changed = true;
            }
            changed
        }
        _ => false,
    }
}

/// Runs the step over every stored bypass, list of mutations, and event.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT graph_id, node, bypass FROM overrides WHERE bypass IS NOT NULL";
    for row in rows(connection, select, Vec::new())? {
        let (graph, node) = (row.text(0).map_err(failed)?, row.text(1).map_err(failed)?);
        let mut stored: Value = serde_json::from_str(&row.text(2).map_err(failed)?)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        if stored
            .as_object_mut()
            .is_some_and(|bypass| bypass.remove("guards").is_some())
        {
            execute(
                connection,
                "UPDATE overrides SET bypass = ?1 WHERE graph_id = ?2 AND node = ?3",
                vec![text(&stored.to_string()), text(&graph), text(&node)],
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
    column(connection, "events", "seq", &["delta"], rewrite)?;
    execute(
        connection,
        "UPDATE events SET event_type = 'override_applied' WHERE event_type = 'guard_bypassed'",
        Vec::new(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_bypass_loses_its_guard_list_wherever_it_sits() {
        let mut value = serde_json::json!([{"put": {"overrides": {"node": "n_a", "overrides": {
            "keep": "Kept.",
            "bypass": {"guards": ["has_note"], "reason": "Phoned.", "failures": ["missing_note"]},
        }}}}]);
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!([{"put": {"overrides": {"node": "n_a", "overrides": {
                "keep": "Kept.",
                "bypass": {"reason": "Phoned.", "failures": ["missing_note"]},
            }}}}])
        );
        assert!(!rewrite(&mut value));
    }

    #[test]
    fn a_stored_violation_loses_the_guard_a_bypass_would_accept_it_past() {
        let mut value = serde_json::json!({"code": "guard_failed", "at": {}, "message": "m",
            "bypassable": "has_note", "failures": ["missing_note"]});
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!({"code": "guard_failed", "at": {}, "message": "m",
                "failures": ["missing_note"]})
        );
    }

    #[test]
    fn a_bypass_beside_its_completion_becomes_the_completion_going_ahead() {
        let bypass = |node: &str, applied: Value| {
            serde_json::json!({"op": "apply_override", "node": node,
                "override": {"guard_bypass": applied}})
        };
        let complete = |node: &str| serde_json::json!({"op": "transition", "node": node, "transition": "complete"});
        let mut value = serde_json::json!([
            bypass("n_a", serde_json::json!({"guards": ["has_note"], "reason": "Phoned."})),
            complete("n_a"),
            complete("n_b"),
            bypass("n_c", serde_json::json!({"guards": ["all_ticked"]})),
            complete("n_c"),
            bypass("n_d", serde_json::json!({"guards": ["has_note"]})),
            {"op": "remove_override", "node": "n_e", "kind": "guard_bypass"},
        ]);
        assert!(rewrite(&mut value));
        let ahead = |node: &str, ahead: Value| {
            serde_json::json!({"op": "transition", "node": node,
                "transition": {"complete_anyway": ahead}})
        };
        assert_eq!(
            value,
            serde_json::json!([
                ahead("n_a", serde_json::json!({"reason": "Phoned."})),
                complete("n_b"),
                ahead("n_c", serde_json::json!({})),
            ])
        );
        let rewritten = value.clone();
        assert!(!rewrite(&mut value));
        assert_eq!(value, rewritten);
    }
}
