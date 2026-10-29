//! The last migration on a database frozen at migration 13 (see `at_migration_13.sql`), with
//! what a journey used to keep of its route besides a pointer: local-edit markers, tombstones,
//! each node's provenance, an upgrade in the log, and a proposal that upgrades. The markers,
//! tombstones and provenance are gone, the upgrade reads as a journey edit, the events that
//! only recorded a marker are gone, and the proposal that can no longer be applied goes with
//! them while one that still can is kept.

#[cfg(test)]
mod pointer {
    use std::path::PathBuf;

    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const OLD: &str = r#"
INSERT INTO "tombstones" VALUES ('journey/j_one', 'n_gone');
INSERT INTO "events" VALUES (50, 'p_up', 0, 'journey', 'j_one', 'journey_upgraded', 'u_lead', NULL, NULL, '{}', 1791288000000000050, NULL, '[]');
INSERT INTO "events" VALUES (51, 'p_mark', 0, 'journey', 'j_one', 'local_edit_changed', 'u_lead', NULL, NULL, '{"node":"n_gate"}', 1791288000000000051, NULL, '[]');
INSERT INTO "event_nodes" VALUES (51, 'n_gate');
INSERT INTO "events" VALUES (52, 'p_pr', 0, 'journey', 'j_one', 'proposal_created', 'u_lead', NULL, NULL, '{}', 1791288000000000052, NULL, '[{"put":{"proposal":{"id":"pr_upgrade","destination":{"journey":"j_one"},"revision":1,"status":"open","draft":{"title":"Upgrade","destination_revision":1,"mutations":[{"op":"upgrade","to":2}]},"created_by":"u_lead","created_at":"2026-10-05T10:00:00Z"}}}]');
INSERT INTO "proposals" VALUES ('pr_upgrade', 'journey', 'j_one', 1, 'open', 'Upgrade', NULL, 1, '[{"op":"upgrade","to":2}]', '[{"item":"orphan","node":"n_work","keep":true,"removal":{"node":"n_work"}}]', NULL, 'u_lead', 1791288000000000000);
"#;

    #[test]
    fn a_journey_keeps_only_its_pointer() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("pointer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(OLD).unwrap();
        drop(connection);
        drop(SqliteStore::open(&path).unwrap());

        let connection = rusqlite::Connection::open(&path).unwrap();
        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!tables.contains(&"local_edits".to_owned()));
        assert!(!tables.contains(&"tombstones".to_owned()));
        let columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('node_states')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!columns.contains(&"provenance".to_owned()));
        let event_type = |seq: i64| -> Option<String> {
            connection
                .query_row(
                    "SELECT event_type FROM events WHERE seq = ?1",
                    [seq],
                    |row| row.get(0),
                )
                .ok()
        };
        assert_eq!(event_type(50).as_deref(), Some("journey_edited"));
        assert_eq!(event_type(51), None);
        let delta: String = connection
            .query_row("SELECT delta FROM events WHERE seq = 52", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!delta.contains("upgrade"), "{delta}");
        let proposals: Vec<String> = connection
            .prepare("SELECT id FROM proposals ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(proposals, ["pr_one", "pr_two"]);
    }
}
