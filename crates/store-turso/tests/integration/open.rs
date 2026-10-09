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

    /// A patch revising the journey's one decision, with `rationale` when given.
    fn revise(patch: &str, base: u32, yes: bool, rationale: Option<&str>) -> cairn_store::Commit {
        use cairn_schema::{AnswerValue, EventType, GraphRecord, Subject};
        use cairn_store::build::{id, journey_graph, journey_patch, put_in};

        let record = GraphRecord::Answer {
            decision: id("n_who"),
            value: AnswerValue::Boolean(yes),
            rationale: rationale.map(|text| text.parse().unwrap()),
        };
        journey_patch(patch, "j_one", base)
            .event(
                EventType::AnswerSet,
                Subject::Node(id("n_who")),
                vec![put_in(&journey_graph("j_one"), record)],
            )
            .commit()
    }

    /// The decision's answer and rationale as the store at `path` loads them.
    async fn answer_at(path: &std::path::Path) -> (bool, Option<String>) {
        let store = TursoStore::open(path).await.unwrap();
        let loaded = store
            .load(&LoadTarget::Journey(cairn_store::build::id("j_one")))
            .await
            .unwrap();
        let Some(Document::Journey(journey)) = loaded else {
            panic!("the journey loads: {loaded:?}");
        };
        let state = journey.graph.state;
        let key = cairn_store::build::id("n_who");
        let Some(cairn_schema::AnswerValue::Boolean(yes)) = state.answers.get(&key) else {
            panic!("the answer loads: {:?}", state.answers);
        };
        (
            *yes,
            state
                .rationales
                .get(&key)
                .map(|text| text.as_str().to_owned()),
        )
    }

    /// Whether the loaded journey's action `node` requires a note.
    fn requires_note(loaded: Option<Document>, node: &str) -> bool {
        let Some(Document::Journey(journey)) = loaded else {
            panic!("the journey loads: {loaded:?}");
        };
        let node = journey
            .graph
            .nodes
            .get(&cairn_store::build::id(node))
            .unwrap();
        matches!(&node.payload, cairn_schema::Payload::Action(action) if action.requires_note)
    }

    /// An answer's rationale, a node's `requires_note` flag, and a route's kind: a database
    /// from before the columns existed keeps its answers, nodes, and routes, which load with
    /// no rationale, the flag off, and the kind `process`, and takes a rationale on a later
    /// revision (a revision that gives none stores none, B2) and nodes that set the flag (G4).
    /// The older database is the current one with the columns and the insertions table taken
    /// away, so the migrations run over a stored answer, node, and route.
    #[test]
    fn a_database_from_before_rationales_requires_note_and_kinds_loads_and_takes_them() {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("rationale-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("store.db");
        run(async {
            let store = TursoStore::open(&path).await.unwrap();
            let decision = cairn_store::build::node(serde_json::json!({
                "key": "n_who", "id": "who", "kind": "decision", "title": "Who",
                "prompt": "Who?", "answer_type": "boolean",
            }));
            let create =
                create_journey("p_one", "j_one", vec![decision, action("n_a", "a", None)]).commit();
            store.commit(create).await.unwrap();
            store.commit(revise("p_two", 1, true, None)).await.unwrap();
            let route = cairn_store::build::create_route("p_route", "vendor").commit();
            store.commit(route).await.unwrap();
            drop(store);

            let database = turso::Builder::new_local(path.to_str().unwrap())
                .build()
                .await
                .unwrap();
            let connection = database.connect().unwrap();
            for statement in [
                "ALTER TABLE nodes DROP COLUMN requires_note",
                "ALTER TABLE answers DROP COLUMN rationale",
                "ALTER TABLE routes DROP COLUMN kind",
                "DROP TABLE insertions",
                "DELETE FROM schema_migrations WHERE version >= 5",
            ] {
                connection.execute(statement, ()).await.unwrap();
            }
            drop(connection);
            drop(database);

            let store = TursoStore::open(&path).await.unwrap();
            let reason = Some("A reason.\n\n- one");
            store
                .commit(revise("p_three", 2, false, reason))
                .await
                .unwrap();
            drop(store);
            assert_eq!(answer_at(&path).await, (false, reason.map(str::to_owned)));
            let store = TursoStore::open(&path).await.unwrap();
            store.commit(revise("p_four", 3, true, None)).await.unwrap();
            drop(store);
            assert_eq!(answer_at(&path).await, (true, None));

            let store = TursoStore::open(&path).await.unwrap();
            let flagged = cairn_store::build::node(serde_json::json!({
                "key": "n_write", "id": "write", "kind": "action", "title": "Write",
                "requires_note": true,
            }));
            let create = create_journey("p_new", "j_new", vec![flagged]).commit();
            store.commit(create).await.unwrap();
            let load = |journey: &str| LoadTarget::Journey(cairn_store::build::id(journey));
            let route = LoadTarget::Route(cairn_store::build::id("vendor"));
            let Some(Document::Route(route)) = store.load(&route).await.unwrap() else {
                panic!("the route loads");
            };
            assert_eq!(route.header.kind, cairn_schema::RouteKind::Process);
            assert!(!requires_note(
                store.load(&load("j_one")).await.unwrap(),
                "n_a"
            ));
            assert!(requires_note(
                store.load(&load("j_new")).await.unwrap(),
                "n_write"
            ));
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
        ("Route, Segment", &["routes", "routes.kind"]),
        ("Insertion", &["insertions"]),
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
        (
            "Required note",
            &["nodes.requires_note", "annotations.type"],
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
        (
            "Answer, Rationale",
            &["answers", "answer_entities", "answers.rationale"],
        ),
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
