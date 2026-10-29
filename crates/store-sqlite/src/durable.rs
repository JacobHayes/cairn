//! A commit the store cannot settle fails it closed. A `COMMIT`
//! that fails other than by a constraint (an I/O error, a full disk) may have written its
//! record to the write-ahead log without syncing it, so the store cannot say whether the
//! commit holds, and it answers nothing more: every call errs, and the host exits so a
//! supervisor restarts it. An open starts by evicting the database files' cached pages, so
//! what it reads comes from the disk and not from a cache that may hold a record that never
//! synced.

#[cfg(target_os = "linux")]
use std::path::Path;
use std::sync::OnceLock;

use cairn_store::StoreError;

/// Whether the store still answers.
#[derive(Default)]
pub(crate) struct Durability {
    /// Why the store failed closed: a `COMMIT` failed with its fate unknown.
    failed: OnceLock<String>,
}

impl Durability {
    /// A `COMMIT` failed other than by a constraint, so what it wrote may be in the log and
    /// not on disk (a constraint fails before the log is written). Every call errs from now
    /// on.
    pub fn fail_closed(&self, reason: &str) {
        let _ = self.failed.set(reason.to_owned());
    }

    /// Before a call starts and before it answers: an error once the store has failed
    /// closed, so nothing is answered from a write that may not be on disk.
    ///
    /// # Errors
    ///
    /// Once the store has failed closed.
    pub fn still_open(&self) -> Result<(), StoreError> {
        match self.failed.get() {
            None => Ok(()),
            Some(reason) => Err(StoreError::Backend(format!(
                "the store failed closed: a commit's outcome is unknown ({reason}); it answers \
                 again once restarted"
            ))),
        }
    }
}

/// Drops the database files' cached pages before an open reads them: a log record whose
/// fsync failed can still sit in the operating system's cache, and a reopened store would
/// recover it from the write-ahead log as if it were on disk.
/// `posix_fadvise(POSIX_FADV_DONTNEED)` drops clean pages, which is what a failed fsync
/// leaves. Linux only: elsewhere there is no such call, the open does not make it, and the
/// store relies on its supervisor restart alone. A file that does not exist yet has nothing
/// cached.
///
/// # Errors
///
/// When a file that exists cannot be opened or advised.
#[cfg(target_os = "linux")]
pub(crate) fn evict_cached_pages(database: &Path) -> Result<(), StoreError> {
    // SQLite names the write-ahead log by appending `-wal` to the database's file name.
    let mut log = database.as_os_str().to_owned();
    log.push("-wal");
    for path in [database, Path::new(&log)] {
        let evicted = match std::fs::File::open(path) {
            Ok(file) => rustix::fs::fadvise(&file, 0, None, rustix::fs::Advice::DontNeed)
                .map_err(std::io::Error::from),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
        evicted.map_err(|error| {
            StoreError::Backend(format!(
                "open {}: drop its cached pages: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}
