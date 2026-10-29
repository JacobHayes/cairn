//! The code step of migration 26: there is one kind of route, and an insertion keeps no
//! record of its own. Each stored insertion becomes the pointer ("from route X, version N") on
//! its inserted root, and the stored JSON that named the segment kind is rewritten. SQL cannot
//! tell stored JSON from text a person wrote, so each value is walked:
//!
//! - an `insert_segment` mutation becomes `insert_route`, its `segment` the `route`, and an
//!   edge end `segment` an end `route`;
//! - a `create_route` mutation and a route's header lose their `kind`, and the violations of
//!   the segment rules are dropped;
//! - in an event's delta, each node an insertion put gets the pointer it now carries;
//! - in an event's delta, the writes of an insertion record go, and an event whose subject was
//!   an insertion is about its journey or route.
//!
//! A rerun changes nothing.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, int, rows, text};
use crate::stored_json::{column, failed, rename_key};

/// The pointer an inserted root now carries, by graph row id and node key.
type Pointers = BTreeMap<(String, String), (String, i64)>;

/// Moves each insertion onto its root: the member copied from the version's root, else the
/// first member the graph still holds. Answers each pointer it set.
fn insertions_to_pointers(connection: &Connection) -> Result<Pointers, SqlError> {
    let mut pointers = Pointers::new();
    let select = "SELECT graph_id, segment, version, nodes FROM insertions ORDER BY graph_id, key";
    for row in rows(connection, select, Vec::new())? {
        let graph = row.text(0).map_err(failed)?;
        let segment = row.text(1).map_err(failed)?;
        let version = row.int(2).map_err(failed)?;
        let members: BTreeMap<String, String> = serde_json::from_str(&row.text(3).map_err(failed)?)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        let version_root = rows(
            connection,
            "SELECT key FROM nodes WHERE graph_id = ?1 AND parent_key IS NULL",
            vec![text(&format!("version/{segment}/{version}"))],
        )?
        .first()
        .map(|found| found.text(0).map_err(failed))
        .transpose()?;
        let held: Vec<&String> = members
            .keys()
            .filter(|member| {
                rows(
                    connection,
                    "SELECT 1 FROM nodes WHERE graph_id = ?1 AND key = ?2",
                    vec![text(&graph), text(member)],
                )
                .is_ok_and(|found| !found.is_empty())
            })
            .collect();
        let root = held
            .iter()
            .find(|member| Some(&members[**member]) == version_root.as_ref())
            .or(held.first());
        if let Some(root) = root {
            execute(
                connection,
                "UPDATE nodes SET from_route = ?1, from_version = ?2 \
                 WHERE graph_id = ?3 AND key = ?4",
                vec![text(&segment), int(version), text(&graph), text(root)],
            )?;
            pointers.insert((graph, (*root).clone()), (segment, version));
        }
    }
    Ok(pointers)
}

/// The row id of the graph a stored write names, for the graphs that hold insertions.
fn graph_of(write: &Value) -> Option<String> {
    let graph = write.pointer("/put/graph/graph")?.as_object()?;
    let (kind, id) = graph.iter().next()?;
    match kind.as_str() {
        "journey" => Some(format!("journey/{}", id.as_str()?)),
        "route_draft" => Some(format!("draft/{}", id.as_str()?)),
        _ => None,
    }
}

/// The pointer of a node as a write found it: in the graph itself, or, for a route's draft,
/// as its published version holds it (an insertion made in a draft that was published since).
fn pointer_of<'a>(pointers: &'a Pointers, graph: &str, key: &str) -> Option<&'a (String, i64)> {
    pointers
        .get(&(graph.to_owned(), key.to_owned()))
        .or_else(|| {
            let route = graph.strip_prefix("draft/")?;
            let published = format!("version/{route}/");
            pointers
                .iter()
                .find(|((held, node), _)| held.starts_with(&published) && node == key)
                .map(|(_, found)| found)
        })
}

/// Gives each node a write puts the pointer its insertion left on it, so replaying the log
/// reaches the stored state.
fn put_pointers(delta: &mut Value, pointers: &Pointers) -> bool {
    let Value::Array(writes) = delta else {
        return false;
    };
    let mut changed = false;
    for write in writes {
        let Some(graph) = graph_of(write) else {
            continue;
        };
        let Some(node) = write
            .pointer_mut("/put/graph/record/node")
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let key = node.get("key").and_then(Value::as_str).map(str::to_owned);
        if let Some((route, version)) = key.and_then(|key| pointer_of(pointers, &graph, &key)) {
            node.insert(
                "from_route".to_owned(),
                serde_json::json!({"route": route, "version": version}),
            );
            changed = true;
        }
    }
    changed
}

/// The codes of the violations of the segment rules, which no longer exist.
const GONE_CODES: [&str; 2] = ["segment_rule", "not_a_process_route"];

/// A review item, or a violation itself, that reports a removed rule.
fn is_removed_violation(item: &Value) -> bool {
    let code = item
        .get("violation")
        .unwrap_or(item)
        .get("code")
        .and_then(Value::as_str);
    code.is_some_and(|code| GONE_CODES.contains(&code))
}

fn write_is_gone(write: &Value) -> bool {
    let inserted = |value: Option<&Value>| {
        value
            .and_then(Value::as_object)
            .is_some_and(|object| object.contains_key("insertion"))
    };
    inserted(write.pointer("/put/graph/record")) || inserted(write.pointer("/remove/in_graph/key"))
}

