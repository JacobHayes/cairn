//! A commit the store cannot settle fails it closed
//! (decisions/2026-10-09-a-commit-the-store-cannot-settle-fails-it-closed.md). Turso keeps a
//! transaction whose record it appended to the logical log even when the log's fsync then
//! fails and its `COMMIT` errs (`turso_core` 0.8.2, a dropped commit whose log was appended
//! is marked committed): the write is visible, and reaches the disk only with a later
//! successful sync. So a `COMMIT` that fails other than by a conflict or a constraint leaves
//! the store's state in doubt, and the store answers nothing more: every call errs, and the
//! host exits so a supervisor restarts it. An open starts by evicting the database files'
//! cached pages, so what it reads comes from the disk and not from a cache that may hold a
//! record that never synced.

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
    /// A `COMMIT` failed other than by a conflict or a constraint, so what it wrote may be
    /// visible and not on disk (those two fail before the log record is written). Every
    /// call errs from now on.
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
/// read it back as if it were on disk. `posix_fadvise(POSIX_FADV_DONTNEED)` drops clean
/// pages, which is what a failed fsync leaves. Linux only: elsewhere there is no such call,
/// the open does not make it, and the store relies on its supervisor restart alone. A file
/// that does not exist yet has nothing cached.
///
/// # Errors
///
/// When a file that exists cannot be opened or advised.
#[cfg(target_os = "linux")]
pub(crate) fn evict_cached_pages(database: &Path) -> Result<(), StoreError> {
    // Turso names the logical log by replacing the extension (`data.sqlite` logs to
    // `data.db-log`), not by appending to the file name.
    for path in [database.to_path_buf(), database.with_extension("db-log")] {
        let path = path.as_path();
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
