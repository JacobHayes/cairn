//! Migration 0016 on a database frozen at migration 13 (`at_migration_13.sql`) with a journey
//! hard-deleted the way the code before it did: the journey's own patch, its deleted id in
//! `deleted_journeys`, the record and its address in the old JSON shapes, an open proposal
//! deleting its journey. The expected state is written here, not derived from the current types.

#[cfg(test)]
mod deleted_ids {
    use std::path::PathBuf;

    use cairn_schema::{DeletedId, Mutation, Record, RecordKey, Write};
    use cairn_store::{EventQuery, Store};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    /// A journey deleted before the migration, in the shapes the code then wrote.
    const DELETED: &str = r#"
INSERT INTO "deleted_journeys" VALUES ('j_old', 1791288000000000009);
INSERT INTO "events" VALUES (9, 'p_gone', 0, 'deployment', NULL, 'journey_deleted', 'u_lead', NULL, NULL, '{"journey":"j_old"}', 1791288000000000009, NULL, '[{"remove":{"domain":{"journey":"j_old"}}},{"put":{"deleted_journey":{"journey":"j_old","deleted_at":"2026-10-06T12:00:00Z"}}}]');
INSERT INTO "patch_receipts" VALUES ('p_gone', 'journey', 'j_old', NULL, 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 4, NULL, '{}', '[{"deleted_journey":"j_old"}]');
INSERT INTO "proposals" VALUES ('pr_gone', 'journey', 'j_one', 1, 'open', 'Delete it', NULL, 1, '[{"op":"delete_journey"}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// The store opened on the frozen dump with the deletion, which migrates it.
    fn migrated() -> (PathBuf, SqliteStore) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("deleted-ids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(DELETED).unwrap();
        drop(connection);
        let store = SqliteStore::open(&path).unwrap();
        (path, store)
    }

    #[test]
    fn a_deleted_journey_reads_back_in_the_new_shapes() {
        run(async {
            let (path, store) = migrated();
            let gone = DeletedId::Journey("j_old".parse().unwrap());
            let query = EventQuery {
                log: Some(cairn_schema::Domain::Deployment),
                ..EventQuery::default()
            };
            let logged = store.events(&query).await.unwrap().items;
            let [entry] = logged.as_slice() else {
                panic!("{logged:#?}")
            };
            assert!(
                entry.event.delta.iter().any(|write| matches!(
                    write,
                    Write::Put(Record::Deleted { id, .. }) if *id == gone
                )),
                "{:?}",
                entry.event.delta
            );
            let proposal = store
                .proposal(&"pr_gone".parse().unwrap())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                proposal.draft.mutations.as_slice(),
                [Mutation::DeleteJourney {
                    journey: "j_one".parse().unwrap(),
                    revision: cairn_schema::Revision::NONE.next(),
                }]
            );
            drop(store);
            let connection = rusqlite::Connection::open(&path).unwrap();
            let (kind, id): (String, String) = connection
                .query_row("SELECT kind, id FROM deleted_ids", (), |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap();
            assert_eq!((kind.as_str(), id.as_str()), ("journey", "j_old"));
            let touched: String = connection
                .query_row(
                    "SELECT deployment_touched FROM patch_receipts WHERE patch_id = 'p_gone'",
                    (),
                    |row| row.get(0),
                )
                .unwrap();
            let touched: Vec<RecordKey> = serde_json::from_str(&touched).unwrap();
            assert_eq!(touched, [RecordKey::Deleted(gone)]);
        });
    }
}
