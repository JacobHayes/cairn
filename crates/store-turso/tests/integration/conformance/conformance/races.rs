//! Turso-specific cases (readiness ruling for 3.1): commits in flight together, a long
//! commit beside reads and another domain's commit, a crash in the middle of a commit, and
//! the foreign-key gap the domain revision row closes
//! (decisions/2026-10-06-turso-checks-no-foreign-key-against-a-concurrent-transaction.md).
//! A commit on rows one in flight writes waits its turn and then sees what that one did
//! (the H5 fix,
//! decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md);
//! a commit on other rows does not wait.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cairn_schema::{Domain, EventType, GraphRecord, Record, RecordKey, Rejection, Subject, Write};
use cairn_store::build::{
    action, create_entity, create_journey, entity, id, journey_graph, journey_patch, put_in,
    revision,
};
use cairn_store::conformance::{Backend, applied, both, deployment, journey};
use cairn_store::{Commit, CommitError, CommitPoint, Committed, Faults, PauseHook, Store};
use cairn_store_turso::TursoStore;
use tokio::sync::Notify;

use super::Turso;

/// Holds the first commit to reach `point` there until released.
pub(super) struct Gate {
    point: CommitPoint,
    armed: AtomicBool,
    reached: Notify,
    release: Notify,
}

impl Gate {
    pub(super) fn at(point: CommitPoint, faults: &Faults) -> Arc<Gate> {
        let gate = Arc::new(Gate {
            point,
            armed: AtomicBool::new(true),
            reached: Notify::new(),
            release: Notify::new(),
        });
        let held = gate.clone();
        let hook: PauseHook = Arc::new(move |point| {
            let gate = held.clone();
            Box::pin(async move {
                if point == gate.point && gate.armed.swap(false, Ordering::SeqCst) {
                    gate.reached.notify_one();
                    gate.release.notified().await;
                }
            })
        });
        faults.pause_with(hook);
        gate
    }
}

/// Waits at most five seconds: a store call that waits on a commit in flight fails the case
/// instead of hanging it.
async fn promptly<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("a store call waited on a commit in flight")
}

pub fn add_node(patch: &str, journey: &str, base: u32, key: &str) -> Commit {
    journey_patch(patch, journey, base)
        .event(
            EventType::NodeAdded,
            Subject::Node(id(key)),
            vec![put_in(
                &journey_graph(journey),
                GraphRecord::Node(action(key, key, None)),
            )],
        )
        .commit()
}

/// How long a commit waiting its turn beside a held one is shown to keep waiting.
const MEANWHILE: Duration = Duration::from_millis(50);

/// Runs `held` until it reaches the gate, then `meanwhile` to completion, then releases the
/// gate and lets `held` finish.
pub(super) async fn while_held<T, M: Future<Output = T>>(
    store: &TursoStore,
    gate: &Gate,
    held: Commit,
    meanwhile: M,
) -> (Result<Committed, CommitError>, T) {
    let others = async {
        gate.reached.notified().await;
        let result = meanwhile.await;
        gate.release.notify_one();
        result
    };
    both(store.commit(held), others).await
}

/// Runs `held` until it reaches the gate, then `meanwhile`, which must still be waiting for
/// `held` a while later; then releases the gate and lets both finish.
async fn beside_held<T, M: Future<Output = T>>(
    store: &TursoStore,
    gate: &Gate,
    held: Commit,
    meanwhile: M,
) -> (Result<Committed, CommitError>, T) {
    let others = async {
        gate.reached.notified().await;
        let mut meanwhile = pin!(meanwhile);
        let early = tokio::time::timeout(MEANWHILE, meanwhile.as_mut()).await;
        assert!(
            early.is_err(),
            "a commit beside the held one waits its turn"
        );
        gate.release.notify_one();
        meanwhile.await
    };
    both(store.commit(held), others).await
}

fn rejected_as_stale(result: &Result<Committed, CommitError>) -> bool {
    matches!(result, Err(CommitError::Rejected(Rejection::Stale { .. })))
}

