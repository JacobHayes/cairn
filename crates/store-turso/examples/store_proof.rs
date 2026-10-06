//! The proof for brief 3.1, run by `briefs/proof/3.1/prove.sh`: a journey holding every
//! record committed to a Turso file and loaded back equal, with the rows it became; a
//! revision conflict; a failure between state rows and events leaving nothing; an entity
//! create riding in a journey patch; a create at a deleted journey's id; an oversize
//! commit; and the glossary's concepts found as tables and columns (A14). Prints Markdown
//! and exits non-zero if any outcome differs from the expected one.
//!
//! usage: `cargo run -p cairn-store-turso --example store_proof -- WORK_DIR`

use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use cairn_schema::{EventType, Graph, GraphId, GraphRecord, Journey, Record, Subject, Write};
use cairn_store::build::{
    action, create_entity, create_journey, create_route, entity, every_content_graph, every_state,
    graph_writes, id, journey_graph, journey_header, journey_patch, publish, put_in, route_patch,
};
use cairn_store::{CommitError, CommitPoint, Document, Faults, LoadTarget, Store};
use cairn_store_turso::TursoStore;

type Outcome<T = ()> = Result<T, Box<dyn Error>>;

/// The Markdown written so far and the outcomes that differed from the expected.
#[derive(Default)]
struct Report {
    text: String,
    mismatches: Vec<String>,
}

impl Report {
    fn section(&mut self, title: &str, body: &str) -> Outcome {
        write!(self.text, "\n## {title}\n\n{body}\n")?;
        Ok(())
    }

    fn expect(&mut self, what: &str, held: bool) {
        if !held {
            self.mismatches.push(what.to_owned());
        }
    }
}

fn main() -> Outcome {
    let work = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: store_proof WORK_DIR")?,
    );
    // Every run starts from empty databases.
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let report = runtime.block_on(prove(&work))?;
    print!("{}", report.text);
    if report.mismatches.is_empty() {
        Ok(())
    } else {
        Err(format!("outcomes differed: {:?}", report.mismatches).into())
    }
}

async fn prove(work: &Path) -> Outcome<Report> {
    let mut report = Report::default();
    let path = work.join("every-record.db");
    round_trip(&path, &mut report).await?;
    revision_conflict(&path, &mut report).await?;
    atomic_failure(&work.join("atomic.db"), &mut report).await?;
    riding_entity(&path, &mut report).await?;
    deleted_journey(&path, &mut report).await?;
    oversize(&work.join("oversize.db"), &mut report).await?;
    glossary(&path, &mut report).await?;
    Ok(report)
}

fn json<T: serde::Serialize>(value: &T) -> Outcome<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

/// A journey holding every record, created from a published version of a route holding
/// every content record, then loaded back.
async fn round_trip(path: &Path, report: &mut Report) -> Outcome {
    let store = TursoStore::open(path).await?;
    let content = every_content_graph();
    let draft = GraphId::RouteDraft(id("every"));
    let route = create_route("p_route", "every").event(
        EventType::RouteVersionImported,
        Subject::Route(id("every")),
        graph_writes(&draft, &content),
    );
    store.commit(route.commit()).await?;
    store
        .commit(publish(route_patch("p_publish", "every", 1), "every", 1, 7).commit())
        .await?;
    let header = journey_header("j_every", "Every record", Some(("every", 1)));
    let state = Graph {
        state: every_state(),
        ..Graph::default()
    };
    let mut writes = vec![
        Write::Put(Record::JourneyHeader(header.clone())),
        Write::CopyGraph {
            from: GraphId::RouteVersion {
                route: id("every"),
                version: cairn_store::build::version(1),
            },
            to: journey_graph("j_every"),
        },
    ];
    writes.extend(graph_writes(&journey_graph("j_every"), &state));
    let create = journey_patch("p_journey", "j_every", 0).event(
        EventType::JourneyCreated,
        Subject::Journey(id("j_every")),
        writes,
    );
    let receipt = store.commit(create.commit()).await?;
    let expected = Journey {
        header,
        revision: cairn_store::build::revision(1),
        graph: Graph {
            state: every_state(),
            ..content
        },
    };
    let loaded = store.load(&LoadTarget::Journey(id("j_every"))).await?;
    let equal = loaded == Some(Document::Journey(expected.clone()));
    report.expect("the journey loads back equal", equal);
    drop(store);
    let counts = row_counts(path).await?;
    let body = format!(
        "The route `every` holds a node of every kind with every field, edges, participations \
         of each source, every resource type, roles, kinds, and retired keys; version 1 is \
         published from it, and the journey `j_every` is created from version 1 with state of \
         every kind (`cairn_store::build::every_content_graph`, `every_state`). Commit \
         receipt:\n\n```json\n{}\n```\n\nLoaded back from the Turso file and compared with the \
         journey written: **{}**. The rows the three graphs (the draft, version 1, and the journey) and the log became, by table:\n\n| Table | Rows |\n|---|---|\n{counts}",
        json(receipt.receipt())?,
        if equal { "equal" } else { "DIFFERENT" },
    );
    report.section(
        "A journey with every record, committed to Turso and loaded back",
        &body,
    )
}

