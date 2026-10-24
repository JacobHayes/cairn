//! Migration 0021 on a database frozen at migration 13 (see `at_migration_13.sql`), with
//! bypasses written while order was a guard: one that also bypassed a missing note, and one of
//! order alone. The evidence part survives everywhere it is stored; the order-only bypass goes
//! from the table, from the event that wrote it (its write becomes the record's removal, so
//! the log agrees with the table) and from the proposal that would send it.

#[cfg(test)]
mod evidence_guards {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use cairn_schema::{GraphKey, GraphRecord, Guard, GuardFailure, Record, RecordKey, Write};
    use cairn_store::{Document, EventQuery, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    /// Two stored bypasses, the events that wrote them, and a proposal that starts work
    /// with an order bypass.
    const BYPASSES: &str = r#"
INSERT INTO "overrides" VALUES ('journey/j_one', 'n_work', NULL, NULL, '{"guards":["deps_done","has_note"],"reason":"Checked.","failures":[{"open_dependency":"n_gate"},"missing_note"]}');
INSERT INTO "overrides" VALUES ('journey/j_one', 'n_reach', NULL, NULL, '{"guards":["deps_done"],"reason":"Ahead.","failures":[{"open_dependency":"n_gate"}]}');
INSERT INTO "events" VALUES (40, 'p_work', 0, 'journey', 'j_one', 'override_applied', 'u_lead', NULL, NULL, '{"node":"n_work"}', 1791288000000000040, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"overrides":{"node":"n_work","overrides":{"bypass":{"guards":["deps_done","has_note"],"reason":"Checked.","failures":[{"open_dependency":"n_gate"},"missing_note"]}}}}}}}]');
INSERT INTO "events" VALUES (41, 'p_reach', 0, 'journey', 'j_one', 'override_applied', 'u_lead', NULL, NULL, '{"node":"n_reach"}', 1791288000000000041, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"overrides":{"node":"n_reach","overrides":{"bypass":{"guards":["deps_done"],"reason":"Ahead.","failures":[{"open_dependency":"n_gate"}]}}}}}}}]');
INSERT INTO "proposals" VALUES ('pr_ahead', 'journey', 'j_one', 1, 'open', 'Ahead', NULL, 1, '[{"op":"apply_override","node":"n_reach","override":{"guard_bypass":{"guards":["deps_done"],"reason":"Ahead."}}},{"op":"transition","node":"n_reach","transition":"start"}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// The store opened on the frozen dump with the bypasses, which migrates it.
    fn migrated(name: &str) -> (PathBuf, SqliteStore) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("evidence-guards-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(BYPASSES).unwrap();
        drop(connection);
        let store = SqliteStore::open(&path).unwrap();
        (path, store)
    }

    fn text(path: &Path, sql: &str) -> Vec<String> {
        let connection = rusqlite::Connection::open(path).unwrap();
        let mut statement = connection.prepare(sql).unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn a_bypass_keeps_its_evidence_and_the_log_agrees_with_the_table() {
        run(async {
            let (_, store) = migrated("table");
            let Some(Document::Journey(journey)) = store
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap()
            else {
                panic!("no journey")
            };
            let overrides = &journey.graph.state.overrides;
            let work = overrides[&"n_work".parse().unwrap()]
                .bypass
                .clone()
                .unwrap();
            assert_eq!(work.guards, BTreeSet::from([Guard::HasNote]));
            assert_eq!(work.failures, BTreeSet::from([GuardFailure::MissingNote]));
            assert_eq!(
                work.reason.as_ref().map(cairn_schema::Reason::as_str),
                Some("Checked.")
            );
            assert!(!overrides.contains_key(&"n_reach".parse().unwrap()));

            // The last override write per node, in log order, is what the table holds.
            let query = EventQuery {
                log: Some(cairn_schema::Domain::Journey("j_one".parse().unwrap())),
                ..EventQuery::default()
            };
            let mut replayed = std::collections::BTreeMap::new();
            for entry in store.events(&query).await.unwrap().items {
                for write in &entry.event.delta {
                    match write {
                        Write::Put(Record::Graph {
                            record: GraphRecord::Overrides { node, overrides },
                            ..
                        }) => {
                            replayed.insert(node.clone(), Some(overrides.clone()));
                        }
                        Write::Remove(RecordKey::InGraph {
                            key: GraphKey::Overrides(node),
                            ..
                        }) => {
                            replayed.insert(node.clone(), None);
                        }
                        _ => {}
                    }
                }
            }
            let replayed: std::collections::BTreeMap<_, _> = replayed
                .into_iter()
                .filter_map(|(node, overrides)| overrides.map(|kept| (node, kept)))
                .collect();
            assert_eq!(&replayed, overrides);
        });
    }

    #[test]
    fn a_proposal_drops_an_order_bypass_and_keeps_its_move() {
        run(async {
            let (path, store) = migrated("proposal");
            drop(store);
            let mutations: serde_json::Value = serde_json::from_str(
                &text(
                    &path,
                    "SELECT mutations FROM proposals WHERE id = 'pr_ahead'",
                )[0],
            )
            .unwrap();
            assert_eq!(
                mutations,
                serde_json::json!([{"op": "transition", "node": "n_reach", "transition": "start"}])
            );
            let before = text(&path, "SELECT delta FROM events ORDER BY seq");
            drop(SqliteStore::open(&path).unwrap());
            assert_eq!(text(&path, "SELECT delta FROM events ORDER BY seq"), before);
        });
    }
}
