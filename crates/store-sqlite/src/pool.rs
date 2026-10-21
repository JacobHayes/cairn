//! The store's connections (ARCHITECTURE, Backends): one writer, which every write takes in
//! turn, and a small pool of readers, at most `store_connection_count_max`, which refuse every
//! write (`query_only`), so only the writer ever writes. Every wait for a
//! connection lasts at most `store_connection_acquire` (PRACTICES, Explicit limits). Every
//! connection enforces foreign keys and syncs each commit to disk, which SQLite sets per
//! connection.
//!
//! Every connection opens on SQLite's `unix-excl` VFS: the process takes one exclusive lock
//! on the database file at its first access and holds it while any connection is open, so
//! one process has the database (ARCHITECTURE, Assumptions), and the write-ahead log's index
//! is kept in this process's memory rather than in a memory-mapped `-shm` file, which the
//! durability simulation's mapped files cannot be trusted with (a short injected read
//! truncates the mapping, a patina defect) and which a process that is alone needs no file for.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use cairn_store::StoreError;
use cairn_store::limits::{STORE_CONNECTION_ACQUIRE, STORE_CONNECTION_COUNT_MAX};
use rusqlite::{Connection, ErrorCode, OpenFlags};
use tokio::sync::{OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};

/// The connections. A writer guard and a lease own what they hold, so a call can take its
/// connection onto a blocking thread.
pub(crate) struct Pool {
    path: PathBuf,
    writer: Arc<tokio::sync::Mutex<Connection>>,
    idle: Mutex<Vec<Connection>>,
    permits: Arc<Semaphore>,
}

/// A reader on loan. It goes back to the pool only when [`Lease::release`] says it is
/// outside any transaction; otherwise it is dropped, which rolls back what it held.
pub(crate) struct Lease {
    pool: Arc<Pool>,
    connection: Option<Connection>,
    _permit: OwnedSemaphorePermit,
}

impl Pool {
    /// The pool over the database at `path`, whose writer is `writer`.
    pub fn new(path: &Path, writer: Connection) -> Arc<Self> {
        let permits = usize::try_from(STORE_CONNECTION_COUNT_MAX).unwrap_or(usize::MAX);
        Arc::new(Self {
            path: path.to_path_buf(),
            writer: Arc::new(tokio::sync::Mutex::new(writer)),
            idle: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(permits)),
        })
    }

    /// The writer, once every write before has ended, waiting at most the acquire limit.
    pub async fn writer(&self) -> Result<OwnedMutexGuard<Connection>, StoreError> {
        let writer = tokio::time::timeout(
            STORE_CONNECTION_ACQUIRE,
            Arc::clone(&self.writer).lock_owned(),
        )
        .await
        .map_err(|_| StoreError::AcquireTimeout)?;
        if writer.is_autocommit() {
            return Ok(writer);
        }
        // A write that panicked left its transaction open; the next starts from what is
        // committed, rolled back off the runtime's threads as every statement is.
        crate::blocking(move || {
            crate::commit::rollback(&writer);
            Ok::<_, StoreError>(writer)
        })
        .await
    }

    /// A reader, waiting at most the acquire limit for one to come free.
    pub async fn reader(self: &Arc<Self>) -> Result<Lease, StoreError> {
        let permit = tokio::time::timeout(
            STORE_CONNECTION_ACQUIRE,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| StoreError::AcquireTimeout)?
        .map_err(|error| StoreError::Backend(format!("the pool is closed: {error}")))?;
        let idle = self
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let connection = if let Some(connection) = idle {
            connection
        } else {
            let path = self.path.clone();
            crate::blocking(move || read_only(connect(&path)?)).await?
        };
        Ok(Lease {
            pool: Arc::clone(self),
            connection: Some(connection),
            _permit: permit,
        })
    }
}

/// A new connection on `unix-excl` with foreign keys on and every commit synced (`FULL`: in WAL mode
/// `NORMAL` may lose the last commits to a power cut, and nothing is answered from a write
/// that may not be on disk).
///
/// # Errors
///
/// When the file cannot be opened.
pub(crate) fn connect(path: &Path) -> Result<Connection, StoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let opened = |error: rusqlite::Error| match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy) => StoreError::Backend(format!(
            "open {}: another process has it open",
            path.display()
        )),
        // The first statement reads the file's header: a database the Turso store wrote
        // marks a journal mode SQLite cannot read until it is converted.
        Some(ErrorCode::NotADatabase) => StoreError::Backend(format!(
            "open {}: {error}: a database an earlier release wrote opens only once converted; \
             convert it as the guide to running a deployment says, under Upgrades",
            path.display()
        )),
        _ => StoreError::Backend(format!("open {}: {error}", path.display())),
    };
    let connection =
        Connection::open_with_flags_and_vfs(path, flags, c"unix-excl").map_err(opened)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
        .map_err(opened)?;
    Ok(connection)
}