/// ARCHITECTURE, Backends: a long commit to one journey blocks neither a read of it, which
/// sees the state before the commit, nor a commit to another journey.
pub async fn a_long_commit_blocks_neither_reads_nor_another_journeys_commit(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(
        &store,
        create_journey("p_two", "j_two", Vec::new()).commit(),
    )
    .await;
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let meanwhile = Box::pin(async {
        let seen = promptly(journey(&store, "j_one")).await.unwrap();
        let other = promptly(store.commit(add_node("p_other", "j_two", 1, "n_other"))).await;
        (seen, other)
    });
    let (long, (seen, other)) = while_held(
        &store,
        &gate,
        add_node("p_long", "j_one", 1, "n_long"),
        meanwhile,
    )
    .await;
    assert!(matches!(long, Ok(Committed::Applied(_))), "{long:?}");
    assert!(matches!(other, Ok(Committed::Applied(_))), "{other:?}");
    assert_eq!((seen.revision, seen.graph.nodes.len()), (revision(1), 0));
    let after = journey(&store, "j_one").await.unwrap();
    assert_eq!((after.revision, after.graph.nodes.len()), (revision(2), 1));
}

/// Two creates of one journey (a unique key race on its revision row): the second waits its
/// turn, then is a revision conflict; one journey results.
pub async fn two_creates_of_one_journey_in_flight_yield_one_journey(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    let gate = Gate::at(CommitPoint::AfterRevisionRow, &faults);
    let first = create_journey("p_first", "j_one", vec![action("n_first", "first", None)]);
    let second = create_journey(
        "p_second",
        "j_one",
        vec![action("n_second", "second", None)],
    );
    let meanwhile = promptly(store.commit(second.commit()));
    let (first, second) = beside_held(&store, &gate, first.commit(), meanwhile).await;
    let outcomes = [&first, &second];
    let won = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Ok(Committed::Applied(_))))
        .count();
    let lost = outcomes
        .iter()
        .filter(|outcome| rejected_as_stale(outcome))
        .count();
    assert_eq!((won, lost), (1, 1), "{first:?} {second:?}");
    let loaded = journey(&store, "j_one").await.unwrap();
    assert_eq!(
        (loaded.revision, loaded.graph.nodes.len()),
        (revision(1), 1)
    );
}

/// E6: two entity creates of one key, riding in patches to two journeys at once (a unique
/// key race on the entity and the deployment's revision row): the second waits its turn,
/// then finds the key taken; one entity results and the deployment revision moves once.
pub async fn two_entity_creates_of_one_key_in_flight_yield_one_entity(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(
        &store,
        create_journey("p_two", "j_two", Vec::new()).commit(),
    )
    .await;
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let ride = |patch: &str, journey: &str, name: &str| {
        create_entity(
            journey_patch(patch, journey, 1),
            entity("e_shared", name, &[]),
        )
        .commit()
    };
    let meanwhile = promptly(store.commit(ride("p_b", "j_two", "From two")));
    let (first, second) =
        beside_held(&store, &gate, ride("p_a", "j_one", "From one"), meanwhile).await;
    assert!(matches!(first, Ok(Committed::Applied(_))), "{first:?}");
    assert!(
        matches!(
            second,
            Err(CommitError::Rejected(Rejection::Invalid { .. }))
        ),
        "{second:?}"
    );
    let people = deployment(&store).await;
    assert_eq!(people.revision, revision(1));
    let names: Vec<_> = people
        .entities
        .values()
        .map(|held| held.name.as_str().to_owned())
        .collect();
    assert_eq!(names, vec!["From one"]);
    let again = ride("p_b_again", "j_two", "From two");
    let error = store.commit(again).await.unwrap_err();
    let CommitError::Rejected(Rejection::Invalid { violations }) = error else {
        panic!("expected the key to be taken, got {error:?}");
    };
    assert_eq!(
        violations.as_slice()[0].code,
        cairn_schema::ViolationCode::EntityKeyTaken
    );
}

