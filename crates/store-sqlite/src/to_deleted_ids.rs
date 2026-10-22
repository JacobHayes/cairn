//! The code step of migration 0016: a hard delete is now the deployment's, and the record of a
//! deleted id names a journey or a route. Only the positions that hold the old shapes change,
//! so a user's text or map that happens to use the same words is left alone: a deleted
//! journey's record put by an event (`{deleted_journey: {journey, deleted_at}}` becomes
//! `{deleted: {id: {journey}, deleted_at}}`), its address in a receipt's touched set
//! (`{deleted_journey: j}` becomes `{deleted: {journey: j}}`), and a journey's own
//! `{op: delete_journey}` in a proposal, stored or put by an event, which now names the journey
//! and its revision (the proposal's destination and the revision it was drafted against). Such a proposal no longer applies to its journey; it reads back, and an
//! open one is refused when applied. A value already in the new shapes is left as it is.

use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::sql::{SqlError, execute, rows, text};
use crate::stored_json::{column, failed};

/// A deleted journey's record, `{deleted_journey: {journey, deleted_at}}`, as
/// `{deleted: {id: {journey}, deleted_at}}`; whether `record` was one.
fn rewrite_record(record: &mut Value) -> bool {
    let Value::Object(object) = record else {
        return false;
    };
    if object.len() != 1 {
        return false;
    }
    let Some(Value::Object(mut fields)) = object.remove("deleted_journey") else {
        return false;
    };
    let journey = fields.remove("journey").unwrap_or(Value::Null);
    fields.insert("id".to_owned(), journey_id(journey));
    object.insert("deleted".to_owned(), Value::Object(fields));
    true
}

/// A deleted journey's address, `{deleted_journey: j}`, as `{deleted: {journey: j}}`; whether
/// `key` was one.
fn rewrite_key(key: &mut Value) -> bool {
    let Value::Object(object) = key else {
        return false;
    };
    if object.len() != 1 {
        return false;
    }
    let Some(journey) = object.remove("deleted_journey") else {
        return false;
    };
    object.insert("deleted".to_owned(), journey_id(journey));
    true
}

fn journey_id(journey: Value) -> Value {
    let mut id = Map::new();
    id.insert("journey".to_owned(), journey);
    Value::Object(id)
}

/// An event's writes: the deleted journey's record it puts, and the journeys' proposals it
/// puts, whose deletions now name their journey; whether any changed.
fn rewrite_delta(delta: &mut Value) -> bool {
    let Value::Array(writes) = delta else {
        return false;
    };
    let mut changed = false;
    for write in writes {
        let Some(put) = write.get_mut("put") else {
            continue;
        };
        changed |= rewrite_record(put);
        if let Some(proposal) = put.get_mut("proposal") {
            changed |= name_proposal_deletions(proposal);
        }
    }
    changed
}

/// A receipt's touched set: the deleted journeys' addresses; whether any changed.
fn rewrite_touched(touched: &mut Value) -> bool {
    let Value::Array(keys) = touched else {
        return false;
    };
    let mut changed = false;
    for key in keys {
        changed |= rewrite_key(key);
    }
    changed
}

/// A stored proposal record to a journey: its draft's deletions, given the proposal's
/// destination and the revision the draft was made against; whether any changed.
fn name_proposal_deletions(proposal: &mut Value) -> bool {
    let Some(journey) = proposal
        .pointer("/destination/journey")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return false;
    };
    let Some(revision) = proposal
        .pointer("/draft/destination_revision")
        .and_then(Value::as_i64)
    else {
        return false;
    };
    proposal
        .pointer_mut("/draft/mutations")
        .is_some_and(|mutations| name_deletions(mutations, &journey, revision))
}

/// A journey proposal's `{op: delete_journey}` mutations, given the journey and revision they
/// meant; whether any changed.
fn name_deletions(mutations: &mut Value, journey: &str, revision: i64) -> bool {
    let Value::Array(items) = mutations else {
        return false;
    };
    let mut changed = false;
    for item in items {
        if let Value::Object(mutation) = item
            && mutation.len() == 1
            && mutation.get("op").and_then(Value::as_str) == Some("delete_journey")
        {
            mutation.insert("journey".to_owned(), Value::String(journey.to_owned()));
            mutation.insert("revision".to_owned(), Value::from(revision));
            changed = true;
        }
    }
    changed
}

/// Runs the step over every stored JSON value that can hold an old shape.
pub(crate) fn run(connection: &Connection) -> Result<(), SqlError> {
    column(connection, "events", "seq", &["delta"], rewrite_delta)?;
    column(
        connection,
        "patch_receipts",
        "patch_id",
        &["deployment_touched"],
        rewrite_touched,
    )?;
    let select = "SELECT id, destination_id, destination_revision, mutations FROM proposals \
                  WHERE destination_kind = 'journey'";
    for row in rows(connection, select, Vec::new())? {
        let stored = row.text(3).map_err(failed)?;
        let mut mutations: Value = serde_json::from_str(&stored)
            .map_err(|error| SqlError::Other(format!("stored JSON: {error}")))?;
        let journey = row.text(1).map_err(failed)?;
        let revision = row.int(2).map_err(failed)?;
        if name_deletions(&mut mutations, &journey, revision) {
            execute(
                connection,
                "UPDATE proposals SET mutations = ?1 WHERE id = ?2",
                vec![
                    text(&mutations.to_string()),
                    text(&row.text(0).map_err(failed)?),
                ],
            )?;
        }
    }
    Ok(())
}
