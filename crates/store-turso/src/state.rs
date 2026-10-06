//! A journey's state rows, and the columns of the state records that split into several:
//! each built field by field, as `crate::graph` builds content.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use cairn_schema::{
    Annotation, AnnotationBody, AnnotationContent, BoundedSet, EntityKey, JourneyState, Keyed,
    LocalEdit, NodeState, Overrides, RoleKey, SnoozeTarget,
};
use cairn_store::StoreError;
use turso::Connection;

use crate::sql::{Row, corrupt, enum_from, enum_name, parse, rows, text};

/// Every state record of one graph.
pub(crate) async fn load(connection: &Connection, id: &str) -> Result<JourneyState, StoreError> {
    let select = "SELECT node, state, provenance, atomic, started_on, finished_on, skip_reason \
                  FROM node_states WHERE graph_id = ?1";
    let nodes = keyed(connection, select, id, |row| {
        Ok(NodeState {
            state: row.name(1)?,
            provenance: row.name(2)?,
            atomic: row.flag(3)?,
            started_on: row.opt_parse(4)?,
            finished_on: row.opt_parse(5)?,
            skip_reason: row.opt_parse(6)?,
        })
    })
    .await?;
    let select = "SELECT decision, value FROM answers WHERE graph_id = ?1";
    let answers = keyed(connection, select, id, |row| row.json(1)).await?;
    let select = "SELECT node, date FROM pins WHERE graph_id = ?1";
    let pins = keyed(connection, select, id, |row| row.parse(1)).await?;
    let select = "SELECT node, until_date, until_node FROM snoozes WHERE graph_id = ?1";
    let snoozes = keyed(connection, select, id, |row| {
        match (row.opt_parse(1)?, row.opt_parse(2)?) {
            (Some(date), None) => Ok(SnoozeTarget::Date(date)),
            (None, Some(node)) => Ok(SnoozeTarget::Node(node)),
            (Some(_), Some(_)) | (None, None) => Err(corrupt("a snooze has one target")),
        }
    })
    .await?;
    let select = "SELECT node, force_include, keep, bypass FROM overrides WHERE graph_id = ?1";
    let overrides = keyed(connection, select, id, |row| {
        Ok(Overrides {
            force_include: row.opt_parse(1)?,
            keep: row.opt_parse(2)?,
            bypass: row.opt_json(3)?,
        })
    })
    .await?;
    let select = "SELECT node FROM tombstones WHERE graph_id = ?1";
    let mut tombstones = BTreeSet::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        tombstones.insert(row.parse(0)?);
    }
    Ok(JourneyState {
        nodes,
        local_edits: local_edits(connection, id).await?,
        answers,
        role_fills: role_fills(connection, id).await?,
        pins,
        snoozes,
        overrides,
        tombstones,
        annotations: annotations(connection, id).await?,
    })
}

/// A map from each row's first column to a value built from the row.
async fn keyed<K: FromStr + Ord, V>(
    connection: &Connection,
    select: &str,
    id: &str,
    value: impl Fn(&Row) -> Result<V, StoreError>,
) -> Result<BTreeMap<K, V>, StoreError>
where
    K::Err: std::fmt::Display,
{
    let mut map = BTreeMap::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        map.insert(row.parse(0)?, value(&row)?);
    }
    Ok(map)
}

/// A local edit's aspect and target columns; a marker without a target (the shape) has an
/// empty target.
pub(crate) fn local_edit_columns(edit: &LocalEdit) -> Result<(&'static str, String), StoreError> {
    Ok(match edit {
        LocalEdit::Field(field) => ("field", enum_name(field)?),
        LocalEdit::Requires(node) => ("requires", node.to_string()),
        LocalEdit::Participation(kind) => ("participation", kind.to_string()),
        LocalEdit::Resource(resource) => ("resource", resource.to_string()),
        LocalEdit::Shape => ("shape", String::new()),
    })
}

