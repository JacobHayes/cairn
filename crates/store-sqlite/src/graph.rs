//! A graph's rows: a node's columns, and loading every table of one graph into a [`Graph`].
//!
//! Each codec here destructures the schema value it splits into columns and builds the
//! value it loads field by field, with no `..` and no catch-all arm over a schema enum, so a
//! field or variant added to the schema does not compile until it has a column (PRACTICES,
//! Shell: conformance and integration). Text names back to variants are the one direction
//! the compiler cannot check; the every-field and every-variant conformance cases do.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::collections::{InsertionCount, LimitOf};
use cairn_schema::{
    AnswerType, BoundedSet, Days, Graph, GraphId, HasKey, IdentityKey, Insertion, Keyed,
    KindColumns, KindKey, Lineage, Node, NodeKey, NodeKind, ParticipationKind, ParticipationSource,
    Participations, Payload, Resource, ResourceContent, RetiredKeys, Role, RoleKey, Weight,
    node::Requires, refs::KeyRefs,
};
use cairn_store::StoreError;
use rusqlite::Connection;
use rusqlite::types::Value;

use crate::sql::{
    self, Row, SqlError, corrupt, flag, opt_int, opt_json, opt_text, parse, rows, text,
};

/// The nodes table's columns after `graph_id`, in the order [`node_values`] writes them and
/// [`node_from_row`] reads them (A1a): the fields every kind shares, then [`KindColumns`].
/// A node's `requires`, `participations`, and `resources` are rows of their own tables.
pub(crate) const NODE_COLUMNS: [&str; 26] = [
    "key",
    "parent_key",
    "id",
    "kind",
    "title",
    "description",
    "weight",
    "relevant_when",
    "due_by",
    "not_before",
    "estimate",
    "placeholder",
    "requires_artifact",
    "requires_note",
    "is_final",
    "auto_done",
    "opens_at",
    "closes_at",
    "gates",
    "closes",
    "prompt",
    "answer_type",
    "choices",
    "fills_role",
    "feeds_milestone",
    "help",
];

/// Where [`KindColumns`] start in [`NODE_COLUMNS`].
const KIND_COLUMNS_START: usize = 10;

/// The values of a node's [`NODE_COLUMNS`], in order.
///
/// `requires`, `participations`, and `resources` are rows of their own tables, which
/// `Writer::put_node` writes from the same node.
pub(crate) fn node_values(node: &Node<KeyRefs>) -> Result<Vec<Value>, StoreError> {
    let Node {
        key,
        id,
        parent,
        title,
        description,
        weight,
        requires: _,
        relevant_when,
        due_by,
        not_before,
        participations: _,
        resources: _,
        payload,
    } = node;
    let mut values = vec![
        text(key),
        opt_text(parent.as_ref()),
        text(id),
        text(payload.kind().name()),
        text(title),
        opt_text(description.as_ref()),
        opt_int(weight.map(Weight::get)),
        opt_json(relevant_when.as_ref())?,
        opt_json(due_by.as_ref())?,
        opt_json(not_before.as_ref())?,
    ];
    values.extend(kind_values(payload)?);
    Ok(values)
}

/// The values of a node's kind-specific columns, in [`NODE_COLUMNS`] order: each null unless
/// the node's kind, and for a decision its answer type, has the field (A1a). Flags are
/// always written.
fn kind_values(payload: &Payload<KeyRefs>) -> Result<Vec<Value>, StoreError> {
    let KindColumns {
        estimate,
        placeholder,
        requires_artifact,
        requires_note,
        is_final,
        auto_done,
        opens_at,
        closes_at,
        gates,
        closes,
        prompt,
        answer_type,
        choices,
        fills_role,
        feeds_milestone,
        help,
    } = KindColumns::of(payload);
    let opt_flag = |value: Option<bool>| value.map_or(Value::Null, flag);
    Ok(vec![
        opt_int(estimate.map(Days::get)),
        opt_flag(placeholder),
        opt_flag(requires_artifact),
        opt_flag(requires_note),
        opt_flag(is_final),
        opt_flag(auto_done),
        opt_text(opens_at),
        opt_text(closes_at),
        opt_flag(gates),
        opt_flag(closes),
        opt_text(prompt),
        opt_text(answer_type.map(AnswerType::name)),
        opt_json(choices.as_ref())?,
        opt_text(fills_role),
        opt_text(feeds_milestone),
        opt_text(help),
    ])
}

