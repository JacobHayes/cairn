//! A journey's state rows, loaded into the written form of its `JourneyState`.

use std::collections::BTreeMap;

use cairn_store::StoreError;
use serde_json::{Map, Value as Json, json};
use turso::Connection;

use crate::sql::{self, rows, text};

/// Every state record of one graph, as the `state` field of its written form.
pub(crate) async fn load(connection: &Connection, id: &str) -> Result<Json, StoreError> {
    let mut state = Map::new();
    state.insert("nodes".to_owned(), node_states(connection, id).await?);
    state.insert("local_edits".to_owned(), local_edits(connection, id).await?);
    let answers = keyed(
        connection,
        "SELECT decision, value FROM answers WHERE graph_id = ?1",
        id,
        |row| row.json(1),
    )
    .await?;
    state.insert("answers".to_owned(), answers);
    state.insert("role_fills".to_owned(), role_fills(connection, id).await?);
    let pins = keyed(
        connection,
        "SELECT node, date FROM pins WHERE graph_id = ?1",
        id,
        |row| Ok(Json::String(row.text(1)?)),
    )
    .await?;
    state.insert("pins".to_owned(), pins);
    let select = "SELECT node, until_date, until_node FROM snoozes WHERE graph_id = ?1";
    let snoozes = keyed(connection, select, id, |row| {
        Ok(match row.opt_text(1)? {
            Some(date) => json!({"date": date}),
            None => json!({"node": row.text(2)?}),
        })
    })
    .await?;
    state.insert("snoozes".to_owned(), snoozes);
    let select = "SELECT node, force_include, keep, bypass FROM overrides WHERE graph_id = ?1";
    let overrides = keyed(connection, select, id, |row| {
        let mut overrides = Map::new();
        if let Some(reason) = row.opt_text(1)? {
            overrides.insert("force_include".to_owned(), Json::String(reason));
        }
        if let Some(reason) = row.opt_text(2)? {
            overrides.insert("keep".to_owned(), Json::String(reason));
        }
        if row.opt_text(3)?.is_some() {
            overrides.insert("bypass".to_owned(), row.json(3)?);
        }
        Ok(Json::Object(overrides))
    })
    .await?;
    state.insert("overrides".to_owned(), overrides);
    let select = "SELECT node FROM tombstones WHERE graph_id = ?1 ORDER BY node";
    let tombstones: Result<Vec<_>, StoreError> = rows(connection, select, vec![text(id)])
        .await?
        .iter()
        .map(|row| row.text(0).map(Json::String))
        .collect();
    state.insert("tombstones".to_owned(), Json::Array(tombstones?));
    state.insert("annotations".to_owned(), annotations(connection, id).await?);
    Ok(Json::Object(state))
}

/// A map from each row's first column to a value built from the row.
async fn keyed(
    connection: &Connection,
    select: &str,
    id: &str,
    value: impl Fn(&sql::Row) -> Result<Json, StoreError>,
) -> Result<Json, StoreError> {
    let mut map = Map::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        map.insert(row.text(0)?, value(&row)?);
    }
    Ok(Json::Object(map))
}

async fn node_states(connection: &Connection, id: &str) -> Result<Json, StoreError> {
    let select = "SELECT node, state, provenance, atomic, started_on, finished_on, \
                  skip_reason FROM node_states WHERE graph_id = ?1";
    keyed(connection, select, id, |row| {
        let mut state = json!({"state": row.text(1)?, "provenance": row.text(2)?});
        if row.int(3)? != 0 {
            state["atomic"] = Json::Bool(true);
        }
        if let Some(date) = row.opt_text(4)? {
            state["started_on"] = Json::String(date);
        }
        if let Some(date) = row.opt_text(5)? {
            state["finished_on"] = Json::String(date);
        }
        if let Some(reason) = row.opt_text(6)? {
            state["skip_reason"] = Json::String(reason);
        }
        Ok(state)
    })
    .await
}

async fn local_edits(connection: &Connection, id: &str) -> Result<Json, StoreError> {
    let mut edits: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    let select = "SELECT node, aspect, target FROM local_edits WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)]).await? {
        let (aspect, target) = (row.text(1)?, row.text(2)?);
        let edit = if target.is_empty() {
            Json::String(aspect)
        } else {
            let mut edit = Map::new();
            edit.insert(aspect, Json::String(target));
            Json::Object(edit)
        };
        edits.entry(row.text(0)?).or_default().push(edit);
    }
    Ok(Json::Object(
        edits
            .into_iter()
            .map(|(node, edits)| (node, Json::Array(edits)))
            .collect(),
    ))
}

async fn role_fills(connection: &Connection, id: &str) -> Result<Json, StoreError> {
    let mut fills: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    for row in rows(
        connection,
        "SELECT role FROM role_fills WHERE graph_id = ?1",
        vec![text(id)],
    )
    .await?
    {
        fills.insert(row.text(0)?, Vec::new());
    }
    let select = "SELECT role, entity FROM role_fill_entities WHERE graph_id = ?1 ORDER BY entity";
    for row in rows(connection, select, vec![text(id)]).await? {
        fills
            .entry(row.text(0)?)
            .or_default()
            .push(Json::String(row.text(1)?));
    }
    Ok(Json::Object(
        fills
            .into_iter()
            .map(|(role, entities)| (role, Json::Array(entities)))
            .collect(),
    ))
}

async fn annotations(connection: &Connection, id: &str) -> Result<Json, StoreError> {
    let select = "SELECT key, node, title, type, content, created_by, created_at, edited_at \
                  FROM annotations WHERE graph_id = ?1 ORDER BY key";
    let mut annotations = Vec::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        let mut body = json!({"key": row.text(0)?});
        if let Some(node) = row.opt_text(1)? {
            body["node"] = Json::String(node);
        }
        if let Some(title) = row.opt_text(2)? {
            body["title"] = Json::String(title);
        }
        body[row.text(3)?] = Json::String(row.text(4)?);
        let mut annotation = json!({
            "body": body,
            "created_by": row.text(5)?,
            "created_at": row.timestamp(6)?.to_string(),
        });
        if let Some(edited) = row.opt_int(7)? {
            annotation["edited_at"] = Json::String(sql::timestamp(edited)?.to_string());
        }
        annotations.push(annotation);
    }
    Ok(Json::Array(annotations))
}