fn delete_journey(patch: &str, journey: &str, base: u32) -> Commit {
    journey_patch(patch, journey, base)
        .event_in(
            Domain::Deployment,
            EventType::JourneyDeleted,
            Subject::Journey(id(journey)),
            vec![
                Write::Remove(RecordKey::Domain(Domain::Journey(id(journey)))),
                Write::Put(Record::DeletedJourney {
                    journey: id(journey),
                    deleted_at: cairn_store::build::at(9),
                }),
            ],
        )
        .commit()
}

/// A19 and the foreign-key gap
/// (decisions/2026-10-06-turso-checks-no-foreign-key-against-a-concurrent-transaction.md):
/// a journey's hard delete (removing parent rows) racing a commit that adds a node to
/// it (a child row) cannot leave an orphan, because both write the journey's revision
/// row: the add waits its turn and is stale.
pub async fn a_delete_racing_a_child_insert_leaves_no_orphan(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let meanwhile = promptly(store.commit(add_node("p_add", "j_one", 1, "n_b")));
    let (deleted, added) = beside_held(
        &store,
        &gate,
        delete_journey("p_delete", "j_one", 1),
        meanwhile,
    )
    .await;
    assert!(matches!(deleted, Ok(Committed::Applied(_))), "{deleted:?}");
    assert!(rejected_as_stale(&added), "{added:?}");
    assert_eq!(journey(&store, "j_one").await, None);
    drop(store);
    let orphans = raw_count(
        &backend.path(0),
        "SELECT count(*) FROM nodes WHERE graph_id NOT IN (SELECT id FROM graphs)",
    )
    .await;
    assert_eq!(orphans, 0);
}

async fn raw_count(path: &std::path::Path, sql: &str) -> i64 {
    let database = turso::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let connection = database.connect().unwrap();
    let mut rows = connection.query(sql, ()).await.unwrap();
    rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap()
}

/// The gap itself, at the SQL level, at the pinned Turso: a child insert and its parent's
/// delete in two concurrent MVCC transactions both commit, leaving an orphan. When this
/// starts failing, Turso checks foreign keys across transactions and
/// decisions/2026-10-06-turso-checks-no-foreign-key-against-a-concurrent-transaction.md
/// can be revisited.
pub async fn turso_lets_a_child_insert_race_its_parents_delete(backend: &Turso) {
    let path = backend.path(0);
    let database = turso::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let setup = database.connect().unwrap();
    for sql in [
        "PRAGMA journal_mode = mvcc",
        "PRAGMA foreign_keys = ON",
        "CREATE TABLE parent (id TEXT PRIMARY KEY) STRICT",
        "CREATE TABLE child (id TEXT PRIMARY KEY, parent TEXT NOT NULL REFERENCES parent (id)) STRICT",
        "INSERT INTO parent VALUES ('p')",
    ] {
        let mut rows = setup.query(sql, ()).await.unwrap();
        while rows.next().await.unwrap().is_some() {}
    }
    let deleting = database.connect().unwrap();
    let inserting = database.connect().unwrap();
    for connection in [&deleting, &inserting] {
        connection
            .execute("PRAGMA foreign_keys = ON", ())
            .await
            .unwrap();
        connection.execute("BEGIN CONCURRENT", ()).await.unwrap();
    }
    deleting
        .execute("DELETE FROM parent WHERE id = 'p'", ())
        .await
        .unwrap();
    inserting
        .execute("INSERT INTO child VALUES ('c', 'p')", ())
        .await
        .unwrap();
    deleting.execute("COMMIT", ()).await.unwrap();
    inserting.execute("COMMIT", ()).await.unwrap();
    let mut rows = setup
        .query(
            "SELECT count(*) FROM child WHERE parent NOT IN (SELECT id FROM parent)",
            (),
        )
        .await
        .unwrap();
    let orphans: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(orphans, 1);
}

const CRASH_PATH: &str = "CAIRN_STORE_CRASH_PATH";

