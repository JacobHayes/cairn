//! A graph's rows: loading every table of one graph into a [`Graph`], and the column layout
//! of a node. Values are rebuilt through their written (serde) form, so a row that no
//! longer parses is reported rather than loaded wrong.

use std::collections::BTreeMap;

use cairn_schema::{Graph, GraphId, Node, refs::KeyRefs};
use cairn_store::StoreError;
use serde_json::{Map, Value as Json, json};
use turso::{Connection, Value};

use crate::sql::{self, Row, SqlError, corrupt, rows, text};

/// How a node field is held in its column.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Column {
    Text,
    Integer,
    Flag,
    Json,
}

/// The nodes table's columns after `graph_id`, each with the written field it holds (A1a).
/// A node's `requires`, `participations`, and `resources` are rows of their own tables.
pub(crate) const NODE_COLUMNS: [(&str, &str, Column); 25] = [
    ("key", "key", Column::Text),
    ("parent_key", "parent", Column::Text),
    ("id", "id", Column::Text),
    ("kind", "kind", Column::Text),
    ("title", "title", Column::Text),
    ("description", "description", Column::Text),
    ("weight", "weight", Column::Integer),
    ("relevant_when", "relevant_when", Column::Json),
    ("due_by", "due_by", Column::Json),
    ("not_before", "not_before", Column::Json),
    ("estimate", "estimate", Column::Integer),
    ("placeholder", "placeholder", Column::Flag),
    ("requires_artifact", "requires_artifact", Column::Flag),
    ("is_final", "final", Column::Flag),
    ("auto_reach", "auto_reach", Column::Flag),
    ("opens_at", "opens_at", Column::Text),
    ("closes_at", "closes_at", Column::Text),
    ("gates", "gates", Column::Flag),
    ("closes", "closes", Column::Flag),
    ("prompt", "prompt", Column::Text),
    ("answer_type", "answer_type", Column::Text),
    ("choices", "choices", Column::Json),
    ("fills_role", "fills_role", Column::Text),
    ("feeds_milestone", "feeds_milestone", Column::Text),
    ("help", "help", Column::Text),
];

/// The node fields held in tables of their own.
pub(crate) const NODE_CHILD_FIELDS: [&str; 3] = ["requires", "participations", "resources"];

/// A node's written form, as a JSON object.
pub(crate) fn node_fields(node: &Node<KeyRefs>) -> Result<Map<String, Json>, StoreError> {
    match serde_json::to_value(node) {
        Ok(Json::Object(fields)) => Ok(fields),
        Ok(other) => Err(StoreError::Backend(format!("a node wrote {other}"))),
        Err(error) => Err(StoreError::Backend(format!(
            "a node does not serialize: {error}"
        ))),
    }
}

/// The values of a node's columns, in [`NODE_COLUMNS`] order.
///
/// # Errors
///
/// A field the layout has no column for: the layout must hold every field a node writes.
pub(crate) fn node_values(fields: &Map<String, Json>) -> Result<Vec<Value>, StoreError> {
    for name in fields.keys() {
        let mapped = NODE_COLUMNS.iter().any(|(_, field, _)| field == name)
            || NODE_CHILD_FIELDS.contains(&name.as_str());
        if !mapped {
            return Err(StoreError::Backend(format!(
                "node field {name} has no column"
            )));
        }
    }
    NODE_COLUMNS
        .iter()
        .map(|(_, field, column)| column_value(fields.get(*field), *column))
        .collect()
}

fn column_value(value: Option<&Json>, column: Column) -> Result<Value, StoreError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Value::Null);
    };
    let mismatch = || StoreError::Backend(format!("{value} does not fit a {column:?} column"));
    match column {
        Column::Text => value.as_str().map(text).ok_or_else(mismatch),
        Column::Integer => value.as_i64().map(Value::Integer).ok_or_else(mismatch),
        Column::Flag => value.as_bool().map(sql::flag).ok_or_else(mismatch),
        Column::Json => Ok(Value::Text(value.to_string())),
    }
}

fn column_json(row: &Row, index: usize, column: Column) -> Result<Option<Json>, StoreError> {
    Ok(match column {
        Column::Text => row.opt_text(index)?.map(Json::String),
        Column::Integer => row.opt_int(index)?.map(Json::from),
        Column::Flag => row.opt_int(index)?.map(|flag| Json::Bool(flag != 0)),
        Column::Json => row
            .opt_text(index)?
            .map(|text| serde_json::from_str(&text))
            .transpose()
            .map_err(|error| corrupt(&format!("column {index}: {error}")))?,
    })
}

