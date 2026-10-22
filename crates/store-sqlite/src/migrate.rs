//! Numbered migrations, embedded in the binary and applied in order inside one transaction
//! when a store opens (ARCHITECTURE, Backends). The database is put in WAL mode first, so no
//! read waits on the writer.
//!
//! The first migration is 13: the schema the Turso store's migrations 1 to 13 left, as one
//! file. A database converted from a release on that store records 1 to 13 or later and
//! takes the migrations after what it records; one that stopped short of 13 is refused,
//! since only the Turso store could bring it there
//! (decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md).

use cairn_store::StoreError;
use rusqlite::Connection;

use crate::sql::{SqlError, execute, int, rows};

/// One step of a migration: statements, or code for what SQL cannot tell apart (JSON whose
/// tags and keys sit among text a person wrote).
pub(crate) enum Step {
    Sql(&'static str),
    MilestoneDone,
    DeletedIds,
}

/// The first migration this store applies.
const FIRST: u32 = 13;

/// Every migration, numbered from [`FIRST`], in order. A migration is never edited once
/// released; a change is a new one. It is a list of steps run in one transaction with its
/// number.
pub(crate) const MIGRATIONS: [(u32, &[Step]); 6] = [
    (
        FIRST,
        &[Step::Sql(include_str!("migrations/0013_schema.sql"))],
    ),
    (
        14,
        &[
            Step::MilestoneDone,
            Step::Sql(include_str!("migrations/0014_milestone_done.sql")),
        ],
    ),
    (
        15,
        &[Step::Sql(include_str!(
            "migrations/0015_event_positions.sql"
        ))],
    ),
    (
        16,
        &[
            Step::DeletedIds,
            Step::Sql(include_str!("migrations/0016_deleted_ids.sql")),
        ],
    ),
    (17, &[Step::Sql(include_str!("migrations/0017_tour.sql"))]),
    (
        18,
        &[Step::Sql(include_str!(
            "migrations/0018_journey_owners.sql"
        ))],
    ),
];

/// Puts the database in WAL mode and applies the migrations it lacks.
pub(crate) fn run(connection: &Connection) -> Result<(), StoreError> {
    let mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|error| StoreError::Backend(error.to_string()))?;
    if mode != "wal" {
        return Err(StoreError::Backend(format!(
            "the database did not take WAL mode (it is in {mode:?})"
        )));
    }
    execute(
        connection,
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY) STRICT",
        Vec::new(),
    )?;
    let recorded: Vec<i64> = rows(
        connection,
        "SELECT version FROM schema_migrations",
        Vec::new(),
    )?
    .iter()
    .map(|row| row.int(0))
    .collect::<Result<_, _>>()?;
    if !recorded.is_empty() && !recorded.contains(&i64::from(FIRST)) {
        return Err(StoreError::Backend(format!(
            "the database's schema predates this release (its last migration is {:?}); bring \
             it up to date with an earlier release first, then convert it",
            recorded.iter().max()
        )));
    }
    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|(number, _)| !recorded.contains(&i64::from(*number)))
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    execute(connection, "BEGIN IMMEDIATE", Vec::new())?;
    for (number, steps) in pending {
        let applied = steps
            .iter()
            .try_for_each(|step| match step {
                Step::Sql(sql) => connection.execute_batch(sql).map_err(SqlError::from),
                Step::MilestoneDone => crate::to_done::run(connection),
                Step::DeletedIds => crate::to_deleted_ids::run(connection),
            })
            .and_then(|()| {
                execute(
                    connection,
                    "INSERT INTO schema_migrations (version) VALUES (?1)",
                    vec![int(*number)],
                )
            });
        if let Err(error) = applied {
            let _ = execute(connection, "ROLLBACK", Vec::new());
            return Err(StoreError::Backend(format!(
                "migration {number}: {error:?}"
            )));
        }
    }
    execute(connection, "COMMIT", Vec::new())?;
    Ok(())
}