/// A crash in the middle of a commit loses that commit and nothing else: the process
/// aborts between a commit's state rows and its events, and the reopened store holds
/// every commit before it, none of the crashed one, and goes on committing.
pub async fn a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own(backend: &Turso) {
    if let Ok(path) = std::env::var(CRASH_PATH) {
        crash(std::path::Path::new(&path)).await;
    }
    let path = backend.path(0);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "conformance::conformance::a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own",
            "--test-threads=1",
        ])
        .env(CRASH_PATH, &path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "the child process was to abort");

    let store = TursoStore::open(&path).await.unwrap();
    let survived = journey(&store, "j_one").await.unwrap();
    assert_eq!(
        (survived.revision, survived.graph.nodes.len()),
        (revision(1), 1)
    );
    assert_eq!(store.receipt(&id("p_crash")).await.unwrap(), None);
    let events = cairn_store::conformance::all_events(&store).await;
    assert!(
        events
            .iter()
            .all(|logged| logged.event.patch_id.as_str() == "p_one")
    );
    applied(&store, add_node("p_after", "j_one", 1, "n_after")).await;
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(2)
    );
}

/// The child: one commit, then a second that aborts the process between its state rows
/// and its events.
async fn crash(path: &std::path::Path) -> ! {
    let faults = Faults::default();
    let store = TursoStore::open_with_faults(path, faults.clone())
        .await
        .unwrap();
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    let abort: PauseHook = Arc::new(|point| {
        Box::pin(async move {
            if point == CommitPoint::BetweenStateAndEvents {
                std::process::abort();
            }
        })
    });
    faults.pause_with(abort);
    let _ = store
        .commit(add_node("p_crash", "j_one", 1, "n_crash"))
        .await;
    unreachable!("the commit aborts the process");
}

fn answer_with(patch: &str, journey: &str, base: u32, entity_key: &str) -> Commit {
    journey_patch(patch, journey, base)
        .expects(cairn_schema::RevisionOf::Domain(Domain::Deployment), 1)
        .event(
            EventType::AnswerSet,
            Subject::Node(id("n_who")),
            vec![put_in(
                &journey_graph(journey),
                GraphRecord::Answer {
                    decision: id("n_who"),
                    value: cairn_schema::AnswerValue::Entity(id(entity_key)),
                    rationale: None,
                },
            )],
        )
        .commit()
}

fn merge(patch: &str) -> Commit {
    cairn_store::build::deployment_patch(patch, 1)
        .referencing(&["e_a", "e_b"], &[])
        .event(
            EventType::EntitiesMerged,
            Subject::Entity(id("e_b")),
            vec![
                Write::Remove(RecordKey::Entity(id("e_b"))),
                Write::Put(Record::EntityAlias {
                    alias: id("e_b"),
                    entity: id("e_a"),
                }),
            ],
        )
        .commit()
}

/// E6: a journey patch starting to reference an entity and a merge of that entity, in
/// flight together, do not both commit: the merge would miss the new reference. The patch
/// writing the reference holds the deployment's revision row, so the merge waits its turn
/// and is then stale on the journey that now references its entity.
pub async fn a_reference_and_a_merge_in_flight_do_not_both_commit(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    let people = cairn_store::build::deployment_patch("p_people", 0).event(
        EventType::EntityCreated,
        Subject::Entity(id("e_a")),
        vec![
            Write::Put(Record::Entity(entity("e_a", "A", &[]))),
            Write::Put(Record::Entity(entity("e_b", "B", &[]))),
        ],
    );
    applied(&store, people.commit()).await;
    let who = cairn_store::build::node(serde_json::json!({
        "key": "n_who", "id": "who", "kind": "decision", "title": "Who",
        "prompt": "Who?", "answer_type": "entity",
    }));
    applied(&store, create_journey("p_one", "j_one", vec![who]).commit()).await;
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let meanwhile = promptly(store.commit(merge("p_merge")));
    let (referenced, merged) = beside_held(
        &store,
        &gate,
        answer_with("p_answer", "j_one", 1, "e_b"),
        meanwhile,
    )
    .await;
    assert!(
        matches!(referenced, Ok(Committed::Applied(_))),
        "{referenced:?}"
    );
    assert!(rejected_as_stale(&merged), "{merged:?}");
    let again = store.commit(merge("p_merge_again")).await;
    assert!(rejected_as_stale(&again), "{again:?}");
}

