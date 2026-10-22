//! What a migration's code step uses to rewrite stored JSON: each value of a column read,
//! changed by the step's own walk, and written back only when the walk changed it, so a rerun
//! changes nothing.

use rusqlite::Connection;
use serde_json::Value;

use crate::sql::{SqlError, execute, rows, text};

/// Renames the key `from` of `object` to `to`, keeping its value; whether `from` was there.
pub(crate) fn rename_key(
    object: &mut serde_json::Map<String, Value>,
    from: &str,
    to: &str,
) -> bool {
    match object.remove(from) {
        Some(value) => {
            object.insert(to.to_owned(), value);
            true
        }
        None => false,
    }
}

/// Rewrites the JSON `columns` of every row of `table`, found by `key`, with `rewrite`, which
/// says whether it changed the value.
pub(crate) fn column(
    connection: &Connection,
    table: &str,
    key: &str,
    columns: &[&str],
    rewrite: fn(&mut Value) -> bool,
) -> Result<(), SqlError> {
    for name in columns {
        let select = format!("SELECT {key}, {name} FROM {table}");
        let update = format!("UPDATE {table} SET {name} = ?1 WHERE {key} = ?2");
        for row in rows(connection, &select, Vec::new())? {
            let stored = row.text(1).map_err(failed)?;
            let mut value: Value = serde_json::from_str(&stored)
                .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
            if rewrite(&mut value) {
                let at = if table == "events" {
                    crate::sql::int(row.int(0).map_err(failed)?)
                } else {
                    text(&row.text(0).map_err(failed)?)
                };
                execute(connection, &update, vec![text(&value.to_string()), at])?;
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "passed to `map_err`, which hands over the error"
)]
pub(crate) fn failed(error: cairn_store::StoreError) -> SqlError {
    SqlError::Other(format!("{error:?}"))
}
