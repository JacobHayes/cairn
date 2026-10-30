//! The migration that makes a stage an ordinary group, on a database frozen at migration 13
//! (see `at_migration_13.sql`) with groups holding the four fields a stage used to have, their
//! history, and proposals that edited them. An opening becomes a requirement and a closing a
//! due-by (merged into the group's own rule where it has one); a bound that did not gate or
//! close is dropped; groups whose own rule cannot take the closing stop the migration and are
//! all listed; stored events still read back; an open proposal that held a stage edit is
//! discarded with a reason.

#[cfg(test)]
mod plain_stages {
    use std::path::PathBuf;

    use cairn_schema::Write;
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const NODE: &str = "INSERT INTO nodes (graph_id, key, id, kind, title, opens_at, closes_at, gates, closes, due_by) VALUES";

    fn groups(extra: &str) -> String {
        format!(
            r#"
{NODE} ('journey/j_one', 'n_both', 'both', 'group', 'Both', 'n_kick', 'n_gate', 1, 1, NULL);
{NODE} ('journey/j_one', 'n_loose', 'loose', 'group', 'Loose', 'n_kick', 'n_gate', 0, 0, NULL);
{NODE} ('journey/j_one', 'n_zero', 'zero', 'group', 'Zero', NULL, 'n_gate', 1, 1, '{{"after":"n_kick"}}');
{NODE} ('journey/j_one', 'n_same', 'same', 'group', 'Same', NULL, 'n_gate', 1, 1, '{{"before":"n_gate","offset":3}}');
{NODE} ('journey/j_one', 'n_plain', 'plain', 'group', 'Plain', NULL, NULL, NULL, NULL, NULL);
{extra}
"#
        )
    }