/// Every table's row count, read from the file directly.
async fn row_counts(path: &Path) -> Outcome<String> {
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    let mut tables = Vec::new();
    let mut rows = connection
        .query(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite%' \
             AND name NOT LIKE '\\_\\_%' ESCAPE '\\' ORDER BY name",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        tables.push(row.get::<String>(0)?);
    }
    let mut lines = String::new();
    for table in tables {
        let mut count = connection
            .query(format!("SELECT count(*) FROM {table}"), ())
            .await?;
        let rows: i64 = match count.next().await? {
            Some(row) => row.get(0)?,
            None => 0,
        };
        if rows > 0 {
            writeln!(lines, "| `{table}` | {rows} |")?;
        }
    }
    Ok(lines)
}

/// H5: two patches drafted against revision 1; the second is stale.
async fn revision_conflict(path: &Path, report: &mut Report) -> Outcome {
    let store = TursoStore::open(path).await?;
    let add = |patch: &str, key: &str| {
        journey_patch(patch, "j_every", 1)
            .event(
                EventType::NodeAdded,
                Subject::Node(id(key)),
                vec![put_in(
                    &journey_graph("j_every"),
                    GraphRecord::Node(action(key, key, None)),
                )],
            )
            .commit()
    };
    let first = store.commit(add("p_first", "n_first")).await?;
    let second = store.commit(add("p_second", "n_second")).await;
    let stale = matches!(
        &second,
        Err(CommitError::Rejected(cairn_schema::Rejection::Stale { .. }))
    );
    report.expect("the second patch at revision 1 is stale", stale);
    let rejection = match &second {
        Err(CommitError::Rejected(rejection)) => json(rejection)?,
        other => format!("{other:?}"),
    };
    let body = format!(
        "Two patches to `j_every`, both against revision 1. The first commits and produces \
         revision {}; the second is rejected, naming the revision that moved and the touched \
         set of the events since (H5), so a client can retry on its own when its own touched \
         set does not overlap:\n\n```json\n{rejection}\n```",
        first.receipt().revision
    );
    report.section("A revision conflict", &body)
}

/// A17, J2: a failure after the state rows and before the events leaves nothing.
async fn atomic_failure(path: &Path, report: &mut Report) -> Outcome {
    let faults = Faults::default();
    let store = TursoStore::open_with_faults(path, faults.clone()).await?;
    store
        .commit(create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit())
        .await?;
    let commit = create_entity(
        journey_patch("p_two", "j_one", 1),
        entity("e_new", "New", &[]),
    )
    .event(
        EventType::NodeAdded,
        Subject::Node(id("n_b")),
        vec![put_in(
            &journey_graph("j_one"),
            GraphRecord::Node(action("n_b", "b", None)),
        )],
    )
    .commit();
    faults.fail_once(CommitPoint::BetweenStateAndEvents);
    let failed = store.commit(commit.clone()).await;
    let receipt = store.receipt(&id("p_two")).await?;
    drop(store);
    let after_failure = row_counts(path).await?;
    let store = TursoStore::open(path).await?;
    let retried = store.commit(commit).await?;
    report.expect("the failed commit errs", failed.is_err());
    report.expect("the failed commit leaves no receipt", receipt.is_none());
    let unchanged = after_failure.contains("| `nodes` | 1 |")
        && !after_failure.contains("`entities`")
        && after_failure.contains("| `events` | 2 |");
    report.expect("the failed commit leaves no rows", unchanged);
    let body = format!(
        "A patch to `j_one` (revision 1) adds node `n_b` and creates entity `e_new`. A fault \
         armed between its state rows and its events fails it: `{failed:?}`. Its receipt \
         afterwards: `{receipt:?}`. The rows of the file then, with only the first commit in \
         them (one node, its two events, no entity):\n\n| Table | Rows |\n|---|---|\n\
         {after_failure}\nThe same commit, submitted again with no fault armed, produces \
         revision {}.",
        retried.receipt().revision
    );
    report.section(
        "A failure between state rows and events leaves nothing",
        &body,
    )
}

