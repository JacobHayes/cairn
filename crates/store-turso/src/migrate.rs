//! Numbered SQL migrations, embedded in the binary and applied in order inside one
//! transaction when a store opens (ARCHITECTURE, Backends). The database is put in MVCC
//! mode first: data MVCC writes is invisible to a non-MVCC open until checkpointed.

use cairn_store::StoreError;
use turso::Connection;

use crate::sql::{execute, first, int, rows};

/// Every migration, numbered from 1, in order. A migration is never edited once released;
/// a change is a new one.
pub(crate) const MIGRATIONS: [(u32, &str); 7] = [
    (1, include_str!("migrations/0001_initial.sql")),
    (2, include_str!("migrations/0002_outside_domains.sql")),
    (3, include_str!("migrations/0003_receipt_footprints.sql")),
    (4, include_str!("migrations/0004_log_barrier.sql")),
    (5, include_str!("migrations/0005_answer_rationale.sql")),
    (6, include_str!("migrations/0006_requires_note.sql")),
    (7, include_str!("migrations/0007_segments.sql")),
];

/// Puts the database in MVCC mode and applies the migrations it lacks.
pub(crate) async fn run(connection: &Connection) -> Result<(), StoreError> {
    rows(connection, "PRAGMA journal_mode = mvcc", Vec::new()).await?;
    execute(
        connection,
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY) STRICT",
        Vec::new(),
    )
    .await?;
    let applied = first(
        connection,
        "SELECT max(version) FROM schema_migrations",
        Vec::new(),
    )
    .await?
    .and_then(|row| row.opt_int(0).ok().flatten())
    .unwrap_or(0);
    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|(number, _)| i64::from(*number) > applied)
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    execute(connection, "BEGIN", Vec::new()).await?;
    for (number, sql) in pending {
        let applied = async {
            connection
                .execute_batch(sql)
                .await
                .map_err(crate::sql::SqlError::from)?;
            execute(
                connection,
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                vec![int(*number)],
            )
            .await
        }
        .await;
        if let Err(error) = applied {
            let _ = execute(connection, "ROLLBACK", Vec::new()).await;
            return Err(StoreError::Backend(format!(
                "migration {number}: {error:?}"
            )));
        }
    }
    execute(connection, "COMMIT", Vec::new()).await?;
    Ok(())
}
