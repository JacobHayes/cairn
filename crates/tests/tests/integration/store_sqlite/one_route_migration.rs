//! The migration to one kind of route, on a database frozen at migration 13 (see
//! `at_migration_13.sql`) that holds a segment route with a version, a journey with an
//! insertion of it, the insertion's event and a proposal that inserts. The route has no kind,
//! the insertion's root keeps a pointer to the version it was copied from, no other node
//! does, the insertion table is gone, and the stored JSON that named the segment kind reads
//! as a route's.

#[cfg(test)]
mod one_route {
    use std::path::PathBuf;

    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const OLD: &str = r#"
INSERT INTO "routes" VALUES ('seg', 1, 'Security review', NULL, 0, 'segment');
INSERT INTO "route_versions" VALUES ('seg', 1, 1791288000000000000);
INSERT INTO "graphs" VALUES ('version/seg/1', 'route_version', NULL, 'seg', 1, NULL);
INSERT INTO "nodes" (graph_id, key, id, kind, title, parent_key) VALUES ('version/seg/1', 'n_seg', 'seg', 'group', 'Security review', NULL);
INSERT INTO "nodes" (graph_id, key, id, kind, title, parent_key) VALUES ('version/seg/1', 'n_c', 'check', 'action', 'Check', 'n_seg');
INSERT INTO "nodes" (graph_id, key, id, kind, title, parent_key) VALUES ('journey/j_one', 'n_ins', 'seg', 'group', 'Security review', NULL);
INSERT INTO "nodes" (graph_id, key, id, kind, title, parent_key) VALUES ('journey/j_one', 'n_ins_c', 'check', 'action', 'Check', 'n_ins');
INSERT INTO "insertions" VALUES ('journey/j_one', 'i_one', 'seg', 1, NULL, '{}', '{}', '{"n_ins":"n_seg","n_ins_c":"n_c"}');
INSERT INTO "events" VALUES (60, 'p_ins', 0, 'journey', 'j_one', 'segment_inserted', 'u_lead', NULL, NULL, '{"insertion":"i_one"}', 1791288000000000060, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"insertion":{"key":"i_one","segment":{"route":"seg","version":1}}}}}}]');
INSERT INTO "proposals" VALUES ('pr_ins', 'journey', 'j_one', 1, 'open', 'Insert', NULL, 1, '[{"op":"insert_segment","insertion":"i_two","segment":{"route":"seg","version":1},"edges":[{"node":{"segment":"n_seg"},"requires":{"host":"n_kick"}}]}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn column<T: rusqlite::types::FromSql>(connection: &rusqlite::Connection, sql: &str) -> Vec<T> {
        connection
            .prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn a_segment_becomes_a_route_and_its_insertion_a_pointer() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("one-route-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(OLD).unwrap();
        drop(connection);
        drop(SqliteStore::open(&path).unwrap());

        let connection = rusqlite::Connection::open(&path).unwrap();
        let tables: Vec<String> = column(
            &connection,
            "SELECT name FROM sqlite_master WHERE type = 'table'",
        );
        assert!(!tables.contains(&"insertions".to_owned()));
        let routes: Vec<String> =
            column(&connection, "SELECT name FROM pragma_table_info('routes')");
        assert!(!routes.contains(&"kind".to_owned()));
        assert_eq!(
            column::<String>(&connection, "SELECT id FROM routes WHERE id = 'seg'"),
            ["seg"]
        );
        let pointed: Vec<String> = column(
            &connection,
            "SELECT key || '=' || from_route || '/' || from_version FROM nodes \
             WHERE from_route IS NOT NULL",
        );
        assert_eq!(pointed, ["n_ins=seg/1"]);
        let event: (String, String, String) = connection
            .query_row(
                "SELECT event_type, subject, delta FROM events WHERE seq = 60",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(event.0, "route_inserted");
        assert_eq!(event.1, r#"{"journey":"j_one"}"#);
        assert_eq!(event.2, "[]");
        let mutations: String = connection
            .query_row(
                "SELECT mutations FROM proposals WHERE id = 'pr_ins'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mutations: serde_json::Value = serde_json::from_str(&mutations).unwrap();
        assert_eq!(
            mutations,
            serde_json::json!([{"op": "insert_route", "insertion": "i_two",
                "route": {"route": "seg", "version": 1},
                "edges": [{"node": {"route": "n_seg"}, "requires": {"host": "n_kick"}}]}])
        );
    }
}
