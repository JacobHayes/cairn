//! Puts of journey state records, and removals by address inside a graph.

use cairn_schema::{
    Annotation, AnnotationBody, GraphKey, GraphRecord, Markdown, NodeKey, NodeState, Overrides,
};
use cairn_store::{CommitError, StoreError};
use turso::Value;

use crate::sql::{Abort, date, flag, json, json_enum, opt_json, opt_text, opt_time, text, time};
use crate::state::{annotation_columns, local_edit_columns};
use crate::write::{Writer, answer_entities, retired_columns, snooze_columns};

/// A statement and its parameters.
type Statement = (String, Vec<Value>);

fn malformed(reason: String) -> Abort {
    Abort::Answer(CommitError::Failed(StoreError::Malformed(reason)))
}

async fn run_all(writer: &Writer<'_>, statements: Vec<Statement>) -> Result<(), Abort> {
    for (sql, params) in statements {
        writer.statement(&sql, params).await?;
    }
    Ok(())
}

/// Puts a state record (or a retired key) into graph `id`.
pub(crate) async fn put(writer: &Writer<'_>, id: &str, record: &GraphRecord) -> Result<(), Abort> {
    run_all(writer, puts(id, record)?).await
}

/// One statement.
fn statement(sql: &str, params: Vec<Value>) -> Statement {
    (sql.to_owned(), params)
}

/// The statements putting a state record (or a retired key) into graph `id`.
fn puts(id: &str, record: &GraphRecord) -> Result<Vec<Statement>, Abort> {
    let graph = text(id);
    Ok(match record {
        GraphRecord::RetiredKey(key) => {
            let (kind, key) = retired_columns(key);
            vec![statement(
                "INSERT OR IGNORE INTO retired_keys (graph_id, key, kind) VALUES (?1, ?2, ?3)",
                vec![graph, text(&key), text(kind)],
            )]
        }
        GraphRecord::NodeState { node, state } => vec![node_state_put(id, node, state)?],
        GraphRecord::LocalEdit { node, edit } => {
            let (aspect, target) = local_edit_columns(edit)?;
            vec![statement(
                "INSERT OR IGNORE INTO local_edits (graph_id, node, aspect, target) \
                 VALUES (?1, ?2, ?3, ?4)",
                vec![graph, text(node), text(aspect), text(&target)],
            )]
        }
        GraphRecord::Answer { .. } => answer_puts(id, record)?,
        GraphRecord::RoleFill { role, entities } => {
            let params = || vec![text(id), text(role)];
            let mut statements = vec![
                statement(
                    "INSERT OR IGNORE INTO role_fills (graph_id, role) VALUES (?1, ?2)",
                    params(),
                ),
                statement(
                    "DELETE FROM role_fill_entities WHERE graph_id = ?1 AND role = ?2",
                    params(),
                ),
            ];
            for entity in entities.iter() {
                let mut values = params();
                values.push(text(entity));
                statements.push(statement(
                    "INSERT INTO role_fill_entities (graph_id, role, entity) VALUES (?1, ?2, ?3)",
                    values,
                ));
            }
            statements
        }
        GraphRecord::Pin { node, date: day } => vec![statement(
            "INSERT OR REPLACE INTO pins (graph_id, node, date) VALUES (?1, ?2, ?3)",
            vec![graph, text(node), date(*day)],
        )],
        GraphRecord::Snooze { node, until } => {
            let (until_date, until_node) = snooze_columns(until);
            vec![statement(
                "INSERT OR REPLACE INTO snoozes (graph_id, node, until_date, until_node) \
                 VALUES (?1, ?2, ?3, ?4)",
                vec![graph, text(node), until_date, until_node],
            )]
        }
        GraphRecord::Overrides { node, overrides } => vec![overrides_put(id, node, overrides)?],
        GraphRecord::Tombstone(node) => vec![statement(
            "INSERT OR IGNORE INTO tombstones (graph_id, node) VALUES (?1, ?2)",
            vec![graph, text(node)],
        )],
        GraphRecord::Annotation(annotation) => vec![annotation_put(id, annotation)?],
        GraphRecord::Node(_)
        | GraphRecord::NodeField { .. }
        | GraphRecord::Edge(_)
        | GraphRecord::Role(_)
        | GraphRecord::Kind(_)
        | GraphRecord::DefaultOwner(_)
        | GraphRecord::Participation { .. }
        | GraphRecord::Resource { .. } => {
            return Err(malformed(format!("{record:?} is graph content, not state")));
        }
    })
}

