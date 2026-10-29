//! Migration 0019 on a database frozen at migration 13 (see `at_migration_13.sql`), with a
//! skipped node written in that code's words: the reason of a skip was `skip_reason`, and is
//! `reason` now that a group marked done keeps one too. The expected state is written here:
//! which positions are renamed, and which text a person chose stays as written.

#[cfg(test)]
mod reason {
    use std::path::PathBuf;

    use cairn_schema::{State, Write};
    use cairn_store::{Document, EventQuery, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;

    const DUMP: &str = include_str!("at_migration_13.sql");

    /// A skipped node in the old words: its state row, the event that skipped it, and a
    /// reviewer's map between choices that spells a node state's keys.
    const SKIPPED: &str = r#"
UPDATE "node_states" SET state = 'skipped', skip_reason = 'Not needed.' WHERE node = 'n_reach';
INSERT INTO "events" VALUES (20, 'p_skip', 0, 'journey', 'j_one', 'node_transitioned', 'u_lead', NULL, NULL, '{"node":"n_reach"}', 1791288000000000020, NULL, '[{"put":{"graph":{"graph":{"journey":"j_one"},"record":{"node_state":{"node":"n_reach","state":{"state":"skipped","provenance":"local","skip_reason":"Not needed."}}}}}}]');
"#;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// The store opened on the frozen dump with the skipped node, which migrates it.
    fn migrated(name: &str) -> (PathBuf, SqliteStore) {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("reason-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(DUMP).unwrap();
        connection.execute_batch(SKIPPED).unwrap();
        drop(connection);
        let store = SqliteStore::open(&path).unwrap();
        (path, store)
    }

    #[test]
    fn a_skip_keeps_its_reason_under_the_new_name_everywhere_it_is_stored() {
        run(async {
            let (_, store) = migrated("node");
            let Some(Document::Journey(journey)) = store
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap()
            else {
                panic!("no journey")
            };
            let stored = &journey.graph.state.nodes[&"n_reach".parse().unwrap()];
            assert_eq!(stored.state, State::Skipped);
            assert_eq!(
                stored.reason.as_ref().map(cairn_schema::Reason::as_str),
                Some("Not needed.")
            );

            let query = EventQuery {
                log: Some(cairn_schema::Domain::Journey("j_one".parse().unwrap())),
                ..EventQuery::default()
            };
            let logged = store.events(&query).await.unwrap().items;
            let skipped = logged
                .iter()
                .flat_map(|entry| entry.event.delta.iter())
                .find_map(|write| match write {
                    Write::Put(cairn_schema::Record::Graph { record, .. }) => match record {
                        cairn_schema::GraphRecord::NodeState { node, state }
                            if node.as_str() == "n_reach" && state.state == State::Skipped =>
                        {
                            Some(state.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                })
                .expect("the skip's event reads back");
            assert_eq!(
                skipped.reason.as_ref().map(cairn_schema::Reason::as_str),
                Some("Not needed.")
            );
        });
    }
}
