//! A commit whose logical-log fsync fails
//! (decisions/2026-10-09-a-commit-the-store-cannot-settle-fails-it-closed.md). The
//! durability testbed found it under patina: the log sync of a commit failed, the commit
//! answered `Failed`, yet Turso kept it visible; the resubmission was answered
//! `AlreadyApplied` from a receipt that had never reached the disk, and a crash before the
//! next log sync lost the acknowledged commit. The store now fails closed instead. These
//! are that run minimized to the faults that matter, with the platform I/O wrapped to fail
//! log fsyncs.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cairn_store::build::{action, at, create_journey, id, revision};
use cairn_store::conformance::{applied, journey};
use cairn_store::{AuthStore, CommitError, CommitPoint, Faults, LoadTarget, Store, UserRecord};
use cairn_store_turso::TursoStore;
use cairn_store_turso::turso_core::{
    Buffer, Clock, Completion, CompletionError, File, IO, LimboError, MonotonicInstant, OpenFlags,
    PlatformIO, Result, WallClockInstant, io::FileSyncType,
};

use super::Turso;
use super::races::{Gate, while_held};

/// The platform's I/O, with the next `failing` fsyncs of a logical log (`*-log`) failing, as
/// `UnixIO` reports a failed `fsync(2)`.
struct FailingLogSync {
    inner: Arc<dyn IO>,
    failing: Arc<AtomicU32>,
}

struct LogFile {
    inner: Arc<dyn File>,
    failing: Arc<AtomicU32>,
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
            failing: Arc::clone(&self.failing),
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
        let failing = self.failing.load(Ordering::SeqCst);
        if failing > 0 {
            self.failing.store(failing - 1, Ordering::SeqCst);
            let error = CompletionError::IOError(std::io::ErrorKind::StorageFull, "sync");
            return Err(LimboError::CompletionError(error));
        }
        self.inner.sync(c, sync_type)
    }

    fn size(&self) -> Result<u64> {
        self.inner.size()
    }

    fn truncate(&self, len: u64, c: Completion) -> Result<Completion> {
        self.inner.truncate(len, c)
    }
}

async fn open(backend: &Turso, failing: &Arc<AtomicU32>, faults: Faults) -> TursoStore {
    let io = Arc::new(FailingLogSync {
        inner: Arc::new(PlatformIO::new().unwrap()),
        failing: Arc::clone(failing),
    });
    TursoStore::open_with_io(&backend.path(0), faults, io)
        .await
        .unwrap()
}

/// A store holding journey `j_one` at revision 1.
async fn with_journey(backend: &Turso, failing: &Arc<AtomicU32>) -> TursoStore {
    let store = open(backend, failing, Faults::default()).await;
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;
    store
}

/// Turso keeps a commit whose log fsync failed, so the store cannot say whether it holds: it
/// answers the commit failed, then answers nothing until it is opened again, and the reopened
/// store holds the commit whole or not at all.
pub async fn a_commit_whose_log_sync_fails_leaves_nothing_visible(backend: &Turso) {
    let failing = Arc::new(AtomicU32::new(0));
    let store = with_journey(backend, &failing).await;

    failing.store(1, Ordering::SeqCst);
    let failed = store
        .commit(super::races::add_node("p_two", "j_one", 1, "n_b"))
        .await;
    assert_eq!(failing.load(Ordering::SeqCst), 0, "the sync was tried");
    assert!(
        matches!(failed, Err(CommitError::Failed(_))),
        "the commit was to fail: {failed:?}"
    );
    assert!(store.health().is_err(), "a closed store is healthy");
    let receipt = store.receipt(&id("p_two")).await;
    assert!(receipt.is_err(), "a receipt was answered: {receipt:?}");
    let loaded = store.load(&LoadTarget::Journey(id("j_one"))).await;
    assert!(loaded.is_err(), "a journey was answered: {loaded:?}");
    let next = store
        .commit(super::races::add_node("p_three", "j_one", 1, "n_c"))
        .await;
    assert!(next.is_err(), "a commit was answered: {next:?}");
    drop(store);

    let reopened = open(backend, &failing, Faults::default()).await;
    assert!(reopened.health().is_ok(), "a reopened store is closed");
    let landed = reopened.receipt(&id("p_two")).await.unwrap().is_some();
    let expected = if landed { revision(2) } else { revision(1) };
    assert_eq!(
        journey(&reopened, "j_one").await.unwrap().revision,
        expected
    );
}

/// A commit in flight when the store fails closed is not answered as applied, and one queued
/// behind the commit that closed it is not answered from what that one left: once closed, the
/// store answers nothing.
pub async fn a_commit_in_flight_when_the_store_fails_closed_is_not_applied(backend: &Turso) {
    let failing = Arc::new(AtomicU32::new(0));
    let faults = Faults::default();
    let store = open(backend, &failing, faults.clone()).await;
    for (patch, journey) in [("p_one", "j_one"), ("p_two", "j_two")] {
        let create = create_journey(patch, journey, vec![action("n_a", "a", None)]);
        applied(&store, create.commit()).await;
    }
    let gate = Gate::at(CommitPoint::BeforeCommit, &faults);
    let closing = async {
        failing.store(1, Ordering::SeqCst);
        tokio::join!(
            store.commit(super::races::add_node("p_closing", "j_two", 1, "n_b")),
            store.commit(super::races::add_node("p_queued", "j_two", 1, "n_c")),
        )
    };
    let (held, (closing, queued)) = while_held(
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
    assert!(
        matches!(queued, Err(CommitError::Failed(_))),
        "the queued commit was answered from the closing one's state: {queued:?}"
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

/// A record write whose log fsync failed may have left its record visible, so every call errs
/// after it.
pub async fn a_record_write_whose_log_sync_fails_closes_the_store(backend: &Turso) {
    let failing = Arc::new(AtomicU32::new(0));
    let store = open(backend, &failing, Faults::default()).await;
    assert!(store.health().is_ok(), "a store just opened is not healthy");

    failing.store(1, Ordering::SeqCst);
    let failed = store.put_user(user("u_ann")).await;
    assert!(failed.is_err(), "the write was to fail: {failed:?}");
    assert!(store.health().is_err(), "a closed store is healthy");
    let read = store.user(&id("u_ann")).await;
    assert!(read.is_err(), "a record was answered: {read:?}");
    let write = store.put_user(user("u_bob")).await;
    assert!(write.is_err(), "a record write was answered: {write:?}");
}