/// A19: a proposal created while its journey is hard-deleted does not survive it: the
/// delete waits its turn at the journey's revision row and takes the proposal with it.
pub async fn a_proposal_and_its_journeys_deletion_in_flight_do_not_both_commit(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let destination = Domain::Journey(id("j_one"));
    let proposal =
        cairn_store::build::proposal_patch("p_proposal", "pr_one", destination.clone(), 0)
            .event(
                EventType::ProposalCreated,
                Subject::Proposal(id("pr_one")),
                vec![Write::Put(Record::Proposal(cairn_store::build::proposal(
                    "pr_one",
                    &destination,
                    1,
                )))],
            )
            .commit();
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let meanwhile = promptly(store.commit(delete_journey("p_delete", "j_one", 1)));
    let (proposed, deleted) = beside_held(&store, &gate, proposal, meanwhile).await;
    assert!(
        matches!(proposed, Ok(Committed::Applied(_))),
        "{proposed:?}"
    );
    assert!(matches!(deleted, Ok(Committed::Applied(_))), "{deleted:?}");
    assert_eq!(store.proposal(&id("pr_one")).await.unwrap(), None);
    assert_eq!(journey(&store, "j_one").await, None);
}

/// J5: history pages by a cursor that never skips an event: an event that becomes visible
/// after a later-numbered one was read is still on a later page.
pub async fn history_paging_never_skips_an_event_committed_late(backend: &Turso) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    applied(
        &store,
        create_journey("p_two", "j_two", Vec::new()).commit(),
    )
    .await;
    let gate = Gate::at(CommitPoint::BeforeCommit, &faults);
    let meanwhile = async {
        promptly(store.commit(add_node("p_quick", "j_two", 1, "n_quick")))
            .await
            .unwrap();
        promptly(read_after(&store, None)).await
    };
    let (held, (seen, cursor)) = while_held(
        &store,
        &gate,
        add_node("p_held", "j_one", 1, "n_held"),
        meanwhile,
    )
    .await;
    assert!(matches!(held, Ok(Committed::Applied(_))), "{held:?}");
    let (rest, _) = read_after(&store, cursor).await;
    let mut patches: Vec<String> = seen.into_iter().chain(rest).collect();
    patches.sort();
    patches.dedup();
    assert_eq!(patches, vec!["p_held", "p_one", "p_quick", "p_two"]);
}

/// The patch of every event after `after`, and the last position read.
async fn read_after(store: &TursoStore, mut after: Option<u64>) -> (Vec<String>, Option<u64>) {
    let mut patches = Vec::new();
    loop {
        let query = cairn_store::EventQuery {
            after,
            size: cairn_store::PageSize::new(1),
            ..cairn_store::EventQuery::default()
        };
        let page = store.events(&query).await.unwrap();
        for logged in &page.items {
            patches.push(logged.event.patch_id.as_str().to_owned());
            after = Some(logged.seq);
        }
        if page.next.is_none() {
            return (patches, after);
        }
    }
}

/// H5: a committed patch resubmitted while a later commit to its journey is in flight waits
/// its turn and is answered from its receipt, never as stale.
pub async fn a_resubmission_beside_a_commit_in_flight_is_answered_from_its_receipt(
    backend: &Turso,
) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let first = add_node("p_first", "j_one", 1, "n_first");
    applied(&store, first.clone()).await;
    let gate = Gate::at(CommitPoint::AfterRevisionRow, &faults);
    let meanwhile = promptly(store.commit(first));
    let (later, again) = beside_held(
        &store,
        &gate,
        add_node("p_later", "j_one", 2, "n_later"),
        meanwhile,
    )
    .await;
    assert!(matches!(later, Ok(Committed::Applied(_))), "{later:?}");
    assert!(
        matches!(again, Ok(Committed::AlreadyApplied(_))),
        "{again:?}"
    );
}
