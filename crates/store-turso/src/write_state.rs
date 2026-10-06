//! Puts of journey state records, and removals by address inside a graph.

use cairn_schema::{GraphKey, GraphRecord, NodeKey};
use cairn_store::{CommitError, StoreError};
use serde_json::Value as Json;
use turso::Value;

use crate::sql::{Abort, date, flag, json, opt_text, text, time};
use crate::write::{
    Writer, answer_entities, json_enum, retired_columns, snooze_columns, written_content,
};

/// A statement and its parameters.
type Statement = (String, Vec<Value>);

fn malformed(reason: String) -> Abort {
    Abort::Answer(CommitError::Failed(StoreError::Malformed(reason)))
}

/// A local edit's aspect and target columns, from its written form. A marker without a
/// target (the shape) is written as its name with an empty target.
fn local_edit_columns(edit: &cairn_schema::LocalEdit) -> Result<(String, String), Abort> {
    let written = serde_json::to_value(edit)
        .map_err(|error| malformed(format!("a local edit does not serialize: {error}")))?;
    if let Json::String(aspect) = written {
        return Ok((aspect, String::new()));
    }
    if let Json::Object(fields) = written
        && let Some((aspect, Json::String(target))) = fields.into_iter().next()
    {
        return Ok((aspect, target));
    }
    Err(malformed(format!("local edit {edit:?} has no aspect")))
}

async fn run_all(writer: &Writer<'_>, statements: Vec<Statement>) -> Result<(), Abort> {
    for (sql, params) in statements {
        writer.statement(&sql, params).await?;
    }
    Ok(())
}

/// Puts a state record (or a retired key) into graph `id`.
pub(crate) async fn put(writer: &Writer<'_>, id: &str, record: &GraphRecord) -> Result<(), Abort> {
    let statements =
        match record {
            GraphRecord::Answer { decision, value } => {
                let mut statements =
                    vec![
                (
                    "INSERT OR REPLACE INTO answers (graph_id, decision, answer_type, value) \
                     VALUES (?1, ?2, ?3, ?4)"
                        .to_owned(),
                    vec![text(id), text(decision), json_enum(&value.answer_type())?, json(value)?],
                ),
                (
                    "DELETE FROM answer_entities WHERE graph_id = ?1 AND decision = ?2".to_owned(),
                    vec![text(id), text(decision)],
                ),
            ];
                for entity in answer_entities(value) {
                    statements.push((
                    "INSERT INTO answer_entities (graph_id, decision, entity) VALUES (?1, ?2, ?3)"
                        .to_owned(),
                    vec![text(id), text(decision), text(&entity)],
                ));
                }
                statements
            }
            GraphRecord::RoleFill { role, entities } => {
                let mut statements = vec![
                    (
                        "INSERT OR IGNORE INTO role_fills (graph_id, role) VALUES (?1, ?2)"
                            .to_owned(),
                        vec![text(id), text(role)],
                    ),
                    (
                        "DELETE FROM role_fill_entities WHERE graph_id = ?1 AND role = ?2"
                            .to_owned(),
                        vec![text(id), text(role)],
                    ),
                ];
                for entity in entities.iter() {
                    statements.push((
                    "INSERT INTO role_fill_entities (graph_id, role, entity) VALUES (?1, ?2, ?3)"
                        .to_owned(),
                    vec![text(id), text(role), text(entity)],
                ));
                }
                statements
            }
            GraphRecord::Annotation(annotation) => vec![annotation_put(id, annotation)?],
            other => vec![single_put(id, other)?],
        };
    run_all(writer, statements).await
}

