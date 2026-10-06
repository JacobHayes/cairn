//! A graph's rows: a node's columns, and loading every table of one graph into a [`Graph`].
//!
//! Each codec here destructures the schema value it splits into columns and builds the
//! value it loads field by field, with no `..` and no catch-all arm over a schema enum, so a
//! field or variant added to the schema does not compile until it has a column (PRACTICES,
//! Shell: conformance and integration). Text names back to variants are the one direction
//! the compiler cannot check; the every-field and every-variant conformance cases do.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::collections::LimitOf;
use cairn_schema::{
    Action, AnswerSpec, AnswerType, BoundedSet, Choices, Days, Decision, Deliverable, EntityKey,
    Graph, GraphId, Group, HasKey, Keyed, KindField, KindKey, Markdown, Milestone, Node, NodeKey,
    NodeKind, ParticipationKind, ParticipationSource, Participations, Payload, Resource,
    ResourceContent, RetiredKeys, Role, RoleKey, Weight, node::Requires, refs::KeyRefs,
};
use cairn_store::StoreError;
use turso::{Connection, Value};

use crate::sql::{
    self, Row, SqlError, corrupt, flag, opt_int, opt_json, opt_text, parse, rows, text,
};

/// The nodes table's columns after `graph_id`, in the order [`node_values`] writes them and
/// [`node_from_row`] reads them (A1a): the fields every kind shares, then [`KindColumns`].
/// A node's `requires`, `participations`, and `resources` are rows of their own tables.
pub(crate) const NODE_COLUMNS: [&str; 25] = [
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
    "is_final",
    "auto_reach",
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
    values.extend(KindColumns::of(payload).values()?);
    Ok(values)
}

/// A node's kind-specific columns: each null unless the node's kind, and for a decision its
/// answer type, has the field (A1a).
struct KindColumns {
    estimate: Option<Days>,
    placeholder: Option<bool>,
    requires_artifact: Option<bool>,
    is_final: Option<bool>,
    auto_reach: Option<bool>,
    opens_at: Option<NodeKey>,
    closes_at: Option<NodeKey>,
    gates: Option<bool>,
    closes: Option<bool>,
    prompt: Option<Markdown>,
    answer_type: Option<AnswerType>,
    choices: Option<Choices>,
    fills_role: Option<RoleKey>,
    feeds_milestone: Option<NodeKey>,
    help: Option<Markdown>,
}

impl KindColumns {
    const NONE: Self = Self {
        estimate: None,
        placeholder: None,
        requires_artifact: None,
        is_final: None,
        auto_reach: None,
        opens_at: None,
        closes_at: None,
        gates: None,
        closes: None,
        prompt: None,
        answer_type: None,
        choices: None,
        fills_role: None,
        feeds_milestone: None,
        help: None,
    };

    /// The columns a payload fills.
    fn of(payload: &Payload<KeyRefs>) -> Self {
        match payload {
            Payload::Decision(Decision {
                prompt,
                help,
                answer,
            }) => {
                let mut columns = Self {
                    prompt: Some(prompt.clone()),
                    help: help.clone(),
                    answer_type: Some(answer.answer_type()),
                    ..Self::NONE
                };
                match answer {
                    AnswerSpec::Boolean | AnswerSpec::Text => {}
                    AnswerSpec::SingleChoice(choices) | AnswerSpec::MultiChoice(choices) => {
                        columns.choices = Some(choices.clone());
                    }
                    AnswerSpec::Date { feeds_milestone } => {
                        columns.feeds_milestone.clone_from(feeds_milestone);
                    }
                    AnswerSpec::Entity { fills_role } | AnswerSpec::EntityList { fills_role } => {
                        columns.fills_role.clone_from(fills_role);
                    }
                }
                columns
            }
            Payload::Deliverable(Deliverable {
                estimate,
                placeholder,
                requires_artifact,
            }) => Self {
                estimate: *estimate,
                placeholder: Some(*placeholder),
                requires_artifact: Some(*requires_artifact),
                ..Self::NONE
            },
            Payload::Action(Action {
                estimate,
                placeholder,
            }) => Self {
                estimate: *estimate,
                placeholder: Some(*placeholder),
                ..Self::NONE
            },
            Payload::Milestone(Milestone {
                is_final,
                auto_reach,
            }) => Self {
                is_final: Some(*is_final),
                auto_reach: Some(*auto_reach),
                ..Self::NONE
            },
            Payload::Group(Group {
                opens_at,
                closes_at,
                gates,
                closes,
            }) => Self {
                opens_at: opens_at.clone(),
                closes_at: closes_at.clone(),
                gates: Some(*gates),
                closes: Some(*closes),
                ..Self::NONE
            },
        }
    }