/// E6: an entity create riding in a journey patch moves the deployment revision.
async fn riding_entity(path: &Path, report: &mut Report) -> Outcome {
    let store = TursoStore::open(path).await?;
    let before = deployment_revision(&store).await?;
    let riding = create_entity(
        journey_patch("p_ride", "j_every", 2),
        entity("e_new", "New Person", &["new@example.org"]),
    );
    let receipt = store.commit(riding.commit()).await?;
    let after = deployment_revision(&store).await?;
    report.expect(
        "the deployment revision moves by one",
        after == before.next(),
    );
    let again = create_entity(
        journey_patch("p_ride_again", "j_every", 3),
        entity("e_new", "Someone Else", &[]),
    );
    let taken = store.commit(again.commit()).await;
    let rejection = match &taken {
        Err(CommitError::Rejected(rejection)) => json(rejection)?,
        other => format!("{other:?}"),
    };
    report.expect("a second create of the key is rejected", taken.is_err());
    let body = format!(
        "A patch to `j_every` creates entity `e_new` with no deployment revision check: the \
         journey moves to revision {}, and the deployment from revision {before} to {after}. A \
         second create of `e_new`, riding in the next patch, is rejected:\n\n```json\n{rejection}\n```",
        receipt.receipt().revision
    );
    report.section("An entity create riding in a journey patch", &body)
}

async fn deployment_revision(store: &TursoStore) -> Outcome<cairn_schema::Revision> {
    match store.load(&LoadTarget::Deployment).await? {
        Some(document) => Ok(document.revision()),
        None => Err("the deployment always loads".into()),
    }
}

/// A19: a hard delete, and a create at the deleted id.
async fn deleted_journey(path: &Path, report: &mut Report) -> Outcome {
    let store = TursoStore::open(path).await?;
    let delete = journey_patch("p_delete", "j_every", 3).event_in(
        cairn_schema::Domain::Deployment,
        EventType::JourneyDeleted,
        Subject::Journey(id("j_every")),
        vec![
            Write::Remove(cairn_schema::RecordKey::Domain(
                cairn_schema::Domain::Journey(id("j_every")),
            )),
            Write::Put(Record::DeletedJourney {
                journey: id("j_every"),
                deleted_at: cairn_store::build::at(60),
            }),
        ],
    );
    store.commit(delete.commit()).await?;
    let again = create_journey("p_again", "j_every", Vec::new());
    let rejected = store.commit(again.commit()).await;
    report.expect("the create at a deleted id is rejected", rejected.is_err());
    let rejection = match &rejected {
        Err(CommitError::Rejected(rejection)) => json(rejection)?,
        other => format!("{other:?}"),
    };
    let body = format!(
        "`j_every` is hard-deleted (its events go with it; the deployment log keeps the \
         deletion), and a create at its id is then rejected:\n\n```json\n{rejection}\n```"
    );
    report.section("A create at a hard-deleted journey's id", &body)
}

/// PRACTICES, Explicit limits: a commit that would take a graph past its cap.
async fn oversize(path: &Path, report: &mut Report) -> Outcome {
    let store = TursoStore::open(path).await?;
    store
        .commit(create_journey("p_one", "j_big", Vec::new()).commit())
        .await?;
    let mut builder = journey_patch("p_big", "j_big", 1);
    for index in 0..300 {
        let key = format!("n_bulk{index}");
        let node = cairn_store::build::node(serde_json::json!({
            "key": key, "id": format!("bulk{index}"), "kind": "action", "title": "Bulk",
            "description": "x".repeat(60_000),
        }));
        builder = builder.event(
            EventType::NodeAdded,
            Subject::Node(id(&key)),
            vec![put_in(&journey_graph("j_big"), GraphRecord::Node(node))],
        );
    }
    let rejected = store.commit(builder.commit()).await;
    let rejection = match &rejected {
        Err(CommitError::Rejected(rejection)) => json(rejection)?,
        other => format!("{other:?}"),
    };
    report.expect(
        "the oversize commit names graph_bytes_max",
        rejection.contains("graph_bytes_max"),
    );
    let body = format!(
        "A patch adding 300 nodes with 60,000-byte descriptions to `j_big` would take its \
         graph past 16 MiB; it is rejected, naming the limit:\n\n```json\n{rejection}\n```"
    );
    report.section("An oversize commit names the limit", &body)
}

