//! SQLite-specific cases: a long commit beside reads and another domain's commit, a
//! resubmission beside a commit in flight, and a crash in the middle of a commit. Commits
//! run one at a time on the writer, so a commit beside one in flight waits for it and then
//! sees what that one did, while reads go on beside it; what each commit decides once it
//! runs is the shared suite's, sequentially.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cairn_schema::{EventType, GraphRecord, Subject};
use cairn_store::build::{
    action, create_journey, id, journey_graph, journey_patch, put_in, revision,
};
use cairn_store::conformance::{Backend, applied, both, journey};
use cairn_store::{Commit, CommitError, CommitPoint, Committed, Faults, PauseHook, Store};
use cairn_store_sqlite::SqliteStore;
use tokio::sync::Notify;

use super::Sqlite;

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

/// Runs `held` until it reaches the gate, then `meanwhile`, which must still be waiting for
/// `held` a while later; then releases the gate and lets both finish.
async fn beside_held<T, M: Future<Output = T>>(
    store: &SqliteStore,
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

/// ARCHITECTURE, Backends: a long commit to one journey blocks no read of it, which sees
/// the state before the commit, and a commit to another journey waits its turn.
pub async fn a_long_commit_blocks_no_read_and_holds_the_next_commit(backend: &Sqlite) {
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
        let other = store
            .commit(add_node("p_other", "j_two", 1, "n_other"))
            .await;
        (seen, other)
    });
    let (long, (seen, other)) = beside_held(
        &store,
        &gate,
        add_node("p_long", "j_one", 1, "n_long"),
        meanwhile,
    )
    .await;
    assert!(matches!(long, Ok(Committed::Applied { .. })), "{long:?}");
    assert!(matches!(other, Ok(Committed::Applied { .. })), "{other:?}");
    assert_eq!((seen.revision, seen.graph.nodes.len()), (revision(1), 0));
    let after = journey(&store, "j_one").await.unwrap();
    assert_eq!((after.revision, after.graph.nodes.len()), (revision(2), 1));
}

const CRASH_PATH: &str = "CAIRN_STORE_CRASH_PATH";

/// A crash in the middle of a commit loses that commit and nothing else: the process
/// aborts between a commit's state rows and its events, and the reopened store holds
/// every commit before it, none of the crashed one, and goes on committing.
pub async fn a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own(
    backend: &Sqlite,
) {
    if let Ok(path) = std::env::var(CRASH_PATH) {
        crash(std::path::Path::new(&path)).await;
    }
    let path = backend.path(0);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store_sqlite::conformance::conformance::a_crash_mid_commit_keeps_every_earlier_commit_and_nothing_of_its_own",
            "--test-threads=1",
        ])
        .env(CRASH_PATH, &path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success(), "the child process was to abort");

    let store = SqliteStore::open(&path).unwrap();
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
    let store = SqliteStore::open_with_faults(path, faults.clone()).unwrap();
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

/// H5: a patch resubmitted while its original is still in flight (its caller timed out)
/// waits for the original and is answered from its receipt: never applied twice, and never
/// stale on the revision its own original moved.
pub async fn a_resubmission_beside_its_original_in_flight_is_answered_from_its_receipt(
    backend: &Sqlite,
) {
    let faults = Faults::default();
    let store = backend.open(faults.clone()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", Vec::new()).commit(),
    )
    .await;
    let original = add_node("p_add", "j_one", 1, "n_added");
    let gate = Gate::at(CommitPoint::BetweenStateAndEvents, &faults);
    let meanwhile = promptly(store.commit(original.clone()));
    let (first, again) = beside_held(&store, &gate, original, meanwhile).await;
    assert!(matches!(first, Ok(Committed::Applied { .. })), "{first:?}");
    assert!(
        matches!(again, Ok(Committed::AlreadyApplied(_))),
        "{again:?}"
    );
    let after = journey(&store, "j_one").await.unwrap();
    assert_eq!((after.revision, after.graph.nodes.len()), (revision(2), 1));
}