fn from_json<T: serde::de::DeserializeOwned>(value: Json, what: &str) -> Result<T, StoreError> {
    serde_json::from_value(value).map_err(|error| corrupt(&format!("{what}: {error}")))
}

/// Loads every record of one graph. A graph with no rows loads empty.
pub(crate) async fn load(connection: &Connection, graph: &GraphId) -> Result<Graph, StoreError> {
    let id = sql::graph_id(graph);
    let mut children = Children::load(connection, &id, None).await?;
    let columns: Vec<_> = NODE_COLUMNS.iter().map(|(name, _, _)| *name).collect();
    let select = format!(
        "SELECT {} FROM nodes WHERE graph_id = ?1 ORDER BY key",
        columns.join(", ")
    );
    let mut nodes = Vec::new();
    for row in rows(connection, &select, vec![text(&id)]).await? {
        let mut fields = Map::new();
        for (index, (_, field, column)) in NODE_COLUMNS.iter().enumerate() {
            if let Some(value) = column_json(&row, index, *column)? {
                fields.insert((*field).to_owned(), value);
            }
        }
        let key = row.text(0)?;
        children.attach(&key, &mut fields);
        nodes.push(Json::Object(fields));
    }
    let mut document = Map::new();
    document.insert("nodes".to_owned(), Json::Array(nodes));
    load_content(connection, &id, &mut document).await?;
    document.insert(
        "state".to_owned(),
        crate::state::load(connection, &id).await?,
    );
    from_json(Json::Object(document), &format!("graph {id}"))
}

/// Roles, kinds, the default owner, and retired keys, into the graph's written form.
async fn load_content(
    connection: &Connection,
    id: &str,
    document: &mut Map<String, Json>,
) -> Result<(), SqlError> {
    let params = || vec![text(id)];
    for (table, field) in [
        ("roles", "roles"),
        ("participation_kinds", "participation_kinds"),
    ] {
        let select =
            format!("SELECT key, id, title, multi FROM {table} WHERE graph_id = ?1 ORDER BY key");
        let mut items = Vec::new();
        for row in rows(connection, &select, params()).await? {
            let mut item = json!({"key": row.text(0)?, "id": row.text(1)?});
            if let Some(title) = row.opt_text(2)? {
                item["title"] = Json::String(title);
            }
            if row.int(3)? != 0 {
                item["multi"] = Json::Bool(true);
            }
            items.push(item);
        }
        document.insert(field.to_owned(), Json::Array(items));
    }
    let owner = sql::first(
        connection,
        "SELECT default_owner FROM graphs WHERE id = ?1",
        params(),
    )
    .await?;
    if let Some(owner) = owner.map(|row| row.opt_text(0)).transpose()?.flatten() {
        document.insert("default_owner".to_owned(), Json::String(owner));
    }
    let mut retired: BTreeMap<&str, Vec<Json>> = BTreeMap::new();
    let select = "SELECT kind, key FROM retired_keys WHERE graph_id = ?1 ORDER BY key";
    for row in rows(connection, select, params()).await? {
        let list = match row.text(0)?.as_str() {
            "node" => "nodes",
            "role" => "roles",
            _ => "kinds",
        };
        retired
            .entry(list)
            .or_default()
            .push(Json::String(row.text(1)?));
    }
    let retired: Map<String, Json> = retired
        .into_iter()
        .map(|(list, keys)| (list.to_owned(), Json::Array(keys)))
        .collect();
    document.insert("retired_keys".to_owned(), Json::Object(retired));
    Ok(())
}

impl From<StoreError> for SqlError {
    fn from(error: StoreError) -> Self {
        SqlError::Other(error.to_string())
    }
}

/// A graph's edges, participations, and resources, by node.
struct Children {
    requires: BTreeMap<String, Vec<Json>>,
    participations: BTreeMap<String, Map<String, Json>>,
    resources: BTreeMap<String, Vec<Json>>,
}

