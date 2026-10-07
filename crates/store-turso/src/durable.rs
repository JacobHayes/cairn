//! Nothing is answered from a write that may not be on disk (DECISIONS.md, the log sync
//! fix). Turso keeps a transaction whose record it appended to the logical log even when
//! the log's fsync then fails and its `COMMIT` errs (`turso_core` 0.8.2, a dropped commit
//! whose log was appended is marked committed): the write is visible, and reaches the disk
//! only with a later successful sync of the log. So a `COMMIT` that fails other than by a
//! conflict or a constraint leaves the store unsettled, and no call is answered until a
//! barrier, a write of its own, has synced the log past it. When the barrier fails too, the
//! store fails closed: every call errs until it is opened again, and every open settles what
//! the log holds with a barrier before it answers anything.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use cairn_store::StoreError;
use turso::Connection;

use crate::sql::{SqlError, execute};

/// Whether every write Turso shows has reached the disk.
#[derive(Default)]
pub(crate) struct Durability {
    /// `COMMIT`s that failed with their fate unknown, counted.
    unsettled: AtomicU64,
    /// How many of those the last successful barrier covered.
    settled: AtomicU64,
    /// Held while a barrier runs, so one runs at a time.
    settling: tokio::sync::Mutex<()>,
    /// Why the store failed closed: the barrier that was to settle a failed `COMMIT` failed.
    failed: OnceLock<String>,
}

impl Durability {
    /// A `COMMIT` failed other than by a conflict or a constraint, so what it wrote may be
    /// visible and not on disk. Called before the failing call awaits anything else, so no
    /// later call settles without covering it.
    pub fn unsettle(&self) {
        self.unsettled.fetch_add(1, Ordering::SeqCst);
    }

    /// Before a call answers: when a `COMMIT` failed since the last barrier, runs one on
    /// `connection` (outside any transaction), whose successful sync covers every log record
    /// appended before it; a barrier that fails other than by Busy or a conflict fails the
    /// store closed.
    ///
    /// # Errors
    ///
    /// Once the store has failed closed, or while a barrier meets Busy or a conflict.
    pub async fn settle(&self, connection: &Connection) -> Result<(), StoreError> {
        if self.is_unsettled() {
            let _turn = self.settling.lock().await;
            // Read before the barrier begins: every failure counted by now appended its
            // record before the barrier's own, so the barrier's sync covers it.
            let through = self.unsettled.load(Ordering::SeqCst);
            if self.is_unsettled() {
                match barrier(connection).await {
                    Ok(()) => {
                        self.settled.fetch_max(through, Ordering::SeqCst);
                    }
                    // Busy or a write-write conflict (a checkpoint in progress holds the
                    // database): nothing failed to sync, so the store stays unsettled and the
                    // next call tries again.
                    Err(SqlError::Conflict(reason)) => {
                        return Err(StoreError::Backend(format!(
                            "the store is settling a failed commit and could not yet: {reason}"
                        )));
                    }
                    Err(error) => {
                        let _ = self.failed.set(format!("{error:?}"));
                    }
                }
            }
        }
        self.still_open()
    }

    /// Before a write answers that it succeeded: a store that failed closed while the write
    /// was in flight answers it failed, since it answers nothing once closed.
    ///
    /// # Errors
    ///
    /// Once the store has failed closed.
    pub fn still_open(&self) -> Result<(), StoreError> {
        match self.failed.get() {
            None => Ok(()),
            Some(reason) => Err(StoreError::Backend(format!(
                "the store failed closed: a commit's log sync failed and the barrier after it \
                 failed too ({reason}); it answers again once reopened"
            ))),
        }
    }

    fn is_unsettled(&self) -> bool {
        self.failed.get().is_none()
            && self.settled.load(Ordering::SeqCst) < self.unsettled.load(Ordering::SeqCst)
    }
}

/// A write of its own, committed: its log record follows every one appended before it, so
/// its successful sync makes them durable too. It counts in `log_barrier`'s one row, which
/// nothing else writes, so it conflicts with no commit.
pub(crate) async fn barrier(connection: &Connection) -> Result<(), SqlError> {
    execute(connection, "BEGIN CONCURRENT", Vec::new()).await?;
    let count = "UPDATE log_barrier SET count = count + 1 WHERE id = 1";
    let committed = match execute(connection, count, Vec::new()).await {
        Ok(_) => execute(connection, "COMMIT", Vec::new()).await,
        Err(error) => Err(error),
    };
    if let Err(error) = committed {
        // A failed commit has already ended its transaction; the rollback only makes sure.
        let _ = execute(connection, "ROLLBACK", Vec::new()).await;
        return Err(error);
    }
    Ok(())
}
