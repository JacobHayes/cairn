//! The tour record's table (migration 17): a database from before it opens and gains the
//! table with its users and journeys untouched, and a record written afterwards survives a
//! reopen, including its practice journey naming a journey that no longer exists.

#[cfg(test)]
mod tour_migration {
    use std::path::PathBuf;

    use cairn_store::build::{action, create_journey, id};
    use cairn_store::{AuthStore, Store, TourRecord, TourState, UserRecord};
    use cairn_store_sqlite::SqliteStore;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn a_database_from_before_the_table_gains_it_and_keeps_its_rows() {
        let root =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("tour-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        let at = cairn_store::build::at(5);
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            store
                .put_user(UserRecord {
                    id: id("u_ann"),
                    created_at: at,
                })
                .await
                .unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
        });
        // Put the file back as migration 16 left it.
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch("DROP TABLE tour; DELETE FROM schema_migrations WHERE version = 17;")
            .unwrap();
        drop(connection);
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            assert!(store.user(&id("u_ann")).await.unwrap().is_some());
            assert_eq!(store.tour(&id("u_ann")).await.unwrap(), None);
            let record = TourRecord {
                user: id("u_ann"),
                state: TourState::Started,
                practice_journey: Some(id("j_gone")),
                updated_at: at,
            };
            assert_eq!(store.put_tour(record.clone(), None).await.unwrap(), Ok(()));
            drop(store);
            let reopened = SqliteStore::open(&path).unwrap();
            assert_eq!(reopened.tour(&id("u_ann")).await.unwrap(), Some(record));
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