    /// The column values, in [`NODE_COLUMNS`] order.
    fn values(self) -> Result<Vec<Value>, StoreError> {
        let Self {
            estimate,
            placeholder,
            requires_artifact,
            is_final,
            auto_reach,
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
        } = self;
        let opt_flag = |value: Option<bool>| value.map_or(Value::Null, flag);
        Ok(vec![
            opt_int(estimate.map(Days::get)),
            opt_flag(placeholder),
            opt_flag(requires_artifact),
            opt_flag(is_final),
            opt_flag(auto_reach),
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

    /// The columns of a nodes row.
    fn read(row: &Row) -> Result<Self, StoreError> {
        let at = |offset: usize| KIND_COLUMNS_START + offset;
        Ok(Self {
            estimate: row.opt_int(at(0))?.map(sql::number).transpose()?,
            placeholder: row.opt_flag(at(1))?,
            requires_artifact: row.opt_flag(at(2))?,
            is_final: row.opt_flag(at(3))?,
            auto_reach: row.opt_flag(at(4))?,
            opens_at: row.opt_parse(at(5))?,
            closes_at: row.opt_parse(at(6))?,
            gates: row.opt_flag(at(7))?,
            closes: row.opt_flag(at(8))?,
            prompt: row.opt_parse(at(9))?,
            answer_type: row
                .opt_text(at(10))?
                .map(|name| answer_type_from(&name))
                .transpose()?,
            choices: row.opt_json(at(11))?,
            fills_role: row.opt_parse(at(12))?,
            feeds_milestone: row.opt_parse(at(13))?,
            help: row.opt_parse(at(14))?,
        })
    }

    /// The kind-restricted fields with a value.
    fn present(&self) -> Vec<KindField> {
        [
            (KindField::Estimate, self.estimate.is_some()),
            (KindField::Placeholder, self.placeholder.is_some()),
            (
                KindField::RequiresArtifact,
                self.requires_artifact.is_some(),
            ),
            (KindField::Final, self.is_final.is_some()),
            (KindField::AutoReach, self.auto_reach.is_some()),
            (KindField::OpensAt, self.opens_at.is_some()),
            (KindField::ClosesAt, self.closes_at.is_some()),
            (KindField::Gates, self.gates.is_some()),
            (KindField::Closes, self.closes.is_some()),
            (KindField::Prompt, self.prompt.is_some()),
            (KindField::Help, self.help.is_some()),
            (KindField::AnswerType, self.answer_type.is_some()),
            (KindField::Choices, self.choices.is_some()),
            (KindField::FillsRole, self.fills_role.is_some()),
            (KindField::FeedsMilestone, self.feeds_milestone.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, present)| present.then_some(field))
        .collect()
    }

    /// The payload of a node of `kind`. A null flag is its default, as the written form
    /// leaves it out.
    ///
    /// # Errors
    ///
    /// A column the kind or answer type cannot have, or a missing required one (A1a).
    fn payload(self, kind: NodeKind) -> Result<Payload<KeyRefs>, StoreError> {
        for field in self.present() {
            let allowed = field.allowed_on(kind)
                && self
                    .answer_type
                    .is_none_or(|answer| field.allowed_for_answer(answer));
            if !allowed {
                return Err(corrupt(&format!(
                    "a {} node has a {} column",
                    kind.name(),
                    field.name()
                )));
            }
        }
        let missing = |field: KindField| corrupt(&format!("a node has no {}", field.name()));
        Ok(match kind {
            NodeKind::Decision => {
                let answer_type = self
                    .answer_type
                    .ok_or_else(|| missing(KindField::AnswerType))?;
                let choices = self.choices.ok_or_else(|| missing(KindField::Choices));
                let answer = match answer_type {
                    AnswerType::Boolean => AnswerSpec::Boolean,
                    AnswerType::SingleChoice => AnswerSpec::SingleChoice(choices?),
                    AnswerType::MultiChoice => AnswerSpec::MultiChoice(choices?),
                    AnswerType::Text => AnswerSpec::Text,
                    AnswerType::Date => AnswerSpec::Date {
                        feeds_milestone: self.feeds_milestone,
                    },
                    AnswerType::Entity => AnswerSpec::Entity {
                        fills_role: self.fills_role,
                    },
                    AnswerType::EntityList => AnswerSpec::EntityList {
                        fills_role: self.fills_role,
                    },
                };
                Payload::Decision(Decision {
                    prompt: self.prompt.ok_or_else(|| missing(KindField::Prompt))?,
                    help: self.help,
                    answer,
                })
            }
            NodeKind::Deliverable => Payload::Deliverable(Deliverable {
                estimate: self.estimate,
                placeholder: self.placeholder.unwrap_or(false),
                requires_artifact: self.requires_artifact.unwrap_or(false),
            }),
            NodeKind::Action => Payload::Action(Action {
                estimate: self.estimate,
                placeholder: self.placeholder.unwrap_or(false),
            }),
            NodeKind::Milestone => Payload::Milestone(Milestone {
                is_final: self.is_final.unwrap_or(false),
                auto_reach: self.auto_reach.unwrap_or(false),
            }),
            NodeKind::Group => Payload::Group(Group {
                opens_at: self.opens_at,
                closes_at: self.closes_at,
                gates: self.gates.unwrap_or(true),
                closes: self.closes.unwrap_or(true),
            }),
        })
    }
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
        payload: KindColumns::read(row)?.payload(kind)?,
    })
}

fn keyed<V: HasKey, L: LimitOf>(values: Vec<V>, what: &str) -> Result<Keyed<V, L>, StoreError> {
    Keyed::new(values).map_err(|error| corrupt(&format!("{what}: {error}")))
}

/// Loads every record of one graph. A graph with no rows loads empty.
pub(crate) async fn load(connection: &Connection, graph: &GraphId) -> Result<Graph, StoreError> {
    let id = sql::graph_id(graph);
    let mut children = Children::load(connection, &id, None).await?;
    let mut nodes = Vec::new();
    for row in rows(connection, &node_select(""), vec![text(&id)]).await? {
        nodes.push(node_from_row(&row, &mut children)?);
    }
    children.check_attached()?;
    let mut roles = Vec::new();
    for (key, slug, title, multi) in labeled(connection, "roles", &id).await? {
        roles.push(Role {
            key: parse(&key)?,
            id: parse(&slug)?,
            title: title.as_deref().map(parse).transpose()?,
            multi,
        });
    }
    let mut kinds = Vec::new();
    for (key, slug, title, multi) in labeled(connection, "participation_kinds", &id).await? {
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
    )
    .await?;
    Ok(Graph {
        default_owner: owner.map(|row| row.opt_parse(0)).transpose()?.flatten(),
        roles: keyed(roles, "roles")?,
        participation_kinds: keyed(kinds, "participation kinds")?,
        nodes: keyed(nodes, "nodes")?,
        retired_keys: retired_keys(connection, &id).await?,
        state: crate::state::load(connection, &id).await?,
    })
}

/// A role's or kind's columns: key, id, title, multi.
type Labeled = (String, String, Option<String>, bool);

async fn labeled(
    connection: &Connection,
    table: &str,
    id: &str,
) -> Result<Vec<Labeled>, StoreError> {
    let select =
        format!("SELECT key, id, title, multi FROM {table} WHERE graph_id = ?1 ORDER BY key");
    let mut found = Vec::new();
    for row in rows(connection, &select, vec![text(id)]).await? {
        found.push((row.text(0)?, row.text(1)?, row.opt_text(2)?, row.flag(3)?));
    }
    Ok(found)
}

async fn retired_keys(connection: &Connection, id: &str) -> Result<RetiredKeys, StoreError> {
    let mut retired = RetiredKeys {
        nodes: BTreeSet::new(),
        roles: BTreeSet::new(),
        kinds: BTreeSet::new(),
    };
    let select = "SELECT kind, key FROM retired_keys WHERE graph_id = ?1";
    for row in rows(connection, select, vec![text(id)]).await? {
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
    /// Each participation's role, or none for explicit entities.
    participations: BTreeMap<NodeKey, BTreeMap<KindKey, Option<RoleKey>>>,
    /// The entities of each participation without a role.
    entities: BTreeMap<(NodeKey, KindKey), BTreeSet<EntityKey>>,
    resources: BTreeMap<NodeKey, Vec<Resource<KeyRefs>>>,
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
        let mut requires: BTreeMap<NodeKey, BTreeSet<NodeKey>> = BTreeMap::new();
        let select = format!("SELECT node, requires FROM edges WHERE graph_id = ?1{only}");
        for row in rows(connection, &select, params()).await? {
            requires
                .entry(row.parse(0)?)
                .or_default()
                .insert(row.parse(1)?);
        }
        let mut participations: BTreeMap<NodeKey, BTreeMap<KindKey, Option<RoleKey>>> =
            BTreeMap::new();
        let select =
            format!("SELECT node, kind, role FROM participations WHERE graph_id = ?1{only}");
        for row in rows(connection, &select, params()).await? {
            participations
                .entry(row.parse(0)?)
                .or_default()
                .insert(row.parse(1)?, row.opt_parse(2)?);
        }
        let mut entities: BTreeMap<(NodeKey, KindKey), BTreeSet<EntityKey>> = BTreeMap::new();
        let select = format!(
            "SELECT node, kind, entity FROM participation_entities WHERE graph_id = ?1{only}"
        );
        for row in rows(connection, &select, params()).await? {
            entities
                .entry((row.parse(0)?, row.parse(1)?))
                .or_default()
                .insert(row.parse(2)?);
        }
        let mut resources: BTreeMap<NodeKey, Vec<Resource<KeyRefs>>> = BTreeMap::new();
        let select = format!(
            "SELECT node, key, title, type, body FROM resources WHERE graph_id = ?1{only} \
             ORDER BY node, position"
        );
        for row in rows(connection, &select, params()).await? {
            resources.entry(row.parse(0)?).or_default().push(Resource {
                key: row.parse(1)?,
                title: row.opt_parse(2)?,
                content: resource_content(&row.text(3)?, &row.text(4)?)?,
            });
        }
        Ok(Self {
            requires,
            participations,
            entities,
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
            let entities = self.entities.remove(&(node.clone(), kind.clone()));
            let source = match (role, entities) {
                (Some(role), None) => ParticipationSource::Role(role),
                (None, entities) => ParticipationSource::Entities(
                    BoundedSet::new(entities.unwrap_or_default())
                        .map_err(|error| corrupt(&format!("{node} {kind}: {error}")))?,
                ),
                (Some(_), Some(_)) => {
                    return Err(corrupt(&format!("{node} {kind} has a role and entities")));
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
            .or_else(|| (self.entities.keys().next()).map(|(node, kind)| format!("{node} {kind}")));
        match stray {
            Some(stray) => Err(corrupt(&format!(
                "child rows of {stray} have no parent row"
            ))),
            None => Ok(()),
        }
    }
}

/// Loads one node, with its edges, participations, and resources.
pub(crate) async fn load_node(
    connection: &Connection,
    graph: &str,
    key: &str,
) -> Result<Option<Node<KeyRefs>>, StoreError> {
    let select = node_select(" AND key = ?2");
    let Some(row) = sql::first(connection, &select, vec![text(graph), text(key)]).await? else {
        return Ok(None);
    };
    let mut children = Children::load(connection, graph, Some(key)).await?;
    node_from_row(&row, &mut children).map(Some)
}

#[cfg(test)]
mod tests {
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
        let on_action = KindColumns {
            requires_artifact: Some(true),
            ..KindColumns::NONE
        };
        assert!(on_action.payload(NodeKind::Action).is_err());
        let on_text = KindColumns {
            prompt: Some(parse("Which?").unwrap()),
            answer_type: Some(AnswerType::Text),
            fills_role: Some(parse("r_lead").unwrap()),
            ..KindColumns::NONE
        };
        assert!(on_text.payload(NodeKind::Decision).is_err());
    }

    #[test]
    fn a_null_flag_loads_as_its_default() {
        let group = KindColumns::NONE.payload(NodeKind::Group).unwrap();
        let expected = Payload::Group(Group {
            opens_at: None,
            closes_at: None,
            gates: true,
            closes: true,
        });
        assert_eq!(group, expected);
    }
}
