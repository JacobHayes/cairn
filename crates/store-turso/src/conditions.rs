//! `condition_identities` (migration 0013): the identities each graph's conditions compare an
//! identity-typed decision with (E6), brought up to date for a graph whenever a commit writes its nodes' conditions or decisions,
//! so a decision retyped after the node holding the condition is read as it now stands. The
//! typed rule is `cairn_schema::condition_identities`, the one the engine and the memory
//! backend use.

use std::collections::BTreeSet;

use crate::sql::SqlError;
use cairn_schema::{Condition, KeyRefs, NodeKey, condition_identities};
use turso::Connection;

use crate::sql::{execute, rows, text};

/// Brings graph `graph`'s rows to what its nodes now say, writing only the rows that differ: a
/// condition edit or a retyped decision touches the few rows it changes, not the graph's whole
/// index.
pub(crate) async fn refresh(connection: &Connection, graph: &str) -> Result<(), SqlError> {
    let select = "SELECT key FROM nodes WHERE graph_id = ?1 \
                  AND answer_type IN ('identity', 'identity_list')";
    let mut identity_typed: BTreeSet<NodeKey> = BTreeSet::new();
    for row in rows(connection, select, vec![text(graph)]).await? {
        identity_typed.insert(row.parse(0)?);
    }
    let mut wanted: BTreeSet<(String, String)> = BTreeSet::new();
    if !identity_typed.is_empty() {
        let select = "SELECT key, relevant_when FROM nodes WHERE graph_id = ?1 \
                      AND relevant_when IS NOT NULL";
        for row in rows(connection, select, vec![text(graph)]).await? {
            let node = row.text(0)?;
            let condition: Condition<KeyRefs> = row.json(1)?;
            for identity in
                condition_identities(&condition, |decision| identity_typed.contains(decision))
            {
                wanted.insert((node.clone(), identity.to_string()));
            }
        }
    }
    let mut held: BTreeSet<(String, String)> = BTreeSet::new();
    let select = "SELECT node, identity FROM condition_identities WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(graph)]).await? {
        held.insert((row.text(0)?, row.text(1)?));
    }
    for (node, identity) in held.difference(&wanted) {
        execute(
            connection,
            "DELETE FROM condition_identities WHERE graph_id = ?1 AND node = ?2 AND identity = ?3",
            vec![text(graph), text(node), text(identity)],
        )
        .await?;
    }
    for (node, identity) in wanted.difference(&held) {
        execute(
            connection,
            "INSERT INTO condition_identities (graph_id, node, identity) VALUES (?1, ?2, ?3)",
            vec![text(graph), text(node), text(identity)],
        )
        .await?;
    }
    Ok(())
}

/// Migration 0013's code step: the rows of every graph that exists.
pub(crate) async fn backfill(connection: &Connection) -> Result<(), SqlError> {
    for row in rows(connection, "SELECT id FROM graphs", Vec::new()).await? {
        refresh(connection, &row.text(0)?).await?;
    }
    Ok(())
}