fn local_edit_from(aspect: &str, target: &str) -> Result<LocalEdit, StoreError> {
    Ok(match (aspect, target) {
        ("field", field) => LocalEdit::Field(enum_from(field)?),
        ("requires", node) => LocalEdit::Requires(parse(node)?),
        ("participation", kind) => LocalEdit::Participation(parse(kind)?),
        ("resource", resource) => LocalEdit::Resource(parse(resource)?),
        ("shape", "") => LocalEdit::Shape,
        (aspect, target) => return Err(corrupt(&format!("local edit {aspect} {target:?}"))),
    })
}

async fn local_edits(
    connection: &Connection,
    id: &str,
) -> Result<BTreeMap<cairn_schema::NodeKey, BTreeSet<LocalEdit>>, StoreError> {
    let mut edits: BTreeMap<_, BTreeSet<LocalEdit>> = BTreeMap::new();
    let select = "SELECT node, aspect, target FROM local_edits WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)]).await? {
        let edit = local_edit_from(&row.text(1)?, &row.text(2)?)?;
        edits.entry(row.parse(0)?).or_default().insert(edit);
    }
    Ok(edits)
}

async fn role_fills(
    connection: &Connection,
    id: &str,
) -> Result<BTreeMap<RoleKey, cairn_schema::EntitySet>, StoreError> {
    let mut entities: BTreeMap<RoleKey, BTreeSet<EntityKey>> = BTreeMap::new();
    let select = "SELECT role, entity FROM role_fill_entities WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)]).await? {
        entities
            .entry(row.parse(0)?)
            .or_default()
            .insert(row.parse(1)?);
    }
    let mut fills = BTreeMap::new();
    let select = "SELECT role FROM role_fills WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)]).await? {
        let role: RoleKey = row.parse(0)?;
        let filled = BoundedSet::new(entities.remove(&role).unwrap_or_default())
            .map_err(|error| corrupt(&format!("{role} fill: {error}")))?;
        fills.insert(role, filled);
    }
    match entities.keys().next() {
        Some(role) => Err(corrupt(&format!("entities fill {role}, which has no fill"))),
        None => Ok(fills),
    }
}

/// An annotation's `type` and `content` columns (G1).
pub(crate) fn annotation_columns(content: &AnnotationContent) -> (&'static str, String) {
    match content {
        AnnotationContent::Note(text) => ("note", text.to_string()),
        AnnotationContent::Artifact(url) => ("artifact", url.to_string()),
        AnnotationContent::Reference(url) => ("reference", url.to_string()),
        AnnotationContent::Conversation(url) => ("conversation", url.to_string()),
    }
}

fn annotation_content(kind: &str, content: &str) -> Result<AnnotationContent, StoreError> {
    Ok(match kind {
        "note" => AnnotationContent::Note(parse(content)?),
        "artifact" => AnnotationContent::Artifact(parse(content)?),
        "reference" => AnnotationContent::Reference(parse(content)?),
        "conversation" => AnnotationContent::Conversation(parse(content)?),
        other => return Err(corrupt(&format!("annotation type {other:?}"))),
    })
}

async fn annotations(
    connection: &Connection,
    id: &str,
) -> Result<Keyed<Annotation, cairn_schema::collections::ByDocumentSize>, StoreError> {
    let select = "SELECT key, node, title, type, content, created_by, created_at, edited_at \
                  FROM annotations WHERE graph_id = ?1";
    let mut annotations = Vec::new();
    for row in rows(connection, select, vec![text(id)]).await? {
        annotations.push(Annotation {
            body: AnnotationBody {
                key: row.parse(0)?,
                node: row.opt_parse(1)?,
                title: row.opt_parse(2)?,
                content: annotation_content(&row.text(3)?, &row.text(4)?)?,
            },
            created_by: row.parse(5)?,
            created_at: row.timestamp(6)?,
            edited_at: row.opt_timestamp(7)?,
        });
    }
    Keyed::new(annotations).map_err(|error| corrupt(&format!("annotations: {error}")))
}
