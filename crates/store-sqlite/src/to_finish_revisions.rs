//! The code step of migration 22: finishes and reopens are ordered by the journey revision
//! they were recorded at, not by their day, so every stored one gains its revision and a
//! reopen record keeps only its two revisions. Each journey's events are walked in order, and
//! every node state they write takes its revisions from the receipt of the patch that wrote
//! it: a finish takes the revision of the patch that first wrote it (a later write of the same
//! finish, such as an edit of its actual date, keeps it), and a reopen record takes the
//! revision of the patch that first wrote it with the revision of the finish it undid. The
//! events' JSON is rewritten, so replaying the log still gives the stored state, and each
//! stored node state takes what its last event wrote. A finish or reopen with no event that
//! wrote it, or first written by an event with no receipt, fails the migration: the log is
//! the record of every write, and guessing would hide that it is not.

use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::sql::{SqlError, execute, int, opt_int, rows, text};
use crate::stored_json::failed;

/// The revisions a node's state was last written with: its finish's, and its reopen's with
/// that of the finish it undid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Recorded {
    finished: Option<i64>,
    reopened: Option<(i64, i64)>,
}

/// Whether `state` holds `key` with a value.
fn holds(state: &Map<String, Value>, key: &str) -> bool {
    state.get(key).is_some_and(|value| !value.is_null())
}

/// Gives one node state, written after `before` by a patch whose receipt says `revision`,
/// its revisions.
fn rewrite_state(
    state: &mut Map<String, Value>,
    revision: Option<i64>,
    before: Recorded,
) -> Result<Recorded, SqlError> {
    let missing =
        || SqlError::Other("a finish or reopen was written by a patch with no receipt".to_owned());
    let finished = match (holds(state, "finished_on"), before.finished) {
        (false, _) => None,
        (true, Some(kept)) => Some(kept),
        (true, None) => Some(revision.ok_or_else(missing)?),
    };
    let reopened = match (holds(state, "reopened"), before.reopened) {
        (false, _) => None,
        (true, Some(kept)) => Some(kept),
        (true, None) => Some((
            revision.ok_or_else(missing)?,
            before.finished.ok_or_else(|| {
                SqlError::Other("a reopen record undoes a finish no event wrote".to_owned())
            })?,
        )),
    };
    match finished {
        Some(finished) => state.insert("finished_revision".to_owned(), finished.into()),
        None => state.remove("finished_revision"),
    };
    if let Some((revision, finished)) = reopened {
        state.insert(
            "reopened".to_owned(),
            serde_json::json!({"revision": revision, "finished_revision": finished}),
        );
    }
    Ok(Recorded { finished, reopened })
}

/// Each node state `delta` (a list of writes) puts, with its node's key.
fn written_states(delta: &mut Value) -> Vec<(String, &mut Map<String, Value>)> {
    let Value::Array(writes) = delta else {
        return Vec::new();
    };
    writes
        .iter_mut()
        .filter_map(|write| write.pointer_mut("/put/graph/record/node_state"))
        .filter_map(|written| {
            let node = written.get("node")?.as_str()?.to_owned();
            let Some(Value::Object(state)) = written.get_mut("state") else {
                return None;
            };
            Some((node, state))
        })
        .collect()
}

/// Runs the step: the journeys' events, then their stored node states.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    let mut recorded: HashMap<(String, String), Recorded> = HashMap::new();
    let select = "SELECT events.seq, events.log_id, events.delta, patch_receipts.revision \
                  FROM events LEFT JOIN patch_receipts \
                  ON patch_receipts.patch_id = events.patch_id \
                  WHERE events.log_kind = 'journey' ORDER BY events.seq";
    for row in rows(connection, select, Vec::new())? {
        let seq = row.int(0).map_err(failed)?;
        let journey = row.text(1).map_err(failed)?;
        let stored = row.text(2).map_err(failed)?;
        let revision = row.opt_int(3).map_err(failed)?;
        let mut delta: Value = serde_json::from_str(&stored)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        let before = delta.clone();
        for (node, state) in written_states(&mut delta) {
            let key = (journey.clone(), node);
            let last = recorded.get(&key).copied().unwrap_or_default();
            let next = rewrite_state(state, revision, last)?;
            recorded.insert(key, next);
        }
        if delta != before {
            execute(
                connection,
                "UPDATE events SET delta = ?1 WHERE seq = ?2",
                vec![text(&delta.to_string()), int(seq)],
            )?;
        }
    }
    let select = "SELECT graph_id, node, finished_on IS NOT NULL, reopened_on IS NOT NULL \
                  FROM node_states WHERE finished_on IS NOT NULL OR reopened_on IS NOT NULL";
    for row in rows(connection, select, Vec::new())? {
        let graph = row.text(0).map_err(failed)?;
        let node = row.text(1).map_err(failed)?;
        let journey = graph.strip_prefix("journey/").unwrap_or(&graph).to_owned();
        let last = recorded
            .get(&(journey, node.clone()))
            .copied()
            .unwrap_or_default();
        if last.finished.is_some() != row.flag(2).map_err(failed)?
            || last.reopened.is_some() != row.flag(3).map_err(failed)?
        {
            return Err(SqlError::Other(format!(
                "the stored state of {node} in {graph} is not what its events wrote"
            )));
        }
        execute(
            connection,
            "UPDATE node_states SET finished_revision = ?1, reopened_revision = ?2, \
             reopened_finished_revision = ?3 WHERE graph_id = ?4 AND node = ?5",
            vec![
                opt_int(last.finished),
                opt_int(last.reopened.map(|(revision, _)| revision)),
                opt_int(last.reopened.map(|(_, finished)| finished)),
                text(&graph),
                text(&node),
            ],
        )?;
    }
    Ok(())
}
