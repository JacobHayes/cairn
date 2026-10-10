//! Property tests: any graph a journey can hold, written as records, loads back
//! equal from the Turso backend and from the memory backend (A14: every field has a home).

#[cfg(test)]
mod property {
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU32, Ordering};

    use cairn_schema::testing::arb_graph;
    use cairn_schema::{EventType, Graph, Record, Subject, Write};
    use cairn_store::build::{graph_writes, id, journey_graph, journey_header, journey_patch};
    use cairn_store::conformance::journey;
    use cairn_store::{Commit, MemoryStore, Store};
    use cairn_store_turso::TursoStore;
    use patina_dst_proptest::prelude::*;
    use tokio::runtime::Runtime;

    /// One runtime and one Turso store for every case, each case in a journey of its own.
    struct Shared {
        runtime: Runtime,
        turso: TursoStore,
        next: AtomicU32,
    }

    fn shared() -> &'static Shared {
        static SHARED: OnceLock<Shared> = OnceLock::new();
        SHARED.get_or_init(|| {
            let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
                .join(format!("property-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            let turso = runtime
                .block_on(TursoStore::open(&root.join("store.db")))
                .unwrap();
            Shared {
                runtime,
                turso,
                next: AtomicU32::new(0),
            }
        })
    }

    /// Whether no two nodes share a parent and an id: the commit rejects such a graph.
    fn siblings_unique(graph: &Graph) -> bool {
        let mut seen = BTreeSet::new();
        graph
            .nodes
            .values()
            .all(|node| seen.insert((node.parent.clone(), node.id.clone())))
    }

    fn create(journey: &str, graph: &Graph) -> Commit {
        let mut writes = vec![Write::Put(Record::JourneyHeader(journey_header(
            journey, "Any", None,
        )))];
        writes.extend(graph_writes(&journey_graph(journey), graph));
        journey_patch(&format!("p_{journey}"), journey, 0)
            .event(
                EventType::JourneyCreated,
                Subject::Journey(id(journey)),
                writes,
            )
            .commit()
    }

    async fn loaded<S: Store>(store: &S, journey_id: &str, graph: &Graph) -> Graph {
        store.commit(create(journey_id, graph)).await.unwrap();
        journey(store, journey_id).await.unwrap().graph
    }

    proptest! {
        #[test]
        fn any_graph_loads_back_equal_from_both_backends(graph in arb_graph()) {
            prop_assume!(siblings_unique(&graph));
            let shared = shared();
            let journey_id = format!("j_case{}", shared.next.fetch_add(1, Ordering::Relaxed));
            let (from_turso, from_memory) = shared.runtime.block_on(async {
                let from_turso = loaded(&shared.turso, &journey_id, &graph).await;
                let from_memory = loaded(&MemoryStore::new(), &journey_id, &graph).await;
                (from_turso, from_memory)
            });
            prop_assert_eq!(&from_turso, &graph);
            prop_assert_eq!(&from_memory, &graph);
        }
    }
}
