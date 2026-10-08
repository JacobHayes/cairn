//! A commit whose logical-log fsync fails
//! (decisions/2026-10-07-the-log-sync-fix-nothing-is-answered-from-a-write-until.md). The
//! durability testbed found it under patina: the log sync of a commit failed, the commit
//! answered `Failed`, yet Turso kept it visible; the resubmission was answered
//! `AlreadyApplied` from a receipt that had never reached the disk, and a crash before the
//! next log sync lost the acknowledged commit. These are that run minimized to the faults
//! that matter, with the platform I/O wrapped to fail log fsyncs.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cairn_store::build::{action, at, create_journey, id, revision};
use cairn_store::conformance::{applied, journey};
use cairn_store::{
    AuthStore, CommitError, CommitPoint, Committed, Faults, LoadTarget, Store, UserRecord,
};
use cairn_store_turso::TursoStore;
use cairn_store_turso::turso_core::{
    Buffer, Clock, Completion, CompletionError, File, IO, LimboError, MonotonicInstant, OpenFlags,
    PlatformIO, Result, WallClockInstant, io::FileSyncType,
};

use super::Turso;
use super::races::{Gate, while_held};

/// The platform's I/O, with the next `failing` fsyncs of a logical log (`*-log`) failing, as
/// `UnixIO` reports a failed `fsync(2)`, and the log's successful fsyncs counted.
struct FailingLogSync {
    inner: Arc<dyn IO>,
    log: Arc<LogSyncs>,
}

#[derive(Default)]
struct LogSyncs {
    failing: AtomicU32,
    synced: AtomicU32,
}

struct LogFile {
    inner: Arc<dyn File>,
    log: Arc<LogSyncs>,
}

impl Clock for FailingLogSync {
    fn current_time_monotonic(&self) -> MonotonicInstant {
        self.inner.current_time_monotonic()
    }

    fn current_time_wall_clock(&self) -> WallClockInstant {
        self.inner.current_time_wall_clock()
    }
}

impl IO for FailingLogSync {
    fn open_file(&self, path: &str, flags: OpenFlags, direct: bool) -> Result<Arc<dyn File>> {
        let file = self.inner.open_file(path, flags, direct)?;
        if !path.ends_with("-log") {
            return Ok(file);
        }
        Ok(Arc::new(LogFile {
            inner: file,
            log: Arc::clone(&self.log),
        }))
    }

    fn remove_file(&self, path: &str) -> Result<()> {
        self.inner.remove_file(path)
    }

    fn step(&self) -> Result<()> {
        self.inner.step()
    }
}

impl File for LogFile {
    fn lock_file(&self, exclusive: bool) -> Result<()> {
        self.inner.lock_file(exclusive)
    }

    fn unlock_file(&self) -> Result<()> {
        self.inner.unlock_file()
    }

    fn pread(&self, pos: u64, c: Completion) -> Result<Completion> {
        self.inner.pread(pos, c)
    }

    fn pwrite(&self, pos: u64, buffer: Arc<Buffer>, c: Completion) -> Result<Completion> {
        self.inner.pwrite(pos, buffer, c)
    }

    fn pwritev(&self, pos: u64, buffers: Vec<Arc<Buffer>>, c: Completion) -> Result<Completion> {
        self.inner.pwritev(pos, buffers, c)
    }

    fn sync(&self, c: Completion, sync_type: FileSyncType) -> Result<Completion> {
        let failing = self.log.failing.load(Ordering::SeqCst);
        if failing > 0 {
            self.log.failing.store(failing - 1, Ordering::SeqCst);
            let error = CompletionError::IOError(std::io::ErrorKind::StorageFull, "sync");
            return Err(LimboError::CompletionError(error));
        }
        let synced = self.inner.sync(c, sync_type)?;
        self.log.synced.fetch_add(1, Ordering::SeqCst);
        Ok(synced)
    }

    fn size(&self) -> Result<u64> {
        self.inner.size()
    }

    fn truncate(&self, len: u64, c: Completion) -> Result<Completion> {
        self.inner.truncate(len, c)
    }
}

async fn open(backend: &Turso, log: &Arc<LogSyncs>, faults: Faults) -> TursoStore {
    let io = Arc::new(FailingLogSync {
        inner: Arc::new(PlatformIO::new().unwrap()),
        log: Arc::clone(log),
    });
    TursoStore::open_with_io(&backend.path(0), faults, io)
        .await
        .unwrap()
}

/// A store holding journey `j_one` at revision 1.
async fn with_journey(backend: &Turso, log: &Arc<LogSyncs>) -> TursoStore {
    let store = open(backend, log, Faults::default()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    store
}

/// Turso keeps a commit whose log fsync failed. Once a later sync of the log has succeeded,
/// that commit is on disk, so it is answered applied, and only then.
pub async fn a_commit_whose_log_sync_fails_is_answered_once_the_log_is_synced(backend: &Turso) {
    let log = Arc::new(LogSyncs::default());
    let store = with_journey(backend, &log).await;

    log.failing.store(1, Ordering::SeqCst);
    let synced_before = log.synced.load(Ordering::SeqCst);
    let answer = store
        .commit(super::races::add_node("p_two", "j_one", 1, "n_b"))
        .await;
    assert_eq!(
        log.failing.load(Ordering::SeqCst),
        0,
        "the commit synced its log"
    );
    assert!(
        matches!(answer, Ok(Committed::Applied(_))),
        "the commit was to be applied: {answer:?}"
    );
    assert!(
        log.synced.load(Ordering::SeqCst) > synced_before,
        "the commit was answered before its log was synced"
    );
    assert!(store.receipt(&id("p_two")).await.unwrap().is_some());
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(2)
    );
}