/// Rewrites, anywhere in `value`, what named the segment kind; whether it changed anything.
fn rewrite(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut changed = false;
            match object.get("op").and_then(Value::as_str) {
                Some("insert_segment") => {
                    object.insert("op".to_owned(), Value::from("insert_route"));
                    rename_key(object, "segment", "route");
                    changed = true;
                }
                Some("create_route") => changed |= object.remove("kind").is_some(),
                _ => {}
            }
            if object.len() == 1 && object.get("segment").is_some_and(Value::is_string) {
                changed |= rename_key(object, "segment", "route");
            }
            if let Some(Value::Object(header)) = object.get_mut("route_header") {
                changed |= header.remove("kind").is_some();
            }
            for nested in object.values_mut() {
                changed |= rewrite(nested);
            }
            changed
        }
        Value::Array(items) => {
            let before = items.len();
            items.retain(|item| !is_removed_violation(item));
            let mut changed = items.len() != before;
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

/// An event that was about an insertion is about the journey or route it is logged on.
fn insertion_subjects(connection: &Connection) -> Result<(), SqlError> {
    let select = "SELECT seq, log_kind, log_id FROM events \
                  WHERE json_extract(subject, '$.insertion') IS NOT NULL";
    for row in rows(connection, select, Vec::new())? {
        let subject =
            serde_json::json!({ row.text(1).map_err(failed)?: row.text(2).map_err(failed)? });
        execute(
            connection,
            "UPDATE events SET subject = ?1 WHERE seq = ?2",
            vec![text(&subject.to_string()), int(row.int(0).map_err(failed)?)],
        )?;
    }
    Ok(())
}

/// Runs the step over the stored insertions and every stored JSON value that can hold a
/// mutation or a route's header.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    let pointers = insertions_to_pointers(connection)?;
    insertion_subjects(connection)?;
    column(
        connection,
        "proposals",
        "id",
        &["mutations", "items"],
        rewrite,
    )?;
    column(connection, "events", "seq", &["delta"], |delta| {
        put_pointers(delta, &pointers) | rewrite_delta(delta)
    })?;
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
    fn a_segment_mutation_becomes_a_route_mutation_and_a_rerun_changes_nothing() {
        let mut value = serde_json::json!([
            {"op": "insert_segment", "insertion": "i_a", "segment": {"route": "sec", "version": 1},
             "edges": [{"node": {"segment": "n_a"}, "requires": {"host": "n_b"}}]},
            {"op": "create_route", "name": "X", "kind": "segment"},
        ]);
        assert!(rewrite(&mut value));
        assert_eq!(
            value,
            serde_json::json!([
                {"op": "insert_route", "insertion": "i_a", "route": {"route": "sec", "version": 1},
                 "edges": [{"node": {"route": "n_a"}, "requires": {"host": "n_b"}}]},
                {"op": "create_route", "name": "X"},
            ])
        );
        assert!(!rewrite(&mut value));
    }

    #[test]
    fn the_writes_of_an_insertion_record_leave_an_events_delta() {
        let record = serde_json::json!({"put": {"graph": {"graph": {"journey": "j_one"},
            "record": {"insertion": {"key": "i_a"}}}}});
        let removed = serde_json::json!({"remove": {"in_graph": {"graph": {"journey": "j_one"},
            "key": {"insertion": "i_a"}}}});
        let node = serde_json::json!({"put": {"graph": {"graph": {"journey": "j_one"},
            "record": {"node": {"key": "n_a"}}}}});
        let mut value = serde_json::json!([record, node.clone(), removed]);
        assert!(rewrite_delta(&mut value));
        assert_eq!(value, serde_json::json!([node]));
        assert!(!rewrite_delta(&mut value));
    }

    #[test]
    fn a_violation_of_a_removed_rule_leaves_stored_items_and_a_rerun_changes_nothing() {
        let kept = serde_json::json!({"item": "exclusion", "node": "n_a", "excluded": true});
        let gone = serde_json::json!({"item": "violation", "violation": {"code": "not_a_process_route", "message": "m"}});
        let mut value = serde_json::json!([kept.clone(), gone]);
        assert!(rewrite(&mut value));
        assert_eq!(value, serde_json::json!([kept]));
        assert!(!rewrite(&mut value));
    }

    #[test]
    fn a_node_put_by_an_insertion_gets_the_pointer_the_graph_now_holds() {
        let put = |key: &str| {
            serde_json::json!({"put": {"graph": {"graph": {"journey": "j_one"},
            "record": {"node": {"key": key}}}}})
        };
        let mut pointers = Pointers::new();
        pointers.insert(
            ("journey/j_one".to_owned(), "n_root".to_owned()),
            ("sec".to_owned(), 2),
        );
        let mut delta = serde_json::json!([put("n_root"), put("n_child")]);
        assert!(put_pointers(&mut delta, &pointers));
        assert_eq!(
            delta[0].pointer("/put/graph/record/node/from_route"),
            Some(&serde_json::json!({"route": "sec", "version": 2}))
        );
        assert!(
            delta[1]
                .pointer("/put/graph/record/node/from_route")
                .is_none()
        );
    }

    #[test]
    fn a_node_put_in_a_draft_gets_the_pointer_its_published_version_holds() {
        let put = serde_json::json!({"put": {"graph": {"graph": {"route_draft": "r"},
            "record": {"node": {"key": "n_root"}}}}});
        let mut pointers = Pointers::new();
        pointers.insert(
            ("version/r/1".to_owned(), "n_root".to_owned()),
            ("sec".to_owned(), 3),
        );
        let mut delta = serde_json::json!([put]);
        assert!(put_pointers(&mut delta, &pointers));
        assert!(
            delta[0]
                .pointer("/put/graph/record/node/from_route")
                .is_some()
        );
    }
}
