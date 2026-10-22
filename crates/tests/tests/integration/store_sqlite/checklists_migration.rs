//! Migration 0020 on a database from before checklists: it opens, reads its actions back with
//! no checklist, and a checklist and a tick written afterwards survive a reopen.

#[cfg(test)]
mod checklists {
    use std::path::{Path, PathBuf};

    use cairn_schema::{EventType, GraphKey, GraphRecord, NodeFieldValue, Subject};
    use cairn_store::build::{
        self, action, create_journey, id, journey_graph, journey_patch, node_key, put_in, remove_in,
    };
    use cairn_store::{Document, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// The journey's graph as the store loads it.
    async fn graph(store: &SqliteStore) -> cairn_schema::Graph {
        let loaded = store.load(&LoadTarget::Journey(id("j_one"))).await.unwrap();
        let Some(Document::Journey(journey)) = loaded else {
            panic!("the journey survives: {loaded:?}");
        };
        journey.graph
    }

    /// The database as it stood at migration 19.
    fn downgrade(path: &Path) {
        let connection = rusqlite::Connection::open(path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE ticks;\
                 ALTER TABLE nodes DROP COLUMN checklist;\
                 ALTER TABLE nodes DROP COLUMN requires_checklist;\
                 DELETE FROM schema_migrations WHERE version = 20;",
            )
            .unwrap();
    }

    /// A patch giving the action a checklist of two items, both ticked, then one unticked.
    fn checklist_with_ticks() -> cairn_store::Commit {
        let list: NodeFieldValue<cairn_schema::KeyRefs> =
            serde_json::from_value(serde_json::json!({"checklist": [
                {"id": "load", "title": "Load test"}, {"id": "docs", "title": "Docs"}
            ]}))
            .unwrap();
        let graph_id = journey_graph("j_one");
        let write = |item: &str| {
            put_in(
                &graph_id,
                GraphRecord::Tick {
                    node: node_key("n_a"),
                    item: build::id(item),
                },
            )
        };
        journey_patch("p_checklist", "j_one", 1)
            .event(
                EventType::NodeChanged,
                Subject::Node(node_key("n_a")),
                vec![
                    put_in(
                        &graph_id,
                        GraphRecord::NodeField {
                            node: node_key("n_a"),
                            value: list,
                        },
                    ),
                    write("load"),
                    write("docs"),
                ],
            )
            .event(
                EventType::ItemUnticked,
                Subject::Node(node_key("n_a")),
                vec![remove_in(
                    &graph_id,
                    GraphKey::Tick {
                        node: node_key("n_a"),
                        item: build::id("docs"),
                    },
                )],
            )
            .commit()
    }

    #[test]
    fn actions_from_before_checklists_read_back_and_a_checklist_with_ticks_persists() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("checklists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            drop(store);

            downgrade(&path);

            let store = SqliteStore::open(&path).unwrap();
            let before = graph(&store).await;
            let node = before.nodes.get(&node_key("n_a")).unwrap();
            assert!(
                node.payload
                    .checklist()
                    .is_none_or(cairn_schema::Checklist::is_empty)
            );
            assert!(!node.payload.requires_checklist());
            assert!(before.state.ticks.is_empty());

            let edit = checklist_with_ticks();
            store.commit(edit).await.unwrap();
            drop(store);

            let reopened = SqliteStore::open(&path).unwrap();
            let after = graph(&reopened).await;
            let node = after.nodes.get(&node_key("n_a")).unwrap();
            let items: Vec<_> = node
                .payload
                .checklist()
                .unwrap()
                .ids()
                .map(ToString::to_string)
                .collect();
            assert_eq!(items, ["load", "docs"]);
            let ticked: Vec<_> = after.state.ticks[&node_key("n_a")]
                .iter()
                .map(ToString::to_string)
                .collect();
            assert_eq!(ticked, ["load"]);
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
