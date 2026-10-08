//! Opening a store: the migrations apply once, committed data survives a reopen, and the
//! migrated schema names the glossary's stored concepts (A14).

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

    /// A14: each stored concept of the PRD glossary and the tables, or `table.column`s, that
    /// hold it. Derived values (due, gravity, rank, frontier, stale, ...) are never stored.
    const GLOSSARY: &[(&str, &[&str])] = &[
        (
            "Deployment",
            &["deployment.revision", "entities", "entity_aliases"],
        ),
        ("Graph", &["graphs"]),
        ("Route", &["routes"]),
        ("Route version", &["route_versions"]),
        ("Route draft", &["route_drafts"]),
        ("Journey", &["journeys"]),
        (
            "Lineage",
            &["journeys.lineage_route", "journeys.lineage_version"],
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
        ("Action", &["nodes.kind"]),
        ("Milestone", &["nodes.is_final", "nodes.auto_reach"]),
        (
            "Group",
            &[
                "nodes.opens_at",
                "nodes.closes_at",
                "nodes.gates",
                "nodes.closes",
            ],
        ),
        ("Container, Parent / child", &["nodes.parent_key"]),
        ("Answer", &["answers", "answer_entities"]),
        ("Edge", &["edges"]),
        ("Condition", &["nodes.relevant_when"]),
        ("Role", &["roles"]),
        ("Filling decision", &["nodes.fills_role"]),
        ("Participation kind", &["participation_kinds"]),
        (
            "Participation, Owner",
            &["participations", "participation_entities"],
        ),
        ("Entity", &["entities", "entity_emails"]),
        ("User", &["users", "user_identities", "identity_emails"]),
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
        ("Local edit", &["local_edits"]),
        ("Provenance", &["node_states.provenance"]),
        ("Node tombstone", &["tombstones"]),
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
        run(async {
            drop(TursoStore::open(&path).await.unwrap());
            let database = turso::Builder::new_local(path.to_str().unwrap())
                .build()
                .await
                .unwrap();
            let connection = database.connect().unwrap();
            for (concept, places) in GLOSSARY.iter().copied() {
                for place in places {
                    let (table, column) = place.split_once('.').unwrap_or((place, ""));
                    let query = format!("PRAGMA table_info({table})");
                    let mut columns = connection.query(query, ()).await.unwrap();
                    let mut found = Vec::new();
                    while let Some(row) = columns.next().await.unwrap() {
                        found.push(row.get::<String>(1).unwrap());
                    }
                    assert!(!found.is_empty(), "{concept}: no table {table}");
                    assert!(
                        column.is_empty() || found.iter().any(|name| name == column),
                        "{concept}: {table} has no column {column}: {found:?}"
                    );
                }
            }
        });
        std::fs::remove_dir_all(&root).unwrap();
    }
}