/// The payload of a node of `kind` from the kind-specific columns of a nodes row. A null
/// flag is its default, as the written form leaves it out.
///
/// # Errors
///
/// A column the kind or answer type cannot have, or a missing required one (A1a).
fn payload_from_row(row: &Row, kind: NodeKind) -> Result<Payload<KeyRefs>, StoreError> {
    let at = |offset: usize| KIND_COLUMNS_START + offset;
    let columns = KindColumns {
        estimate: row.opt_int(at(0))?.map(sql::number).transpose()?,
        placeholder: row.opt_flag(at(1))?,
        requires_artifact: row.opt_flag(at(2))?,
        requires_note: row.opt_flag(at(3))?,
        is_final: row.opt_flag(at(4))?,
        auto_done: row.opt_flag(at(5))?,
        opens_at: row.opt_parse(at(6))?,
        closes_at: row.opt_parse(at(7))?,
        gates: row.opt_flag(at(8))?,
        closes: row.opt_flag(at(9))?,
        prompt: row.opt_parse(at(10))?,
        answer_type: row
            .opt_text(at(11))?
            .map(|name| answer_type_from(&name))
            .transpose()?,
        choices: row.opt_json(at(12))?,
        fills_role: row.opt_parse(at(13))?,
        feeds_milestone: row.opt_parse(at(14))?,
        help: row.opt_parse(at(15))?,
    };
    columns
        .into_payload(kind)
        .map_err(|error| corrupt(&error.to_string()))
}

fn kind_from(name: &str) -> Result<NodeKind, StoreError> {
    NodeKind::ALL
        .into_iter()
        .find(|kind| kind.name() == name)
        .ok_or_else(|| corrupt(&format!("node kind {name:?}")))
}

fn answer_type_from(name: &str) -> Result<AnswerType, StoreError> {
    AnswerType::ALL
        .into_iter()
        .find(|answer| answer.name() == name)
        .ok_or_else(|| corrupt(&format!("answer type {name:?}")))
}

/// A resource's `type` and `body` columns (A10).
pub(crate) fn resource_columns(content: &ResourceContent<KeyRefs>) -> (&'static str, String) {
    match content {
        ResourceContent::Tip(body) => ("tip", body.to_string()),
        ResourceContent::Template(url) => ("template", url.to_string()),
        ResourceContent::Example(url) => ("example", url.to_string()),
        ResourceContent::Reference(url) => ("reference", url.to_string()),
        ResourceContent::MessageDraft(template) => ("message_draft", template.to_string()),
    }
}

fn resource_content(kind: &str, body: &str) -> Result<ResourceContent<KeyRefs>, StoreError> {
    Ok(match kind {
        "tip" => ResourceContent::Tip(parse(body)?),
        "template" => ResourceContent::Template(parse(body)?),
        "example" => ResourceContent::Example(parse(body)?),
        "reference" => ResourceContent::Reference(parse(body)?),
        "message_draft" => ResourceContent::MessageDraft(parse(body)?),
        other => return Err(corrupt(&format!("resource type {other:?}"))),
    })
}

fn node_select(filter: &str) -> String {
    format!(
        "SELECT {} FROM nodes WHERE graph_id = ?1{filter} ORDER BY key",
        NODE_COLUMNS.join(", ")
    )
}

/// A node from its [`NODE_COLUMNS`] row and its children.
fn node_from_row(row: &Row, children: &mut Children) -> Result<Node<KeyRefs>, StoreError> {
    let key: NodeKey = row.parse(0)?;
    let kind = kind_from(&row.text(3)?)?;
    Ok(Node {
        requires: children.requires(&key)?,
        participations: children.participations(&key)?,
        resources: children.resources.remove(&key).unwrap_or_default(),
        key,
        parent: row.opt_parse(1)?,
        id: row.parse(2)?,
        title: row.parse(4)?,
        description: row.opt_parse(5)?,
        weight: row.opt_int(6)?.map(sql::number).transpose()?,
        relevant_when: row.opt_json(7)?,
        due_by: row.opt_json(8)?,
        not_before: row.opt_json(9)?,
        payload: payload_from_row(row, kind)?,
    })
}

fn keyed<V: HasKey, L: LimitOf>(values: Vec<V>, what: &str) -> Result<Keyed<V, L>, StoreError> {
    Keyed::new(values).map_err(|error| corrupt(&format!("{what}: {error}")))
}