/// The put of a state record held in one row.
fn single_put(id: &str, record: &GraphRecord) -> Result<Statement, Abort> {
    let graph = text(id);
    let (sql, params): (&str, Vec<Value>) = match record {
        GraphRecord::RetiredKey(key) => {
            let (kind, key) = retired_columns(key);
            (
                "INSERT OR IGNORE INTO retired_keys (graph_id, key, kind) VALUES (?1, ?2, ?3)",
                vec![graph, text(&key), text(kind)],
            )
        }
        GraphRecord::NodeState { node, state } => (
            "INSERT OR REPLACE INTO node_states (graph_id, node, state, provenance, atomic, \
             started_on, finished_on, skip_reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            vec![
                graph,
                text(node),
                json_enum(&state.state)?,
                json_enum(&state.provenance)?,
                flag(state.atomic),
                state.started_on.map_or(Value::Null, date),
                state.finished_on.map_or(Value::Null, date),
                opt_text(state.skip_reason.as_ref()),
            ],
        ),
        GraphRecord::LocalEdit { node, edit } => {
            let (aspect, target) = local_edit_columns(edit)?;
            (
                "INSERT OR IGNORE INTO local_edits (graph_id, node, aspect, target) \
                 VALUES (?1, ?2, ?3, ?4)",
                vec![graph, text(node), text(&aspect), text(&target)],
            )
        }
        GraphRecord::Pin { node, date: day } => (
            "INSERT OR REPLACE INTO pins (graph_id, node, date) VALUES (?1, ?2, ?3)",
            vec![graph, text(node), date(*day)],
        ),
        GraphRecord::Snooze { node, until } => {
            let (until_date, until_node) = snooze_columns(until);
            (
                "INSERT OR REPLACE INTO snoozes (graph_id, node, until_date, until_node) \
                 VALUES (?1, ?2, ?3, ?4)",
                vec![graph, text(node), until_date, until_node],
            )
        }
        GraphRecord::Overrides { node, overrides } => (
            "INSERT OR REPLACE INTO overrides (graph_id, node, force_include, keep, bypass) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                graph,
                text(node),
                opt_text(overrides.force_include.as_ref()),
                opt_text(overrides.keep.as_ref()),
                overrides
                    .bypass
                    .as_ref()
                    .map(json)
                    .transpose()?
                    .unwrap_or(Value::Null),
            ],
        ),
        GraphRecord::Tombstone(node) => (
            "INSERT OR IGNORE INTO tombstones (graph_id, node) VALUES (?1, ?2)",
            vec![graph, text(node)],
        ),
        content => {
            return Err(malformed(format!(
                "{content:?} is graph content, not state"
            )));
        }
    };
    Ok((sql.to_owned(), params))
}

fn annotation_put(id: &str, annotation: &cairn_schema::Annotation) -> Result<Statement, Abort> {
    let body = &annotation.body;
    let (kind, content) =
        written_content(body, &["note", "artifact", "reference", "conversation"])?;
    Ok((
        "INSERT OR REPLACE INTO annotations (graph_id, key, node, title, type, content, \
         created_by, created_at, edited_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            .to_owned(),
        vec![
            text(id),
            text(&body.key),
            opt_text(body.node.as_ref()),
            opt_text(body.title.as_ref()),
            text(kind),
            text(&content),
            text(&annotation.created_by),
            time(annotation.created_at)?,
            annotation
                .edited_at
                .map(time)
                .transpose()?
                .unwrap_or(Value::Null),
        ],
    ))
}

/// Removes the records an address inside graph `id` covers (`cairn_store::memory`).
pub(crate) async fn remove(writer: &Writer<'_>, id: &str, key: &GraphKey) -> Result<(), Abort> {
    run_all(writer, removal(id, key)?).await
}

