//! Migration 0022 on a database from before finishes and reopens kept their revisions: a
//! finish, a second finish, a reopen of the first and an edit of the second's actual date,
//! one patch each, written as they were then (days only) and migrated. Each takes the revision
//! of the patch that first wrote it, in the table and in the events, so the log agrees with
//! the table.

#[cfg(test)]
mod finish_revisions {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use cairn_schema::{
        EventType, GraphRecord, JourneyState, NodeState, Record, Reopened, Revision, State,
        Subject, Write,
    };
    use cairn_store::build::{action, create_journey, id, journey_graph, journey_patch, put_in};
    use cairn_store::{Document, EventQuery, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;
    use serde_json::Value;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn revision(at: u32) -> Revision {
        Revision::try_from(at).unwrap()
    }

    fn date(text: &str) -> cairn_schema::Date {
        text.parse().unwrap()
    }

    /// A patch at `base` writing `node`'s state.
    fn state(patch: &str, base: u32, node: &str, state: NodeState) -> cairn_store::Commit {
        journey_patch(patch, "j_one", base)
            .event(
                EventType::NodeTransitioned,
                Subject::Node(node.parse().unwrap()),
                vec![put_in(
                    &journey_graph("j_one"),
                    GraphRecord::NodeState {
                        node: node.parse().unwrap(),
                        state,
                    },
                )],
            )
            .commit()
    }

    fn done(on: &str, at: u32) -> NodeState {
        NodeState {
            state: State::Done,
            finished_on: Some(date(on)),
            finished_revision: Some(revision(at)),
            ..NodeState::initial(cairn_schema::NodeKind::Action)
        }
    }

    async fn journey_state(store: &SqliteStore) -> JourneyState {
        let loaded = store.load(&LoadTarget::Journey(id("j_one"))).await.unwrap();
        let Some(Document::Journey(journey)) = loaded else {
            panic!("the journey survives: {loaded:?}");
        };
        journey.graph.state
    }

    /// Each node state as it stood before revisions: no finish revision, and a reopen by its
    /// days.
    fn without_revisions(value: &mut Value) {
        match value {
            Value::Object(object) => {
                if let Some(Value::Object(state)) = object.get_mut("state") {
                    state.remove("finished_revision");
                    if state.contains_key("reopened") {
                        state.insert(
                            "reopened".to_owned(),
                            serde_json::json!({"on": "2026-10-08", "finished_on": "2026-10-07"}),
                        );
                    }
                }
                object.values_mut().for_each(without_revisions);
            }
            Value::Array(items) => items.iter_mut().for_each(without_revisions),
            _ => {}
        }
    }

    /// The database as it stood at migration 21.
    fn downgrade(path: &Path) {
        let connection = rusqlite::Connection::open(path).unwrap();
        let deltas: Vec<(i64, String)> = connection
            .prepare("SELECT seq, delta FROM events")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (seq, delta) in deltas {
            let mut value: Value = serde_json::from_str(&delta).unwrap();
            without_revisions(&mut value);
            connection
                .execute(
                    "UPDATE events SET delta = ?1 WHERE seq = ?2",
                    (value.to_string(), seq),
                )
                .unwrap();
        }
        connection
            .execute_batch(
                "ALTER TABLE node_states ADD COLUMN reopened_on TEXT;\
                 ALTER TABLE node_states ADD COLUMN reopened_finished_on TEXT \
                   CHECK ((reopened_finished_on IS NULL) = (reopened_on IS NULL));\
                 UPDATE node_states SET reopened_on = '2026-10-08', \
                   reopened_finished_on = '2026-10-07' WHERE reopened_revision IS NOT NULL;\
                 ALTER TABLE node_states DROP COLUMN finished_revision;\
                 ALTER TABLE node_states DROP COLUMN reopened_revision;\
                 ALTER TABLE node_states DROP COLUMN reopened_finished_revision;\
                 DELETE FROM schema_migrations WHERE version = 22;",
            )
            .unwrap();
    }

    #[test]
    fn each_finish_and_reopen_takes_the_revision_of_the_patch_that_first_wrote_it() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("finish-revisions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            let nodes = vec![action("n_a", "a", None), action("n_b", "b", None)];
            store
                .commit(create_journey("p_one", "j_one", nodes).commit())
                .await
                .unwrap();
            let reopened = NodeState {
                reopened: Some(Reopened {
                    revision: revision(4),
                    finished_revision: revision(2),
                }),
                ..NodeState::initial(cairn_schema::NodeKind::Action)
            };
            for commit in [
                state("p_a", 1, "n_a", done("2026-10-07", 2)),
                state("p_b", 2, "n_b", done("2026-10-07", 3)),
                state("p_reopen", 3, "n_a", reopened),
                state("p_edit", 4, "n_b", done("2026-10-05", 3)),
            ] {
                store.commit(commit).await.unwrap();
            }
            let recorded = journey_state(&store).await;
            drop(store);

            downgrade(&path);

            let store = SqliteStore::open(&path).unwrap();
            let migrated = journey_state(&store).await;
            assert_eq!(migrated.nodes, recorded.nodes);
            // The last node state write per node, in log order, is what the table holds.
            let query = EventQuery {
                log: Some(cairn_schema::Domain::Journey(id("j_one"))),
                ..EventQuery::default()
            };
            let mut replayed = BTreeMap::new();
            for entry in store.events(&query).await.unwrap().items {
                for write in entry.event.delta {
                    if let Write::Put(Record::Graph {
                        record: GraphRecord::NodeState { node, state },
                        ..
                    }) = write
                    {
                        replayed.insert(node, state);
                    }
                }
            }
            assert_eq!(replayed, migrated.nodes);
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