/// Loads every record of one graph. A graph with no rows loads empty.
pub(crate) fn load(connection: &Connection, graph: &GraphId) -> Result<Graph, StoreError> {
    let id = sql::graph_id(graph);
    let mut children = Children::load(connection, &id, None)?;
    let mut nodes = Vec::new();
    for row in rows(connection, &node_select(""), vec![text(&id)])? {
        nodes.push(node_from_row(&row, &mut children)?);
    }
    children.check_attached()?;
    let mut roles = Vec::new();
    for (key, slug, title, multi) in labeled(connection, "roles", &id)? {
        roles.push(Role {
            key: parse(&key)?,
            id: parse(&slug)?,
            title: title.as_deref().map(parse).transpose()?,
            multi,
        });
    }
    let mut kinds = Vec::new();
    for (key, slug, title, multi) in labeled(connection, "participation_kinds", &id)? {
        kinds.push(ParticipationKind {
            key: parse(&key)?,
            id: parse(&slug)?,
            title: title.as_deref().map(parse).transpose()?,
            multi,
        });
    }
    let owner = sql::first(
        connection,
        "SELECT default_owner FROM graphs WHERE id = ?1",
        vec![text(&id)],
    )?;
    Ok(Graph {
        default_owner: owner.map(|row| row.opt_parse(0)).transpose()?.flatten(),
        roles: keyed(roles, "roles")?,
        participation_kinds: keyed(kinds, "participation kinds")?,
        nodes: keyed(nodes, "nodes")?,
        retired_keys: retired_keys(connection, &id)?,
        insertions: insertions(connection, &id)?,
        state: crate::state::load(connection, &id)?,
    })
}

/// A role's or kind's columns: key, id, title, multi.
type Labeled = (String, String, Option<String>, bool);

fn labeled(connection: &Connection, table: &str, id: &str) -> Result<Vec<Labeled>, StoreError> {
    let select =
        format!("SELECT key, id, title, multi FROM {table} WHERE graph_id = ?1 ORDER BY key");
    let mut found = Vec::new();
    for row in rows(connection, &select, vec![text(id)])? {
        found.push((row.text(0)?, row.text(1)?, row.opt_text(2)?, row.flag(3)?));
    }
    Ok(found)
}

fn insertions(
    connection: &Connection,
    id: &str,
) -> Result<Keyed<Insertion, InsertionCount>, StoreError> {
    let select = "SELECT key, segment, version, parent_key, roles, kinds, nodes \
                  FROM insertions WHERE graph_id = ?1 ORDER BY key";
    let mut found = Vec::new();
    for row in rows(connection, select, vec![text(id)])? {
        found.push(Insertion {
            key: row.parse(0)?,
            segment: Lineage {
                route: row.parse(1)?,
                version: row.number(2)?,
            },
            parent: row.opt_parse(3)?,
            roles: row.json(4)?,
            kinds: row.json(5)?,
            nodes: row.json(6)?,
        });
    }
    keyed(found, "insertions")
}

fn retired_keys(connection: &Connection, id: &str) -> Result<RetiredKeys, StoreError> {
    let mut retired = RetiredKeys {
        nodes: BTreeSet::new(),
        roles: BTreeSet::new(),
        kinds: BTreeSet::new(),
    };
    let select = "SELECT kind, key FROM retired_keys WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)])? {
        match row.text(0)?.as_str() {
            "node" => retired.nodes.insert(row.parse(1)?),
            "role" => retired.roles.insert(row.parse(1)?),
            "kind" => retired.kinds.insert(row.parse(1)?),
            other => return Err(corrupt(&format!("retired key kind {other:?}"))),
        };
    }
    Ok(retired)
}

impl From<StoreError> for SqlError {
    fn from(error: StoreError) -> Self {
        SqlError::Other(error.to_string())
    }
}

/// A graph's edges, participations, and resources, by node.
struct Children {
    requires: BTreeMap<NodeKey, BTreeSet<NodeKey>>,
    /// Each participation's role, or none for explicit identities.
    participations: BTreeMap<NodeKey, BTreeMap<KindKey, Option<RoleKey>>>,
    /// The identities of each participation without a role.
    identities: BTreeMap<(NodeKey, KindKey), BTreeSet<IdentityKey>>,
    resources: BTreeMap<NodeKey, Vec<Resource<KeyRefs>>>,
}