/// The statements removing what `key` covers in graph `id`.
fn removal(id: &str, key: &GraphKey) -> Result<Vec<Statement>, Abort> {
    let graph = || text(id);
    Ok(match key {
        GraphKey::Node(node) => node_removal(id, node),
        GraphKey::NodeField { node, field } => {
            return Err(malformed(format!(
                "field {field:?} of {node} is written, not removed"
            )));
        }
        GraphKey::Edge(edge) => vec![(
            "DELETE FROM edges WHERE graph_id = ?1 AND node = ?2 AND requires = ?3".to_owned(),
            vec![graph(), text(&edge.node), text(&edge.requires)],
        )],
        GraphKey::DefaultOwner => vec![(
            "UPDATE graphs SET default_owner = NULL WHERE id = ?1".to_owned(),
            vec![graph()],
        )],
        GraphKey::Participation { node, kind } => ["participation_entities", "participations"]
            .iter()
            .map(|table| {
                (
                    format!("DELETE FROM {table} WHERE graph_id = ?1 AND node = ?2 AND kind = ?3"),
                    vec![graph(), text(node), text(kind)],
                )
            })
            .collect(),
        GraphKey::Resource { node, resource } => vec![(
            "DELETE FROM resources WHERE graph_id = ?1 AND node = ?2 AND key = ?3".to_owned(),
            vec![graph(), text(node), text(resource)],
        )],
        GraphKey::LocalEdit { node, edit } => {
            let (aspect, target) = local_edit_columns(edit)?;
            vec![(
                "DELETE FROM local_edits WHERE graph_id = ?1 AND node = ?2 AND aspect = ?3 \
                 AND target = ?4"
                    .to_owned(),
                vec![graph(), text(node), text(&aspect), text(&target)],
            )]
        }
        other => keyed_removal(id, other),
    })
}

/// Removals of records addressed by one key column.
fn keyed_removal(id: &str, key: &GraphKey) -> Vec<Statement> {
    let retired;
    let tables: Vec<(&str, &str, &str)> = match key {
        GraphKey::Role(role) => vec![("roles", "key", role.as_str())],
        GraphKey::Kind(kind) => vec![("participation_kinds", "key", kind.as_str())],
        GraphKey::RetiredKey(key) => {
            retired = retired_columns(key).1;
            vec![("retired_keys", "key", retired.as_str())]
        }
        GraphKey::NodeState(node) => vec![("node_states", "node", node.as_str())],
        GraphKey::Answer(node) => vec![
            ("answer_entities", "decision", node.as_str()),
            ("answers", "decision", node.as_str()),
        ],
        GraphKey::RoleFill(role) => vec![
            ("role_fill_entities", "role", role.as_str()),
            ("role_fills", "role", role.as_str()),
        ],
        GraphKey::Pin(node) => vec![("pins", "node", node.as_str())],
        GraphKey::Snooze(node) => vec![("snoozes", "node", node.as_str())],
        GraphKey::Overrides(node) => vec![("overrides", "node", node.as_str())],
        GraphKey::Tombstone(node) => vec![("tombstones", "node", node.as_str())],
        GraphKey::Annotation { annotation, .. } => {
            vec![("annotations", "key", annotation.as_str())]
        }
        // Handled by `removal`.
        GraphKey::Node(_)
        | GraphKey::NodeField { .. }
        | GraphKey::Edge(_)
        | GraphKey::DefaultOwner
        | GraphKey::Participation { .. }
        | GraphKey::Resource { .. }
        | GraphKey::LocalEdit { .. } => Vec::new(),
    };
    tables
        .into_iter()
        .map(|(table, column, value)| {
            (
                format!("DELETE FROM {table} WHERE graph_id = ?1 AND {column} = ?2"),
                vec![text(id), text(value)],
            )
        })
        .collect()
}

/// A whole node's removal: the node, the edges into it, and everything on it.
fn node_removal(id: &str, node: &NodeKey) -> Vec<Statement> {
    let params = || vec![text(id), text(node)];
    let mut statements = vec![(
        "DELETE FROM edges WHERE graph_id = ?1 AND (node = ?2 OR requires = ?2)".to_owned(),
        params(),
    )];
    let tables = [
        ("participation_entities", "node"),
        ("participations", "node"),
        ("resources", "node"),
        ("nodes", "key"),
        ("node_states", "node"),
        ("local_edits", "node"),
        ("answer_entities", "decision"),
        ("answers", "decision"),
        ("pins", "node"),
        ("snoozes", "node"),
        ("overrides", "node"),
        ("tombstones", "node"),
        ("annotations", "node"),
    ];
    for (table, column) in tables {
        statements.push((
            format!("DELETE FROM {table} WHERE graph_id = ?1 AND {column} = ?2"),
            params(),
        ));
    }
    statements
}
