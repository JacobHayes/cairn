//! Numbered SQL migrations, embedded in the binary and applied in order inside one
//! transaction when a store opens (ARCHITECTURE, Backends). The database is put in MVCC
//! mode first: data MVCC writes is invisible to a non-MVCC open until checkpointed.

use cairn_store::StoreError;
use turso::Connection;

use crate::sql::{SqlError, execute, int, rows};

/// One step of a migration: statements, or code for what SQL cannot tell apart (JSON whose
/// tags and keys sit among text a person wrote).
pub(crate) enum Step {
    Sql(&'static str),
    Identities,
}

/// Every migration, numbered from 1, in order. A migration is never edited once released;
/// a change is a new one. It is a list of steps run in one transaction with its number.
pub(crate) const MIGRATIONS: [(u32, &[Step]); 11] = [
    (1, &[Step::Sql(include_str!("migrations/0001_initial.sql"))]),
    (
        2,
        &[Step::Sql(include_str!(
            "migrations/0002_outside_domains.sql"
        ))],
    ),
    (
        3,
        &[Step::Sql(include_str!(
            "migrations/0003_receipt_footprints.sql"
        ))],
    ),
    (
        4,
        &[Step::Sql(include_str!("migrations/0004_log_barrier.sql"))],
    ),
    (
        5,
        &[Step::Sql(include_str!(
            "migrations/0005_answer_rationale.sql"
        ))],
    ),
    (
        6,
        &[Step::Sql(include_str!("migrations/0006_requires_note.sql"))],
    ),
    (
        7,
        &[Step::Sql(include_str!("migrations/0007_segments.sql"))],
    ),
    (
        8,
        &[Step::Sql(include_str!(
            "migrations/0008_drop_log_barrier.sql"
        ))],
    ),
    (
        9,
        &[Step::Sql(include_str!(
            "migrations/0009_container_snoozes.sql"
        ))],
    ),
    (
        10,
        &[Step::Sql(include_str!(
            "migrations/0010_proposal_lists.sql"
        ))],
    ),
    (
        11,
        &[
            Step::Sql(include_str!("migrations/0011_logins.sql")),
            Step::Sql(include_str!("migrations/0011_identity_keys.sql")),
            Step::Identities,
            Step::Sql(include_str!("migrations/0011_identities.sql")),
        ],
    ),
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
    let recorded: Vec<i64> = rows(
        connection,
        "SELECT version FROM schema_migrations",
        Vec::new(),
    )
    .await?
    .iter()
    .map(|row| row.int(0))
    .collect::<Result<_, _>>()?;
    let pending: Vec<_> = MIGRATIONS
        .iter()
        .filter(|(number, _)| !recorded.contains(&i64::from(*number)))
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    execute(connection, "BEGIN", Vec::new()).await?;
    for (number, steps) in pending {
        let applied = async {
            for step in *steps {
                match step {
                    Step::Sql(sql) => connection
                        .execute_batch(sql)
                        .await
                        .map_err(SqlError::from)?,
                    Step::Identities => crate::to_identities::run(connection).await?,
                }
            }
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