impl Children {
    /// The children of every node of a graph, or of one node.
    async fn load(
        connection: &Connection,
        id: &str,
        node: Option<&str>,
    ) -> Result<Self, StoreError> {
        let params = || {
            let mut params = vec![text(id)];
            params.extend(node.map(text));
            params
        };
        let only = if node.is_some() { " AND node = ?2" } else { "" };
        let mut requires: BTreeMap<String, Vec<Json>> = BTreeMap::new();
        let select = format!(
            "SELECT node, requires FROM edges WHERE graph_id = ?1{only} ORDER BY node, requires"
        );
        for row in rows(connection, &select, params()).await? {
            requires
                .entry(row.text(0)?)
                .or_default()
                .push(Json::String(row.text(1)?));
        }
        let mut participations: BTreeMap<String, Map<String, Json>> = BTreeMap::new();
        let select =
            format!("SELECT node, kind, role FROM participations WHERE graph_id = ?1{only}");
        for row in rows(connection, &select, params()).await? {
            let source = match row.opt_text(2)? {
                Some(role) => Json::String(role),
                None => Json::Array(Vec::new()),
            };
            participations
                .entry(row.text(0)?)
                .or_default()
                .insert(row.text(1)?, source);
        }
        let select = format!(
            "SELECT node, kind, entity FROM participation_entities WHERE graph_id = ?1{only} \
             ORDER BY node, kind, entity"
        );
        for row in rows(connection, &select, params()).await? {
            let source = participations
                .entry(row.text(0)?)
                .or_default()
                .entry(row.text(1)?)
                .or_insert_with(|| Json::Array(Vec::new()));
            if let Json::Array(entities) = source {
                entities.push(Json::String(row.text(2)?));
            }
        }
        let mut resources: BTreeMap<String, Vec<Json>> = BTreeMap::new();
        let select = format!(
            "SELECT node, key, title, type, body FROM resources WHERE graph_id = ?1{only} \
             ORDER BY node, position"
        );
        for row in rows(connection, &select, params()).await? {
            let mut resource = json!({"key": row.text(1)?});
            if let Some(title) = row.opt_text(2)? {
                resource["title"] = Json::String(title);
            }
            resource[row.text(3)?] = Json::String(row.text(4)?);
            resources.entry(row.text(0)?).or_default().push(resource);
        }
        Ok(Self {
            requires,
            participations,
            resources,
        })
    }

    fn attach(&mut self, node: &str, fields: &mut Map<String, Json>) {
        if let Some(requires) = self.requires.remove(node) {
            fields.insert("requires".to_owned(), Json::Array(requires));
        }
        if let Some(participations) = self.participations.remove(node) {
            fields.insert("participations".to_owned(), Json::Object(participations));
        }
        if let Some(resources) = self.resources.remove(node) {
            fields.insert("resources".to_owned(), Json::Array(resources));
        }
    }
}

/// Loads one node, with its edges, participations, and resources.
pub(crate) async fn load_node(
    connection: &Connection,
    graph: &str,
    key: &str,
) -> Result<Option<Node<KeyRefs>>, StoreError> {
    let columns: Vec<_> = NODE_COLUMNS.iter().map(|(name, _, _)| *name).collect();
    let select = format!(
        "SELECT {} FROM nodes WHERE graph_id = ?1 AND key = ?2",
        columns.join(", ")
    );
    let Some(row) = sql::first(connection, &select, vec![text(graph), text(key)]).await? else {
        return Ok(None);
    };
    let mut fields = Map::new();
    for (index, (_, field, column)) in NODE_COLUMNS.iter().enumerate() {
        if let Some(value) = column_json(&row, index, *column)? {
            fields.insert((*field).to_owned(), value);
        }
    }
    let mut children = Children::load(connection, graph, Some(key)).await?;
    children.attach(key, &mut fields);
    from_json(Json::Object(fields), &format!("node {key}")).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_field_a_node_writes_has_a_column() {
        let graph = cairn_store::build::every_content_graph();
        for node in graph.nodes.values() {
            let fields = node_fields(node).unwrap();
            assert!(node_values(&fields).is_ok(), "{}", node.key);
        }
        let mut fields = node_fields(graph.nodes.values().next().unwrap()).unwrap();
        fields.insert("unmapped".to_owned(), Json::Bool(true));
        assert!(node_values(&fields).is_err());
    }

    #[test]
    fn node_columns_hold_their_values_by_kind() {
        let cases = [
            (json!("text"), Column::Text, Value::Text("text".to_owned())),
            (json!(7), Column::Integer, Value::Integer(7)),
            (json!(true), Column::Flag, Value::Integer(1)),
            (
                json!({"answered": "n_a"}),
                Column::Json,
                Value::Text(r#"{"answered":"n_a"}"#.to_owned()),
            ),
            (Json::Null, Column::Text, Value::Null),
        ];
        for (value, column, expected) in cases {
            assert_eq!(
                column_value(Some(&value), column).unwrap(),
                expected,
                "{value}"
            );
        }
        assert!(column_value(Some(&json!(1)), Column::Text).is_err());
    }
}
