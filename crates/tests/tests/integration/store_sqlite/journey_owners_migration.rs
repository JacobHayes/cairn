//! Migration 0018 on a database that already holds a journey, its create event and an open
//! proposal whose mutations create and edit a journey, all written before journeys had owners:
//! it opens at the new version, reads them back with no owners, and keeps owners written
//! afterwards across a reopen.

#[cfg(test)]
mod journey_owners {
    use std::path::PathBuf;

    use cairn_schema::{Domain, EventType, Mutation, Record, Subject, Write};
    use cairn_store::build::{self, create_journey, id, journey_patch};
    use cairn_store::{Document, EventQuery, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;

    /// An open proposal as the code before owners stored it.
    const PROPOSAL: &str = r#"
INSERT INTO "proposals" VALUES ('pr_old', 'journey', 'j_one', 1, 'open', 'Old', NULL, 1, '[{"op":"create_journey","name":"Made"},{"op":"edit_journey","name":"Renamed"}]', '[]', NULL, 'u_lead', 1791288000000000000);
"#;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    async fn owners(store: &SqliteStore) -> Vec<String> {
        let loaded = store.load(&LoadTarget::Journey(id("j_one"))).await.unwrap();
        let Some(Document::Journey(journey)) = loaded else {
            panic!("the journey survives: {loaded:?}");
        };
        journey
            .header
            .owners
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn journeys_events_and_proposals_from_before_owners_read_back_and_owners_persist() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("journey-owners-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            store
                .commit(create_journey("p_one", "j_one", Vec::new()).commit())
                .await
                .unwrap();
            drop(store);

            // The database as it stood at migration 17.
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection.execute_batch(PROPOSAL).unwrap();
            connection
                .execute_batch(
                    "DROP TABLE journey_owners; DELETE FROM schema_migrations WHERE version = 18;",
                )
                .unwrap();
            drop(connection);

            let store = SqliteStore::open(&path).unwrap();
            assert_eq!(owners(&store).await, [] as [std::string::String; 0]);
            let proposal = store.proposal(&id("pr_old")).await.unwrap().unwrap();
            let [
                Mutation::CreateJourney { owners: made, .. },
                Mutation::EditJourney { owners: edited, .. },
            ] = proposal.draft.mutations.as_slice()
            else {
                panic!("{:?}", proposal.draft.mutations)
            };
            assert!(made.is_empty() && edited.is_none());
            let query = EventQuery {
                log: Some(Domain::Journey(id("j_one"))),
                ..EventQuery::default()
            };
            let logged = store.events(&query).await.unwrap().items;
            let header = logged
                .iter()
                .flat_map(|entry| entry.event.delta.iter())
                .find_map(|write| match write {
                    Write::Put(Record::JourneyHeader(header)) => Some(header),
                    _ => None,
                })
                .unwrap();
            assert!(header.owners.is_empty());

            let edit = journey_patch("p_owned", "j_one", 1)
                .event(
                    EventType::JourneyEdited,
                    Subject::Journey(id("j_one")),
                    vec![Write::Put(Record::JourneyHeader(build::owned_header(
                        "j_one",
                        &["who_a", "who_b"],
                    )))],
                )
                .commit();
            store.commit(edit).await.unwrap();
            drop(store);
            let reopened = SqliteStore::open(&path).unwrap();
            assert_eq!(owners(&reopened).await, ["who_a", "who_b"]);
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
