//! The last migration on a database frozen at migration 13 (see `at_migration_13.sql`), with
//! a guard bypass written the old way: the event that applied it, and a pending proposal
//! that bypasses a guard beside a completion. The event still reads, as an applied override
//! holding the same bypass; the proposal's completion goes ahead anyway with the same reason.

#[cfg(test)]
mod complete_anyway {
    use std::path::PathBuf;

    use cairn_schema::EventType;
    use cairn_store::{EventQuery, Store};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    const BYPASS: &str = r#"
INSERT INTO "overrides" VALUES ('journey/j_one', 'n_work', NULL, NULL, '{"guards":["has_note"],"reason":"Checked.","failures":["missing_note"]}');
INSERT INTO "events" VALUES (40, 'p_work', 0, 'journey', 'j_one', 'guard_bypassed', 'u_lead', NULL, NULL, '{"node":"n_work"}', 1791288000000000040, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"overrides":{"node":"n_work","overrides":{"bypass":{"guards":["has_note"],"reason":"Checked.","failures":["missing_note"]}}}}}}}]');
INSERT INTO "proposals" VALUES ('pr_ahead', 'journey', 'j_one', 1, 'open', 'Ahead', NULL, 1, '[{"op":"apply_override","node":"n_work","override":{"guard_bypass":{"guards":["has_note"],"reason":"Checked."}}},{"op":"transition","node":"n_work","transition":"complete"}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    #[test]
    fn a_stored_bypass_reads_in_the_new_shape() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("complete-anyway-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(BYPASS).unwrap();
        drop(connection);
        let store = SqliteStore::open(&path).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                let query = EventQuery {
                    log: Some(cairn_schema::Domain::Journey("j_one".parse().unwrap())),
                    ..EventQuery::default()
                };
                let events = store.events(&query).await.unwrap().items;
                let applied: Vec<_> = events
                    .iter()
                    .filter(|entry| {
                        entry.event.subject
                            == cairn_schema::Subject::Node("n_work".parse().unwrap())
                    })
                    .map(|entry| entry.event.event_type)
                    .collect();
                assert!(applied.contains(&EventType::OverrideApplied));
                let Some(cairn_store::Document::Journey(journey)) = store
                    .load(&cairn_store::LoadTarget::Journey(cairn_store::build::id(
                        "j_one",
                    )))
                    .await
                    .unwrap()
                else {
                    panic!("no journey")
                };
                let bypass = journey.graph.state.overrides[&"n_work".parse().unwrap()]
                    .bypass
                    .clone()
                    .unwrap();
                assert_eq!(
                    bypass.failures,
                    std::collections::BTreeSet::from([cairn_schema::GuardFailure::MissingNote])
                );
            });
        drop(store);
        let connection = rusqlite::Connection::open(&path).unwrap();
        let mutations: String = connection
            .query_row(
                "SELECT mutations FROM proposals WHERE id = 'pr_ahead'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&mutations).unwrap(),
            serde_json::json!([{"op": "transition", "node": "n_work",
                "transition": {"complete_anyway": {"reason": "Checked."}}}])
        );
    }
}