/// A14: where each glossary concept lives, checked against the migrated schema.
const GLOSSARY: [(&str, &str); 40] = [
    (
        "Deployment",
        "deployment.revision, entities, entity_aliases",
    ),
    ("Graph", "graphs"),
    ("Route", "routes"),
    ("Route version", "route_versions"),
    ("Route draft", "route_drafts"),
    ("Journey", "journeys"),
    (
        "Lineage",
        "journeys.lineage_route, journeys.lineage_version",
    ),
    ("Node, Node kind", "nodes, nodes.kind"),
    ("Decision", "nodes.prompt, nodes.answer_type, nodes.choices"),
    (
        "Deliverable, Artifact",
        "nodes.requires_artifact, annotations.type",
    ),
    ("Action", "nodes.kind"),
    ("Milestone", "nodes.is_final, nodes.auto_reach"),
    (
        "Group",
        "nodes.opens_at, nodes.closes_at, nodes.gates, nodes.closes",
    ),
    ("Container, Parent / child", "nodes.parent_key"),
    ("Answer", "answers, answer_entities"),
    ("Edge", "edges"),
    ("Condition", "nodes.relevant_when"),
    ("Role", "roles"),
    ("Filling decision", "nodes.fills_role"),
    ("Participation kind", "participation_kinds"),
    (
        "Participation, Owner",
        "participations, participation_entities",
    ),
    ("Entity", "entities, entity_emails"),
    ("User", "users, user_identities, identity_emails"),
    ("Pin", "pins"),
    ("Date rule", "nodes.due_by, nodes.not_before"),
    (
        "Actual date",
        "node_states.started_on, node_states.finished_on",
    ),
    ("Weight", "nodes.weight"),
    ("Placeholder", "nodes.placeholder, node_states.atomic"),
    ("Snooze", "snoozes"),
    ("Skip", "node_states.state"),
    (
        "Override, Keep, Guard",
        "overrides.force_include, overrides.keep, overrides.bypass",
    ),
    ("Local edit", "local_edits"),
    ("Provenance", "node_states.provenance"),
    ("Node tombstone", "tombstones"),
    ("Patch", "patch_receipts"),
    ("Mutation, Event", "events, event_nodes"),
    (
        "Revision",
        "journeys.revision, routes.revision, proposals.revision",
    ),
    ("Proposal", "proposals"),
    ("Resource, Attachment", "resources, resources.type"),
    ("Note / Link", "annotations"),
];

/// Glossary terms that are derived and never stored (D3, J1: events never store derived
/// fields).
const DERIVED: &str = "Due, Effective date, Gravity, Slack, Constraint, Earliest start / \
    latest start, Contradictory chain, Shortfall, Leverage, Rank, Frontier, Acting frontier, \
    Undecided, Stalled, Actionable, Blocked, Terminal, Started early, Stale, Condition gate / \
    implicit edge, Touched set (read from `events.delta`), Upgrade available (from \
    `journeys.lineage_version` and `route_versions`), Structural change, Consequences";

async fn glossary(path: &Path, report: &mut Report) -> Outcome {
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    let mut table = String::from("| Concept | Table or column | Found |\n|---|---|---|\n");
    for (concept, places) in GLOSSARY {
        let mut found = true;
        for place in places.split(", ") {
            let (name, column) = place.split_once('.').unwrap_or((place, ""));
            let mut columns = connection
                .query(format!("PRAGMA table_info({name})"), ())
                .await?;
            let mut present = false;
            while let Some(row) = columns.next().await? {
                present |= column.is_empty() || row.get::<String>(1)? == column;
            }
            found &= present;
        }
        report.expect(concept, found);
        let mark = if found { "yes" } else { "NO" };
        writeln!(table, "| {concept} | `{places}` | {mark} |")?;
    }
    let body = format!(
        "Each concept of the PRD glossary that is stored, and where, checked against the \
         migrated schema of the file above:\n\n{table}\nDerived, never stored: {DERIVED}."
    );
    report.section("A14: the glossary as tables and columns", &body)
}