    const HISTORY: &str = r#"
INSERT INTO "events" VALUES (60, 'p_g1', 0, 'journey', 'j_one', 'node_added', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000060, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node":{"key":"n_ev","id":"ev","kind":"group","title":"Ev","opens_at":"n_kick","closes_at":"n_gate"}}}}}]');
INSERT INTO "events" VALUES (61, 'p_g2', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000061, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"opens_at":"n_gate"}}}}}}]');
INSERT INTO "events" VALUES (62, 'p_g3', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000062, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"gates":false}}}}}}]');
INSERT INTO "events" VALUES (63, 'p_g4', 0, 'journey', 'j_one', 'node_changed', 'u_lead', NULL, NULL, '{"node":"n_ev"}', 1791288000000000063, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_field":{"node":"n_ev","value":{"closes_at":null}}}}}}]');
INSERT INTO "proposals" VALUES ('pr_stage', 'journey', 'j_one', 1, 'open', 'Stage edit', NULL, 1, '[{"op":"edit_node","node":"n_both","fields":{"opens_at":"n_gate"}}]', '[]', NULL, 'u_lead', 1791288000000000000);
INSERT INTO "proposals" VALUES ('pr_old', 'journey', 'j_one', 1, 'applied', 'Mixed', NULL, 1, '[{"op":"edit_node","node":"n_both","fields":{"closes_at":"n_gate"}},{"op":"transition","node":"n_work","transition":"complete"}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn open(name: &str, script: &str) -> (PathBuf, Result<SqliteStore, String>) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("plain-stages-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(script).unwrap();
        drop(connection);
        let opened = SqliteStore::open(&path).map_err(|error| error.to_string());
        (path, opened)
    }

    fn column<T: rusqlite::types::FromSql>(path: &PathBuf, sql: &str) -> Option<T> {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(sql, [], |row| row.get(0))
            .ok()
    }

    fn requires(path: &PathBuf, node: &str) -> Vec<String> {
        let connection = rusqlite::Connection::open(path).unwrap();
        connection
            .prepare("SELECT requires FROM edges WHERE graph_id = 'journey/j_one' AND node = ?1 ORDER BY requires")
            .unwrap()
            .query_map([node], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn due_by(path: &PathBuf, node: &str) -> Option<serde_json::Value> {
        let stored: Option<String> = column(
            path,
            &format!(
                "SELECT due_by FROM nodes WHERE graph_id = 'journey/j_one' AND key = '{node}'"
            ),
        );
        stored.map(|text| serde_json::from_str(&text).unwrap())
    }

    #[test]
    fn bounds_become_requirements_and_deadlines() {
        let (path, opened) = open("map", &groups(HISTORY));
        drop(opened.unwrap());

        let columns: Vec<String> = rusqlite::Connection::open(&path)
            .unwrap()
            .prepare("SELECT name FROM pragma_table_info('nodes')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for gone in ["opens_at", "closes_at", "gates", "closes"] {
            assert!(!columns.iter().any(|name| name == gone), "{gone}");
        }

        assert_eq!(requires(&path, "n_both"), ["n_kick"]);
        assert_eq!(
            due_by(&path, "n_both").unwrap(),
            serde_json::json!({"before": "n_gate"})
        );
        assert_eq!(requires(&path, "n_loose"), [] as [std::string::String; 0]);
        assert_eq!(
            due_by(&path, "n_loose"),
            None,
            "a bound that did not close is dropped"
        );
        assert_eq!(
            due_by(&path, "n_zero").unwrap(),
            serde_json::json!({"after": ["n_gate", "n_kick"]}),
            "a zero-offset rule takes the milestone as one more source"
        );
        assert_eq!(
            due_by(&path, "n_same").unwrap(),
            serde_json::json!({"before": "n_gate", "offset": 3}),
            "a rule already before the milestone is left alone"
        );
        assert_eq!(due_by(&path, "n_plain"), None);
    }

    #[test]
    fn stored_history_still_reads_back() {
        let (path, opened) = open("history", &groups(HISTORY));
        drop(opened.unwrap());
        let delta = |seq: i64| -> serde_json::Value {
            let text: String = column(
                &path,
                &format!("SELECT delta FROM events WHERE seq = {seq}"),
            )
            .unwrap();
            serde_json::from_str::<Vec<Write>>(&text).expect("a stored delta deserializes");
            serde_json::from_str(&text).unwrap()
        };
        let created = delta(60);
        let node = &created[0]["put"]["graph"]["record"]["node"];
        assert_eq!(node["requires"], serde_json::json!(["n_kick"]));
        assert_eq!(node["due_by"], serde_json::json!({"before": "n_gate"}));
        assert!(node.get("opens_at").is_none() && node.get("closes_at").is_none());

        let moved = delta(61);
        assert_eq!(
            moved.as_array().unwrap().len(),
            2,
            "the old requirement goes, the new comes"
        );
        assert_eq!(
            moved[0]["remove"]["in_graph"]["key"]["edge"]["requires"],
            "n_kick"
        );
        assert_eq!(
            moved[1]["put"]["graph"]["record"]["edge"]["requires"],
            "n_gate"
        );

        let dropped = delta(62);
        assert_eq!(
            dropped.as_array().unwrap().len(),
            1,
            "a group that stops gating loses its requirement"
        );
        assert_eq!(
            dropped[0]["remove"]["in_graph"]["key"]["edge"]["requires"],
            "n_gate"
        );

        let cleared = delta(63);
        assert_eq!(
            cleared[0]["put"]["graph"]["record"]["node_field"]["value"],
            serde_json::json!({"due_by": null})
        );
    }

    #[test]
    fn proposals_lose_stage_edits() {
        let (path, opened) = open("proposals", &groups(HISTORY));
        drop(opened.unwrap());
        let status = |id: &str| -> String {
            column(
                &path,
                &format!("SELECT status FROM proposals WHERE id = '{id}'"),
            )
            .unwrap()
        };
        assert_eq!(status("pr_stage"), "discarded");
        let reason: String = column(
            &path,
            "SELECT description FROM proposals WHERE id = 'pr_stage'",
        )
        .unwrap();
        assert!(
            !reason.is_empty(),
            "an open proposal is discarded with a reason"
        );
        let mutations: String = column(
            &path,
            "SELECT mutations FROM proposals WHERE id = 'pr_stage'",
        )
        .unwrap();
        assert_eq!(mutations, "[]");

        assert_eq!(status("pr_old"), "applied");
        let kept: serde_json::Value = serde_json::from_str(
            &column::<String>(&path, "SELECT mutations FROM proposals WHERE id = 'pr_old'")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            kept.as_array().unwrap().len(),
            1,
            "a settled proposal keeps its other mutations"
        );
        assert_eq!(kept[0]["op"], "transition");
        assert_eq!(
            status("pr_one"),
            "open",
            "a proposal with no stage edit is untouched"
        );
    }

    #[test]
    fn groups_that_cannot_take_their_closing_stop_the_migration() {
        let bad = groups(&format!(
            "{NODE} ('journey/j_one', 'n_bad_a', 'bada', 'group', 'A', NULL, 'n_gate', 1, 1, '{{\"after\":\"n_kick\",\"offset\":2}}');\n\
             {NODE} ('journey/j_one', 'n_bad_b', 'badb', 'group', 'B', NULL, 'n_gate', 1, 1, '{{\"after\":\"n_kick\",\"offset\":5}}');"
        ));
        let (path, opened) = open("refused", &bad);
        let Err(message) = opened else {
            panic!("the migration must refuse");
        };
        assert!(
            message.contains("n_bad_a") && message.contains("n_bad_b"),
            "{message}"
        );
        let closes: Option<String> = column(
            &path,
            "SELECT closes_at FROM nodes WHERE graph_id = 'journey/j_one' AND key = 'n_bad_a'",
        );
        assert_eq!(closes.as_deref(), Some("n_gate"));
    }
}
