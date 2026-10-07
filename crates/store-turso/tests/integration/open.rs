//! Opening a store: the migrations apply once, and committed data survives a reopen.

#[cfg(test)]
mod open {
    use std::path::PathBuf;

    use cairn_store::build::{action, create_journey};
    use cairn_store::{Document, LoadTarget, Store};
    use cairn_store_turso::TursoStore;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn a_reopened_store_keeps_its_data_and_its_schema() {
        let root =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("open-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = TursoStore::open(&path).await.unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            drop(store);
            let reopened = TursoStore::open(&path).await.unwrap();
            let loaded = reopened
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap();
            let Some(Document::Journey(journey)) = loaded else {
                panic!("the journey survives a reopen: {loaded:?}");
            };
            assert_eq!(journey.graph.nodes.len(), 1);
            let next = create_journey("p_two", "j_two", Vec::new()).commit();
            reopened.commit(next).await.unwrap();
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