/// When the log cannot be synced after a commit whose log fsync failed, the store answers
/// nothing until it is opened again, and the open settles the log: the commit is there
/// whole, or not at all.
pub async fn a_commit_whose_log_sync_fails_leaves_nothing_visible(backend: &Turso) {
    let log = Arc::new(LogSyncs::default());
    let store = with_journey(backend, &log).await;

    // The commit's sync, and the sync of the barrier after it.
    log.failing.store(2, Ordering::SeqCst);
    let failed = store
        .commit(super::races::add_node("p_two", "j_one", 1, "n_b"))
        .await;
    assert_eq!(
        log.failing.load(Ordering::SeqCst),
        0,
        "both syncs were tried"
    );
    assert!(
        matches!(failed, Err(CommitError::Failed(_))),
        "the commit was to fail: {failed:?}"
    );
    let receipt = store.receipt(&id("p_two")).await;
    assert!(receipt.is_err(), "a receipt was answered: {receipt:?}");
    let loaded = store.load(&LoadTarget::Journey(id("j_one"))).await;
    assert!(loaded.is_err(), "a journey was answered: {loaded:?}");
    let next = store
        .commit(super::races::add_node("p_three", "j_one", 1, "n_c"))
        .await;
    assert!(next.is_err(), "a commit was answered: {next:?}");
    drop(store);

    let reopened = open(backend, &log, Faults::default()).await;
    let landed = reopened.receipt(&id("p_two")).await.unwrap().is_some();
    let expected = if landed { revision(2) } else { revision(1) };
    assert_eq!(
        journey(&reopened, "j_one").await.unwrap().revision,
        expected
    );
}

/// A commit in flight when the store fails closed is not answered as applied: once closed,
/// the store answers nothing.
pub async fn a_commit_in_flight_when_the_store_fails_closed_is_not_applied(backend: &Turso) {
    let log = Arc::new(LogSyncs::default());
    let faults = Faults::default();
    let store = open(backend, &log, faults.clone()).await;
    for (patch, journey) in [("p_one", "j_one"), ("p_two", "j_two")] {
        let create = create_journey(patch, journey, vec![action("n_a", "a", None)]);
        applied(&store, create.commit()).await;
    }
    let gate = Gate::at(CommitPoint::BeforeCommit, &faults);
    let closing = async {
        // The commit's sync, and the sync of the barrier after it.
        log.failing.store(2, Ordering::SeqCst);
        store
            .commit(super::races::add_node("p_closing", "j_two", 1, "n_b"))
            .await
    };
    let (held, closing) = while_held(
        &store,
        &gate,
        super::races::add_node("p_held", "j_one", 1, "n_b"),
        closing,
    )
    .await;
    assert!(
        closing.is_err(),
        "the closing commit was answered: {closing:?}"
    );
    assert!(held.is_err(), "the held commit was answered: {held:?}");
}

fn user(key: &str) -> UserRecord {
    UserRecord {
        id: id(key),
        name: id("Someone"),
        created_at: at(0),
    }
}

/// A record write whose log fsync failed may have left its record visible, so the next call
/// answers only once a later sync of the log has covered it.
pub async fn a_record_write_whose_log_sync_fails_is_settled_before_the_next_answer(
    backend: &Turso,
) {
    let log = Arc::new(LogSyncs::default());
    let store = open(backend, &log, Faults::default()).await;

    log.failing.store(1, Ordering::SeqCst);
    let failed = store.put_user(user("u_ann")).await;
    assert!(failed.is_err(), "the write was to fail: {failed:?}");
    let synced_before = log.synced.load(Ordering::SeqCst);
    let read = store.user(&id("u_ann")).await;
    assert!(
        log.synced.load(Ordering::SeqCst) > synced_before,
        "the read answered before the log was synced: {read:?}"
    );
    assert!(read.unwrap().is_some(), "Turso kept the record");
    assert!(
        store.health().is_ok(),
        "a store that settled is not healthy"
    );
}

/// When the barrier after a record write's failed log fsync fails too, every call errs.
pub async fn a_record_write_whose_log_sync_and_barrier_fail_closes_the_store(backend: &Turso) {
    let log = Arc::new(LogSyncs::default());
    let store = open(backend, &log, Faults::default()).await;
    assert!(store.health().is_ok(), "a store just opened is not healthy");

    log.failing.store(2, Ordering::SeqCst);
    let failed = store.put_user(user("u_ann")).await;
    assert!(failed.is_err(), "the write was to fail: {failed:?}");
    let read = store.user(&id("u_ann")).await;
    assert!(read.is_err(), "a record was answered: {read:?}");
    assert!(
        store.health().is_err(),
        "a store that failed closed is healthy"
    );
    assert_eq!(
        log.failing.load(Ordering::SeqCst),
        0,
        "both syncs were tried"
    );
    let write = store.put_user(user("u_bob")).await;
    assert!(write.is_err(), "a record write was answered: {write:?}");
}

/// A reopened process can read back a log record a failed sync left off the disk, from the
/// operating system's cache, so an open syncs the log before it answers anything.
pub async fn an_open_syncs_the_log_before_it_answers(backend: &Turso) {
    drop(with_journey(backend, &Arc::new(LogSyncs::default())).await);
    let log = Arc::new(LogSyncs::default());
    let reopened = open(backend, &log, Faults::default()).await;
    assert!(
        log.synced.load(Ordering::SeqCst) > 0,
        "the open synced no log"
    );
    assert_eq!(
        journey(&reopened, "j_one").await.unwrap().revision,
        revision(1)
    );
}
