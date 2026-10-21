//! Migration 0014 on a database frozen at migration 13 (see `at_migration_13.sql`, the schema
//! the code before the rename made, with rows written by hand in that code's words). The
//! expected state is written here, not derived from the current fixtures: which position is
//! renamed, and which text stays as written.

#[cfg(test)]
mod milestone_done {
    use std::path::{Path, PathBuf};

    use cairn_store::{Document, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;
    use serde_json::{Value, json};

    const DUMP: &str = include_str!("at_migration_13.sql");

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A database file holding the frozen dump.
    fn frozen(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("milestone-done-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        path
    }

    /// The store opened on the frozen dump, which migrates it.
    fn migrated(name: &str) -> (PathBuf, SqliteStore) {
        let path = frozen(name);
        let store = SqliteStore::open(&path).unwrap();
        (path, store)
    }

    /// Every row of a query, each column as text.
    fn rows(path: &Path, sql: &str) -> Vec<Vec<String>> {
        use rusqlite::types::Value as Column;
        let connection = rusqlite::Connection::open(path).unwrap();
        let mut statement = connection.prepare(sql).unwrap();
        let count = statement.column_count();
        statement
            .query_map([], |row| {
                Ok((0..count)
                    .map(|i| match row.get::<_, Column>(i).unwrap() {
                        Column::Text(text) => text,
                        Column::Integer(n) => n.to_string(),
                        Column::Null => String::new(),
                        other => format!("{other:?}"),
                    })
                    .collect())
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// The JSON in column 0 of each row, in order.
    fn documents(path: &Path, sql: &str) -> Vec<Value> {
        rows(path, sql)
            .into_iter()
            .map(|row| serde_json::from_str(&row[0]).unwrap())
            .collect()
    }

    fn put(record: Value) -> Value {
        let mut write = json!({ "put": { "graph": { "graph": { "journey": "j_one" } } } });
        write["put"]["graph"]["record"] = record;
        write
    }

    #[test]
    fn a_finished_milestone_is_stored_as_done() {
        run(async {
            let (path, store) = migrated("states");
            drop(store);
            assert_eq!(
                rows(
                    &path,
                    "SELECT node, state, finished_on FROM node_states \
                     WHERE graph_id = 'journey/j_one' ORDER BY node"
                ),
                [
                    ["n_gate", "pending", ""],
                    ["n_kick", "done", "2026-10-05"],
                    ["n_reach", "todo", ""],
                    ["n_work", "done", "2026-10-04"],
                ]
            );
            // The rebuilt table refuses the old state.
            let connection = rusqlite::Connection::open(&path).unwrap();
            assert!(
                connection
                    .execute(
                        "INSERT INTO node_states (graph_id, node, state, provenance, atomic) \
                         VALUES ('journey/j_one', 'n_extra', 'reached', 'local', 0)",
                        ()
                    )
                    .is_err()
            );
        });
    }

    #[test]
    fn the_flag_keeps_its_value_under_its_new_name() {
        run(async {
            let (path, store) = migrated("flag");
            assert_eq!(
                rows(
                    &path,
                    "SELECT graph_id, key, auto_done FROM nodes WHERE kind = 'milestone' \
                     ORDER BY graph_id, key"
                ),
                [
                    ["draft/r_launch", "n_go", "1"],
                    ["journey/j_one", "n_gate", "0"],
                    ["journey/j_one", "n_kick", "1"],
                    ["version/r_launch/1", "n_go", "1"],
                ]
            );
            let Some(Document::Journey(journey)) = store
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap()
            else {
                panic!("no journey")
            };
            let graph = serde_json::to_value(&journey.graph).unwrap();
            let nodes = &graph["nodes"];
            let kick = nodes
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["key"] == "n_kick")
                .unwrap();
            assert_eq!(kick["auto_done"], json!(true));
            assert!(kick.get("auto_reach").is_none());
            let state = &graph["state"]["nodes"]["n_kick"];
            assert_eq!(state["state"], json!("done"));
            assert_eq!(state["finished_on"], json!("2026-10-05"));
        });
    }

    #[test]
    fn a_local_edit_of_the_flag_names_the_new_field_and_other_edits_stay() {
        run(async {
            let (path, store) = migrated("edits");
            drop(store);
            assert_eq!(
                rows(
                    &path,
                    "SELECT node, aspect, target FROM local_edits ORDER BY target"
                ),
                [
                    ["n_gate", "field", "auto_done"],
                    ["n_gate", "field", "title"],
                ]
            );
        });
    }

    #[test]
    fn events_are_rewritten_where_a_name_stands_and_nowhere_else() {
        run(async {
            let (path, store) = migrated("events");
            drop(store);
            assert_eq!(
                rows(&path, "SELECT event_type FROM events ORDER BY seq"),
                [
                    ["node_transitioned"],
                    ["node_transitioned"],
                    ["node_changed"],
                    ["node_added"],
                    ["node_added"],
                ]
            );
            assert_eq!(
                documents(&path, "SELECT delta FROM events ORDER BY seq"),
                [
                    json!([put(json!({ "node_state": { "node": "n_kick", "state": {
                        "state": "done", "provenance": "from_route", "finished_on": "2026-10-05"
                    } } }))]),
                    json!([put(json!({ "node_state": { "node": "n_work", "state": {
                        "state": "done", "provenance": "from_route", "finished_on": "2026-10-04"
                    } } }))]),
                    json!([
                        put(
                            json!({ "node_field": { "node": "n_gate", "value": { "auto_done": true } } })
                        ),
                        put(
                            json!({ "local_edit": { "node": "n_gate", "edit": { "field": "auto_done" } } })
                        ),
                    ]),
                    // A title, a description and a state named like the old words stay as written.
                    json!([
                        put(
                            json!({ "node": { "key": "n_reach", "id": "reach", "kind": "action",
                            "title": "auto_reach", "description": "reached" } })
                        ),
                        put(json!({ "node_state": { "node": "n_reach", "state": {
                            "state": "todo", "provenance": "local" } } })),
                    ]),
                    json!([put(
                        json!({ "node": { "key": "n_late", "id": "late", "kind": "milestone",
                        "title": "Late", "final": false, "auto_done": true } })
                    )]),
                ]
            );
        });
    }

    #[test]
    fn proposals_are_rewritten_where_a_name_stands_and_nowhere_else() {
        run(async {
            let (path, store) = migrated("proposals");
            drop(store);
            assert_eq!(
                documents(&path, "SELECT mutations FROM proposals WHERE id = 'pr_one'"),
                [json!([
                    { "op": "transition", "node": "n_gate", "transition": "complete" },
                    { "op": "transition", "node": "n_work", "transition": "complete" },
                    { "op": "add_node", "node": { "key": "n_new", "id": "new", "kind": "milestone",
                        "title": "auto_reach", "auto_done": true } },
                    { "op": "add_node", "node": { "key": "n_act", "id": "act", "kind": "action",
                        "title": "reach", "description": "auto_reach" } },
                    { "op": "set_node_field", "node": "n_gate", "value": { "auto_done": false } },
                    { "op": "set_node_field", "node": "n_gate", "value": { "title": "auto_reach" } },
                    { "op": "answer", "decision": "n_note", "value": { "text": "reach" } },
                ])]
            );
            assert_eq!(
                documents(&path, "SELECT items FROM proposals WHERE id = 'pr_one'"),
                [json!([
                    { "item": "conflict", "conflict": { "about": "field", "node": "n_gate",
                        "journey": { "auto_done": true }, "route": { "auto_done": false } },
                        "resolution": "keep_journey" },
                    // A choice a person named, and the map between choices, stay as written.
                    { "item": "conflict", "conflict": { "about": "answer", "decision": "n_choice",
                        "answer": { "single_choice": "field" }, "choices": ["auto_reach"] },
                        "resolution": { "map_choices": { "map": { "field": "auto_reach" } } } },
                    { "item": "kept_local_edit",
                        "kept": { "node": "n_gate", "edit": { "field": "auto_done" } } },
                    { "item": "conflict", "conflict": { "about": "shape", "node": "n_kick",
                        "journey": { "key": "n_kick", "id": "kick", "kind": "milestone",
                            "title": "Kickoff", "auto_done": true },
                        "route": { "key": "n_kick", "id": "kick", "kind": "action",
                            "title": "Kickoff" } } },
                ])]
            );
            assert_eq!(
                documents(&path, "SELECT mutations FROM proposals WHERE id = 'pr_two'"),
                [
                    json!([{ "op": "add_node", "node": { "key": "n_z", "id": "z",
                    "kind": "milestone", "title": "Z", "auto_done": false } }])
                ]
            );
        });
    }

    #[test]
    fn text_a_person_wrote_is_not_touched() {
        run(async {
            let (path, store) = migrated("text");
            drop(store);
            assert_eq!(
                rows(
                    &path,
                    "SELECT title, description FROM nodes WHERE graph_id = 'journey/j_one' \
                     AND key IN ('n_work', 'n_reach') ORDER BY key"
                ),
                [
                    ["auto_reach", "reach"],
                    ["Work", "Reach the reached state; auto_reach stays on."],
                ]
            );
            assert_eq!(
                rows(&path, "SELECT title, content FROM annotations"),
                [["reached", "Kickoff reached; the gate is auto_reach."]]
            );
            assert_eq!(
                rows(
                    &path,
                    "SELECT description FROM proposals WHERE id = 'pr_one'"
                ),
                [["reached"]]
            );
        });
    }

    #[test]
    fn a_migrated_store_opens_again_unchanged() {
        run(async {
            let (path, store) = migrated("again");
            drop(store);
            let before = rows(
                &path,
                "SELECT seq, event_type, delta FROM events ORDER BY seq",
            );
            drop(SqliteStore::open(&path).unwrap());
            assert_eq!(
                rows(
                    &path,
                    "SELECT seq, event_type, delta FROM events ORDER BY seq"
                ),
                before
            );
            assert_eq!(
                rows(&path, "SELECT MAX(version) FROM schema_migrations"),
                [["15"]]
            );
        });
    }
}