fn node_state_put(id: &str, node: &NodeKey, state: &NodeState) -> Result<Statement, Abort> {
    let NodeState {
        state,
        provenance,
        atomic,
        started_on,
        finished_on,
        skip_reason,
    } = state;
    Ok(statement(
        "INSERT OR REPLACE INTO node_states (graph_id, node, state, provenance, atomic, \
         started_on, finished_on, skip_reason) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        vec![
            text(id),
            text(node),
            json_enum(state)?,
            json_enum(provenance)?,
            flag(*atomic),
            started_on.map_or(Value::Null, date),
            finished_on.map_or(Value::Null, date),
            opt_text(skip_reason.as_ref()),
        ],
    ))
}

fn answer_puts(id: &str, answer: &GraphRecord) -> Result<Vec<Statement>, Abort> {
    let GraphRecord::Answer {
        decision,
        value,
        rationale,
    } = answer
    else {
        unreachable!("an answer record")
    };
    let params = || vec![text(id), text(decision)];
    let mut values = params();
    // The whole row is replaced, so an answer without a rationale stores null (B2).
    values.extend([
        json_enum(&value.answer_type())?,
        json(value)?,
        opt_text(rationale.as_ref().map(Markdown::as_str)),
    ]);
    let mut statements = vec![
        statement(
            "INSERT OR REPLACE INTO answers (graph_id, decision, answer_type, value, rationale) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            values,
        ),
        statement(
            "DELETE FROM answer_entities WHERE graph_id = ?1 AND decision = ?2",
            params(),
        ),
    ];
    for entity in answer_entities(value) {
        let mut values = params();
        values.push(text(&entity));
        statements.push(statement(
            "INSERT INTO answer_entities (graph_id, decision, entity) VALUES (?1, ?2, ?3)",
            values,
        ));
    }
    Ok(statements)
}

fn overrides_put(id: &str, node: &NodeKey, overrides: &Overrides) -> Result<Statement, Abort> {
    let Overrides {
        force_include,
        keep,
        bypass,
    } = overrides;
    Ok(statement(
        "INSERT OR REPLACE INTO overrides (graph_id, node, force_include, keep, bypass) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        vec![
            text(id),
            text(node),
            opt_text(force_include.as_ref()),
            opt_text(keep.as_ref()),
            opt_json(bypass.as_ref())?,
        ],
    ))
}

fn annotation_put(id: &str, annotation: &Annotation) -> Result<Statement, Abort> {
    let Annotation {
        body:
            AnnotationBody {
                key,
                node,
                title,
                content,
            },
        created_by,
        created_at,
        edited_at,
    } = annotation;
    let (kind, content) = annotation_columns(content);
    Ok((
        "INSERT OR REPLACE INTO annotations (graph_id, key, node, title, type, content, \
         created_by, created_at, edited_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
            .to_owned(),
        vec![
            text(id),
            text(key),
            opt_text(node.as_ref()),
            opt_text(title.as_ref()),
            text(kind),
            text(&content),
            text(created_by),
            time(*created_at)?,
            opt_time(*edited_at)?,
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
                vec![graph(), text(node), text(aspect), text(&target)],
            )]
        }
        GraphKey::Role(_)
        | GraphKey::Kind(_)
        | GraphKey::RetiredKey(_)
        | GraphKey::NodeState(_)
        | GraphKey::Answer(_)
        | GraphKey::RoleFill(_)
        | GraphKey::Pin(_)
        | GraphKey::Snooze(_)
        | GraphKey::Overrides(_)
        | GraphKey::Tombstone(_)
        | GraphKey::Annotation { .. } => keyed_removal(id, key),
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
        GraphKey::Annotation {
            annotation,
            node: _,
        } => {
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