impl Children {
    /// The children of every node of a graph, or of one node.
    fn load(connection: &Connection, id: &str, node: Option<&str>) -> Result<Self, StoreError> {
        let params = || {
            let mut params = vec![text(id)];
            params.extend(node.map(text));
            params
        };
        let only = if node.is_some() { " AND node = ?2" } else { "" };
        let mut requires: BTreeMap<NodeKey, BTreeSet<NodeKey>> = BTreeMap::new();
        let select = format!("SELECT node, requires FROM edges WHERE graph_id = ?1{only}");
        for row in rows(connection, &select, params())? {
            requires
                .entry(row.parse(0)?)
                .or_default()
                .insert(row.parse(1)?);
        }
        let mut participations: BTreeMap<NodeKey, BTreeMap<KindKey, Option<RoleKey>>> =
            BTreeMap::new();
        let select =
            format!("SELECT node, kind, role FROM participations WHERE graph_id = ?1{only}");
        for row in rows(connection, &select, params())? {
            participations
                .entry(row.parse(0)?)
                .or_default()
                .insert(row.parse(1)?, row.opt_parse(2)?);
        }
        let mut identities: BTreeMap<(NodeKey, KindKey), BTreeSet<IdentityKey>> = BTreeMap::new();
        let select = format!(
            "SELECT node, kind, identity FROM participation_identities WHERE graph_id = ?1{only}"
        );
        for row in rows(connection, &select, params())? {
            identities
                .entry((row.parse(0)?, row.parse(1)?))
                .or_default()
                .insert(row.parse(2)?);
        }
        let mut resources: BTreeMap<NodeKey, Vec<Resource<KeyRefs>>> = BTreeMap::new();
        let select = format!(
            "SELECT node, key, title, type, body FROM resources WHERE graph_id = ?1{only} \
             ORDER BY node, position"
        );
        for row in rows(connection, &select, params())? {
            resources.entry(row.parse(0)?).or_default().push(Resource {
                key: row.parse(1)?,
                title: row.opt_parse(2)?,
                content: resource_content(&row.text(3)?, &row.text(4)?)?,
            });
        }
        Ok(Self {
            requires,
            participations,
            identities,
            resources,
        })
    }

    fn requires(&mut self, node: &NodeKey) -> Result<Requires<KeyRefs>, StoreError> {
        let requires = self.requires.remove(node).unwrap_or_default();
        BoundedSet::new(requires).map_err(|error| corrupt(&format!("{node} requires: {error}")))
    }

    fn participations(&mut self, node: &NodeKey) -> Result<Participations<KeyRefs>, StoreError> {
        let mut sources = BTreeMap::new();
        for (kind, role) in self.participations.remove(node).unwrap_or_default() {
            let identities = self.identities.remove(&(node.clone(), kind.clone()));
            let source = match (role, identities) {
                (Some(role), None) => ParticipationSource::Role(role),
                (None, identities) => ParticipationSource::Identities(
                    BoundedSet::new(identities.unwrap_or_default())
                        .map_err(|error| corrupt(&format!("{node} {kind}: {error}")))?,
                ),
                (Some(_), Some(_)) => {
                    return Err(corrupt(&format!("{node} {kind} has a role and identities")));
                }
            };
            sources.insert(kind, source);
        }
        Participations::try_from(sources)
            .map_err(|error| corrupt(&format!("{node} participations: {error}")))
    }

    /// Fails when a child row is left over: it names no node or no participation.
    fn check_attached(&self) -> Result<(), StoreError> {
        let stray = (self.requires.keys())
            .chain(self.participations.keys())
            .chain(self.resources.keys())
            .next()
            .map(ToString::to_string)
            .or_else(|| {
                (self.identities.keys().next()).map(|(node, kind)| format!("{node} {kind}"))
            });
        match stray {
            Some(stray) => Err(corrupt(&format!(
                "child rows of {stray} have no parent row"
            ))),
            None => Ok(()),
        }
    }
}

/// Loads one node, with its edges, participations, and resources.
pub(crate) fn load_node(
    connection: &Connection,
    graph: &str,
    key: &str,
) -> Result<Option<Node<KeyRefs>>, StoreError> {
    let select = node_select(" AND key = ?2");
    let Some(row) = sql::first(connection, &select, vec![text(graph), text(key)])? else {
        return Ok(None);
    };
    let mut children = Children::load(connection, graph, Some(key))?;
    node_from_row(&row, &mut children).map(Some)
}

#[cfg(test)]
mod tests {
    use cairn_schema::Group;

    use super::*;

    #[test]
    fn every_node_writes_one_value_per_column() {
        for node in cairn_store::build::every_content_graph().nodes.values() {
            let values = node_values(node).unwrap();
            assert_eq!(values.len(), NODE_COLUMNS.len(), "{}", node.key);
        }
    }

    #[test]
    fn a_column_the_kind_or_answer_type_cannot_have_does_not_load() {
        let on_action = KindColumns::<KeyRefs> {
            requires_artifact: Some(true),
            ..KindColumns::default()
        };
        assert!(on_action.into_payload(NodeKind::Action).is_err());
        let on_text = KindColumns::<KeyRefs> {
            prompt: Some(parse("Which?").unwrap()),
            answer_type: Some(AnswerType::Text),
            fills_role: Some(parse("r_lead").unwrap()),
            ..KindColumns::default()
        };
        assert!(on_text.into_payload(NodeKind::Decision).is_err());
    }

    #[test]
    fn a_null_flag_loads_as_its_default() {
        let group = KindColumns::<KeyRefs>::default()
            .into_payload(NodeKind::Group)
            .unwrap();
        let expected = Payload::Group(Group {
            opens_at: None,
            closes_at: None,
            gates: true,
            closes: true,
        });
        assert_eq!(group, expected);
    }
}
