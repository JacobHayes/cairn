//! Opening a store: the migrations apply once, committed data survives a reopen, a database
//! converted from the Turso store opens, and the migrated schema names the glossary's stored
//! concepts (A14).

#[cfg(test)]
mod open {
    use std::path::PathBuf;

    use cairn_store::build::{action, create_journey};
    use cairn_store::{Document, LoadTarget, Store};
    use cairn_store_sqlite::SqliteStore;

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
            let store = SqliteStore::open(&path).unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            drop(store);
            let reopened = SqliteStore::open(&path).unwrap();
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

    /// The write-ahead log's index lives in the process's memory, never in a memory-mapped
    /// `-shm` file, which the durability simulation truncates under a short injected read (a
    /// `SIGBUS` mid-commit): after commits and reads beside each other, no such file exists.
    #[test]
    fn the_log_index_is_never_a_mapped_file() {
        let root =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("shm-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = SqliteStore::open(&path).unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            let loaded = store
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap();
            assert!(loaded.is_some());
            assert!(root.join("store.db-wal").exists());
            assert!(!root.join("store.db-shm").exists());
        });
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A database a release on the Turso store left, converted to SQLite's WAL mode, opens
    /// with its data once it records migrations 1 to 13; one that stopped short of 13 is
    /// refused, since only the Turso store could bring it there. Each is the current schema
    /// with the Turso store's migrations recorded as it left them; the migrations after 13
    /// over the schema at 13 are `milestone_done_migration`'s.
    #[test]
    fn a_converted_database_opens_only_from_migration_13_on() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("converted-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let record = |path: &std::path::Path, statement: &str| {
            let connection = rusqlite::Connection::open(path).unwrap();
            connection.execute_batch(statement).unwrap();
        };
        let earlier: Vec<String> = (1..13).map(|number| format!("({number})")).collect();
        let earlier = format!(
            "INSERT INTO schema_migrations (version) VALUES {};",
            earlier.join(", ")
        );
        run(async {
            let current = root.join("current.db");
            let store = SqliteStore::open(&current).unwrap();
            let create = create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            drop(store);
            record(&current, &earlier);
            let reopened = SqliteStore::open(&current).unwrap();
            let loaded = reopened
                .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
                .await
                .unwrap();
            assert!(matches!(loaded, Some(Document::Journey(_))), "{loaded:?}");
        });
        let short = root.join("short.db");
        drop(SqliteStore::open(&short).unwrap());
        record(
            &short,
            &format!("DELETE FROM schema_migrations WHERE version >= 13; {earlier}"),
        );
        let refused = SqliteStore::open(&short).unwrap_err().to_string();
        assert!(refused.contains("predates this release"), "{refused}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A database the Turso store wrote and nobody converted is refused with what to do: its
    /// header marks a journal mode SQLite cannot read (the read and write versions, bytes 18
    /// and 19, are 255).
    #[test]
    fn an_unconverted_turso_database_is_refused_with_what_to_do() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("unconverted-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("turso.db");
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (x INTEGER)")
            .unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[18] = 255;
        bytes[19] = 255;
        std::fs::write(&path, bytes).unwrap();
        let refused = SqliteStore::open(&path).unwrap_err().to_string();
        assert!(refused.contains("convert it"), "{refused}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// A14: each stored concept of the PRD glossary and the tables, or `table.column`s, that
    /// hold it. Derived values (due, gravity, rank, frontier, stale, ...) are never stored.
    const GLOSSARY: &[(&str, &[&str])] = &[
        (
            "Deployment",
            &["deployment.revision", "identities", "identity_aliases"],
        ),
        ("Graph", &["graphs"]),
        ("Route", &["routes"]),
        ("Route version", &["route_versions"]),
        ("Route draft", &["route_drafts"]),
        ("Journey", &["journeys"]),
        (
            "Lineage",
            &[
                "journeys.lineage_route",
                "journeys.lineage_version",
                "nodes.from_route",
                "nodes.from_version",
            ],
        ),
        ("Node, Node kind", &["nodes", "nodes.kind"]),
        (
            "Decision",
            &["nodes.prompt", "nodes.answer_type", "nodes.choices"],
        ),
        (
            "Deliverable, Artifact",
            &["nodes.requires_artifact", "annotations.type"],
        ),
        (
            "Required note",
            &["nodes.requires_note", "annotations.type"],
        ),
        (
            "Checklist",
            &["nodes.checklist", "nodes.requires_checklist", "ticks"],
        ),
        ("Action", &["nodes.kind"]),
        ("Milestone", &["nodes.is_final", "nodes.auto_done"]),
        ("Container, Parent / child", &["nodes.parent_key"]),
        (
            "Answer, Rationale",
            &["answers", "answer_identities", "answers.rationale"],
        ),
        ("Edge", &["edges"]),
        ("Condition", &["nodes.relevant_when"]),
        ("Role", &["roles"]),
        ("Filling decision", &["nodes.fills_role"]),
        ("Participation kind", &["participation_kinds"]),
        (
            "Participation, Owner",
            &["participations", "participation_identities"],
        ),
        ("Identity", &["identities", "identity_emails"]),
        ("User", &["users", "user_logins", "login_emails"]),
        ("Pin", &["pins"]),
        ("Date rule", &["nodes.due_by", "nodes.not_before"]),
        (
            "Actual date",
            &["node_states.started_on", "node_states.finished_on"],
        ),
        ("Weight", &["nodes.weight"]),
        ("Placeholder", &["nodes.placeholder", "node_states.atomic"]),
        ("Snooze", &["snoozes"]),
        ("Skip", &["node_states.state"]),
        (
            "Override, Keep, Guard",
            &[
                "overrides.force_include",
                "overrides.keep",
                "overrides.bypass",
            ],
        ),
        ("Patch", &["patch_receipts"]),
        ("Mutation, Event", &["events", "event_nodes"]),
        (
            "Revision",
            &["journeys.revision", "routes.revision", "proposals.revision"],
        ),
        ("Proposal", &["proposals"]),
        ("Resource, Attachment", &["resources", "resources.type"]),
        ("Note / Link", &["annotations"]),
    ];

    #[test]
    fn the_glossarys_stored_concepts_are_tables_and_columns() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("glossary-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        drop(SqliteStore::open(&path).unwrap());
        let connection = rusqlite::Connection::open(&path).unwrap();
        for (concept, places) in GLOSSARY.iter().copied() {
            for place in places {
                let (table, column) = place.split_once('.').unwrap_or((place, ""));
                let query = format!("PRAGMA table_info({table})");
                let mut columns = connection.prepare(&query).unwrap();
                let found: Vec<String> = columns
                    .query_map([], |row| row.get(1))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap();
                assert!(!found.is_empty(), "{concept}: no table {table}");
                assert!(
                    column.is_empty() || found.iter().any(|name| name == column),
                    "{concept}: {table} has no column {column}: {found:?}"
                );
            }
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}
