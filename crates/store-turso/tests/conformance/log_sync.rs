//! A commit whose logical-log fsync fails (DECISIONS.md, 6.2: an acknowledged commit lost
//! after a failed log fsync). The durability testbed found it under patina: the log sync of
//! a commit failed, the commit answered `Failed`, the client resubmitted, the resubmission
//! was answered `AlreadyApplied` from a receipt that had never reached the disk, and a
//! crash before the next log sync lost the acknowledged commit. This is that run minimized
//! to the one fault that matters, with the platform I/O wrapped to fail one log fsync.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cairn_store::build::{action, create_journey, id, revision};
use cairn_store::conformance::{applied, journey};
use cairn_store::{CommitError, Faults, Store};
use cairn_store_turso::TursoStore;
use cairn_store_turso::turso_core::{
    Buffer, Clock, Completion, CompletionError, File, IO, LimboError, MonotonicInstant, OpenFlags,
    PlatformIO, Result, WallClockInstant, io::FileSyncType,
};

use super::Turso;

/// The platform's I/O, with the next fsync of a logical log (`*-log`) failing once, as
/// `UnixIO` reports a failed `fsync(2)`.
struct FailingLogSync {
    inner: Arc<dyn IO>,
    armed: Arc<AtomicBool>,
}

struct LogFile {
    inner: Arc<dyn File>,
    armed: Arc<AtomicBool>,
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
            armed: Arc::clone(&self.armed),
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
        if self.armed.swap(false, Ordering::SeqCst) {
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

/// A commit whose log fsync failed answers `Failed`, so it must leave nothing a reader or a
/// resubmission can see: its receipt would answer a resubmission as applied, and its
/// revision would let the next patch build on a commit a crash can still take away.
pub async fn a_commit_whose_log_sync_fails_leaves_nothing_visible(backend: &Turso) {
    let armed = Arc::new(AtomicBool::new(false));
    let io = Arc::new(FailingLogSync {
        inner: Arc::new(PlatformIO::new().unwrap()),
        armed: Arc::clone(&armed),
    });
    let store = TursoStore::open_with_io(&backend.path(0), Faults::default(), io)
        .await
        .unwrap();
    applied(
        &store,
        create_journey("p_one", "j_one", vec![action("n_a", "a", None)]).commit(),
    )
    .await;

    armed.store(true, Ordering::SeqCst);
    let failed = store
        .commit(super::races::add_node("p_two", "j_one", 1, "n_b"))
        .await;
    assert!(!armed.load(Ordering::SeqCst), "the commit synced its log");
    assert!(
        matches!(failed, Err(CommitError::Failed(_))),
        "the commit was to fail: {failed:?}"
    );
    assert_eq!(store.receipt(&id("p_two")).await.unwrap(), None);
    assert_eq!(
        journey(&store, "j_one").await.unwrap().revision,
        revision(1)
    );
}