/// `connection` refusing every write, so a read that writes by mistake fails instead of
/// writing beside the writer.
fn read_only(connection: Connection) -> Result<Connection, StoreError> {
    connection
        .execute_batch("PRAGMA query_only = ON;")
        .map_err(|error| StoreError::Backend(format!("a reader refusing writes: {error}")))?;
    Ok(connection)
}

impl Lease {
    pub fn connection(&self) -> &Connection {
        // Present until release or drop.
        self.connection
            .as_ref()
            .unwrap_or_else(|| unreachable!("a lease holds its connection until it ends"))
    }

    /// Returns the connection to the pool: it is outside any transaction.
    pub fn release(mut self) {
        if let Some(connection) = self.connection.take() {
            self.pool
                .idle
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(connection);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(future)
    }

    /// A directory of its own, removed when the case ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "cairn-store-sqlite-pool-{name}-{}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            Self(directory)
        }

        fn pool(&self) -> Arc<Pool> {
            let path = self.0.join("pool.db");
            Pool::new(&path, connect(&path).unwrap())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn pragma(connection: &Connection, name: &str) -> i64 {
        crate::sql::first(connection, &format!("PRAGMA {name}"), Vec::new())
            .unwrap()
            .unwrap()
            .int(0)
            .unwrap()
    }

    #[test]
    fn every_connection_enforces_foreign_keys_and_syncs_each_commit() {
        let scratch = Scratch::new("pragmas");
        run(async {
            let pool = scratch.pool();
            let lease = pool.reader().await.unwrap();
            let writer = pool.writer().await.unwrap();
            for connection in [lease.connection(), &*writer] {
                assert_eq!(pragma(connection, "foreign_keys"), 1);
                // 2 is FULL.
                assert_eq!(pragma(connection, "synchronous"), 2);
            }
        });
    }

    /// The bundled SQLite keeps no allocation statistics, whose one process-wide mutex
    /// serializes concurrent readers: the flag comes from the workspace's cargo config, which
    /// a build outside it would not read.
    #[test]
    fn sqlite_is_built_without_allocation_statistics() {
        let scratch = Scratch::new("options");
        let pool = scratch.pool();
        let writer = run(pool.writer()).unwrap();
        let mut statement = writer.prepare("PRAGMA compile_options").unwrap();
        let options: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            options.iter().any(|option| option == "DEFAULT_MEMSTATUS=0"),
            "{options:?}"
        );
    }

    #[test]
    fn a_reader_refuses_every_write() {
        let scratch = Scratch::new("read-only");
        run(async {
            let pool = scratch.pool();
            let writer = pool.writer().await.unwrap();
            writer.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
            drop(writer);
            let lease = pool.reader().await.unwrap();
            let refused = lease
                .connection()
                .execute_batch("INSERT INTO t VALUES (1);")
                .unwrap_err();
            assert_eq!(
                refused.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ReadOnly)
            );
        });
    }

    #[test]
    fn a_reader_past_the_connection_limit_waits_out_the_acquire_limit() {
        let scratch = Scratch::new("readers");
        run(async {
            let pool = scratch.pool();
            let mut leases = Vec::new();
            for _ in 0..STORE_CONNECTION_COUNT_MAX {
                leases.push(pool.reader().await.unwrap());
            }
            let started = tokio::time::Instant::now();
            assert!(matches!(
                pool.reader().await,
                Err(StoreError::AcquireTimeout)
            ));
            assert_eq!(started.elapsed(), STORE_CONNECTION_ACQUIRE);
            leases.pop().unwrap().release();
            assert!(pool.reader().await.is_ok());
        });
    }

    /// A write that panicked mid-transaction leaves it open on the writer; the next write
    /// starts from what is committed.
    #[test]
    fn a_write_left_open_is_rolled_back_before_the_next_write() {
        let scratch = Scratch::new("left-open");
        run(async {
            let pool = scratch.pool();
            let writer = pool.writer().await.unwrap();
            writer
                .execute_batch("CREATE TABLE t (x INTEGER); BEGIN; INSERT INTO t VALUES (1);")
                .unwrap();
            drop(writer);
            let writer = pool.writer().await.unwrap();
            assert!(writer.is_autocommit());
            let count: i64 = writer
                .query_row("SELECT count(*) FROM t", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0);
        });
    }

    #[test]
    fn a_write_waits_for_the_one_before_it_at_most_the_acquire_limit() {
        let scratch = Scratch::new("writer");
        run(async {
            let pool = scratch.pool();
            let held = pool.writer().await.unwrap();
            let started = tokio::time::Instant::now();
            assert!(matches!(
                pool.writer().await,
                Err(StoreError::AcquireTimeout)
            ));
            assert_eq!(started.elapsed(), STORE_CONNECTION_ACQUIRE);
            drop(held);
            assert!(pool.writer().await.is_ok());
        });
    }
}
