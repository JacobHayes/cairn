//! A change set's writes as rows, inside the commit's transaction: the same semantics as
//! the memory backend (`cairn_store::memory`), which the conformance suite holds both to.

use std::collections::BTreeSet;

use cairn_schema::{
    AnswerValue, Domain, Edge, Entity, GraphId, GraphRecord, JourneyHeader, Lineage, Node, NodeKey,
    ParticipationKind, ParticipationSource, Proposal, ProposalDraft, Record, RecordKey, Resource,
    RetiredKey, Revision, Role, RouteHeader, SnoozeTarget, Write,
    limits::{EDGE_COUNT_PER_NODE_MAX, KIND_COUNT_MAX},
    refs::KeyRefs,
};
use cairn_store::{CommitError, StoreError};
use turso::{Connection, Value};

use crate::graph::{self, NODE_COLUMNS};
use crate::sql::{
    Abort, date, domain_columns, execute, first, flag, graph_id, int, json, json_enum, opt_int,
    opt_text, text, time,
};

/// The tables holding one graph's records, children before parents.
pub(crate) const GRAPH_TABLES: [&str; 19] = [
    "participation_entities",
    "answer_entities",
    "role_fill_entities",
    "edges",
    "participations",
    "resources",
    "nodes",
    "roles",
    "participation_kinds",
    "retired_keys",
    "node_states",
    "local_edits",
    "answers",
    "role_fills",
    "pins",
    "snoozes",
    "overrides",
    "tombstones",
    "annotations",
];

fn malformed(reason: String) -> Abort {
    Abort::Answer(CommitError::Failed(StoreError::Malformed(reason)))
}

/// Writes one commit's records.
pub(crate) struct Writer<'connection> {
    connection: &'connection Connection,
    /// The revision the commit produces, for a domain row its first commit writes.
    revision: Revision,
    /// Graph rows known to exist.
    graphs: BTreeSet<String>,
}

impl<'connection> Writer<'connection> {
    pub fn new(connection: &'connection Connection, revision: Revision) -> Self {
        Self {
            connection,
            revision,
            graphs: BTreeSet::new(),
        }
    }

    async fn run(&self, sql: &str, params: Vec<Value>) -> Result<u64, Abort> {
        Ok(execute(self.connection, sql, params).await?)
    }

    async fn exists(&self, sql: &str, params: Vec<Value>) -> Result<bool, Abort> {
        Ok(first(self.connection, sql, params).await?.is_some())
    }

    pub async fn apply(&mut self, write: &Write) -> Result<(), Abort> {
        match write {
            Write::Put(record) => self.put(record).await,
            Write::Remove(key) => self.remove(key).await,
            Write::CopyGraph { from, to } => self.copy(from, to).await,
        }
    }

    /// The graph's row, created on the first write into it.
    async fn graph_row(&mut self, graph: &GraphId) -> Result<String, Abort> {
        let id = graph_id(graph);
        if self.graphs.contains(&id) {
            return Ok(id);
        }
        let (kind, journey, route, version) = match graph {
            GraphId::Journey(journey) => ("journey", text(journey), Value::Null, Value::Null),
            GraphId::RouteDraft(route) => ("route_draft", Value::Null, text(route), Value::Null),
            GraphId::RouteVersion { route, version } => (
                "route_version",
                Value::Null,
                text(route),
                int(version.get()),
            ),
        };
        self.run(
            "INSERT OR IGNORE INTO graphs (id, kind, journey_id, route_id, version_number) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![text(&id), text(kind), journey, route, version],
        )
        .await?;
        self.graphs.insert(id.clone());
        Ok(id)
    }

    async fn put(&mut self, record: &Record) -> Result<(), Abort> {
        match record {
            Record::Graph { graph, record } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return Err(malformed(format!(
                        "{graph:?} is published and never written again"
                    )));
                }
                let id = self.graph_row(graph).await?;
                self.put_graph_record(&id, record).await?;
            }
            Record::JourneyHeader(header) => self.put_journey_header(header).await?,
            Record::RouteHeader(header) => self.put_route_header(header).await?,
            Record::RouteDraft { route, extends } => {
                self.run(
                    "INSERT OR REPLACE INTO route_drafts (route_id, extends) VALUES (?1, ?2)",
                    vec![
                        text(route),
                        opt_int(extends.map(cairn_schema::VersionNumber::get)),
                    ],
                )
                .await?;
            }
            Record::RouteVersion {
                route,
                version,
                published_at,
            } => {
                let params = vec![text(route), int(version.get())];
                let select =
                    "SELECT 1 FROM route_versions WHERE route_id = ?1 AND version_number = ?2";
                if self.exists(select, params.clone()).await? {
                    return Err(malformed(format!(
                        "{route} version {version} is already published"
                    )));
                }
                let mut params = params;
                params.push(time(*published_at)?);
                self.run(
                    "INSERT INTO route_versions (route_id, version_number, published_at) \
                     VALUES (?1, ?2, ?3)",
                    params,
                )
                .await?;
            }
            Record::DeletedJourney {
                journey,
                deleted_at,
            } => {
                self.run(
                    "INSERT OR REPLACE INTO deleted_journeys (id, deleted_at) VALUES (?1, ?2)",
                    vec![text(journey), time(*deleted_at)?],
                )
                .await?;
            }
            Record::Entity(entity) => self.put_entity(entity).await?,
            Record::EntityAlias { alias, entity } => {
                self.run(
                    "INSERT OR REPLACE INTO entity_aliases (alias, entity) VALUES (?1, ?2)",
                    vec![text(alias), text(entity)],
                )
                .await?;
            }
            Record::Proposal(proposal) => self.put_proposal(proposal).await?,
        }
        Ok(())
    }

    async fn put_journey_header(&mut self, header: &JourneyHeader) -> Result<(), Abort> {
        let JourneyHeader {
            id,
            name,
            description,
            status,
            lineage,
            created_at,
            created_on,
        } = header;
        let (route, version) = match lineage {
            Some(Lineage { route, version }) => (text(route), int(version.get())),
            None => (Value::Null, Value::Null),
        };
        self.run(
            "INSERT INTO journeys (id, revision, name, description, status, lineage_route, \
             lineage_version, created_at, created_on) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT (id) DO UPDATE SET name = excluded.name, \
             description = excluded.description, status = excluded.status, \
             lineage_route = excluded.lineage_route, lineage_version = excluded.lineage_version, \
             created_at = excluded.created_at, created_on = excluded.created_on",
            vec![
                text(id),
                int(self.revision.get()),
                text(name),
                opt_text(description.as_ref()),
                json_enum(status)?,
                route,
                version,
                time(*created_at)?,
                date(*created_on),
            ],
        )
        .await?;
        Ok(())
    }

    async fn put_route_header(&mut self, header: &RouteHeader) -> Result<(), Abort> {
        let RouteHeader {
            id,
            name,
            description,
            retired,
        } = header;
        self.run(
            "INSERT INTO routes (id, revision, name, description, retired) \
             VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (id) DO UPDATE SET \
             name = excluded.name, description = excluded.description, \
             retired = excluded.retired",
            vec![
                text(id),
                int(self.revision.get()),
                text(name),
                opt_text(description.as_ref()),
                flag(*retired),
            ],
        )
        .await?;
        Ok(())
    }

    async fn put_entity(&mut self, entity: &Entity) -> Result<(), Abort> {
        let Entity { key, name, emails } = entity;
        self.run(
            "INSERT INTO entities (key, name) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET name = excluded.name",
            vec![text(key), text(name)],
        )
        .await?;
        self.run(
            "DELETE FROM entity_emails WHERE entity = ?1",
            vec![text(key)],
        )
        .await?;
        for email in emails {
            self.run(
                "INSERT INTO entity_emails (entity, email) VALUES (?1, ?2)",
                vec![text(key), text(email)],
            )
            .await?;
        }
        Ok(())
    }

    async fn put_proposal(&mut self, proposal: &Proposal) -> Result<(), Abort> {
        let Proposal {
            id: proposal_id,
            destination,
            revision,
            status,
            draft,
            proposing_agent,
            created_by,
            created_at,
        } = proposal;
        let ProposalDraft {
            title,
            description,
            destination_revision,
            mutations,
            items,
        } = draft;
        let (kind, id) = domain_columns(destination);
        self.run(
            "INSERT OR REPLACE INTO proposals (id, destination_kind, destination_id, revision, \
             status, title, description, destination_revision, mutations, items, \
             proposing_agent, created_by, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            vec![
                text(proposal_id),
                text(kind),
                id,
                int(revision.get()),
                json_enum(status)?,
                text(title),
                opt_text(description.as_ref()),
                int(destination_revision.get()),
                json(mutations)?,
                json(items)?,
                opt_text(proposing_agent.as_ref()),
                text(created_by),
                time(*created_at)?,
            ],
        )
        .await?;
        Ok(())
    }

    async fn node_exists(&self, id: &str, node: &cairn_schema::NodeKey) -> Result<(), Abort> {
        let select = "SELECT 1 FROM nodes WHERE graph_id = ?1 AND key = ?2";
        if self.exists(select, vec![text(id), text(node)]).await? {
            Ok(())
        } else {
            Err(malformed(format!("node {node} is not in the graph")))
        }
    }

    async fn count(&self, sql: &str, params: Vec<Value>) -> Result<i64, Abort> {
        let row = first(self.connection, sql, params).await?;
        Ok(row.map(|row| row.int(0)).transpose()?.unwrap_or(0))
    }

    async fn put_graph_record(&mut self, id: &str, record: &GraphRecord) -> Result<(), Abort> {
        match record {
            GraphRecord::Node(node) => self.put_node(id, node).await,
            GraphRecord::NodeField { node, value } => {
                let held = graph::load_node(self.connection, id, node.as_str()).await?;
                let Some(mut changed) = held else {
                    return Err(malformed(format!("node {node} is not in the graph")));
                };
                if !value.clone().write(&mut changed) {
                    return Err(malformed(format!(
                        "node {node} has no field {:?}",
                        value.field()
                    )));
                }
                self.put_node(id, &changed).await
            }
            GraphRecord::Edge(Edge { node, requires }) => self.put_edge(id, node, requires).await,
            GraphRecord::Role(role) => self.put_labeled("roles", id, role_row(role)).await,
            GraphRecord::Kind(kind) => {
                self.put_labeled("participation_kinds", id, kind_row(kind))
                    .await
            }
            GraphRecord::DefaultOwner(role) => {
                self.run(
                    "UPDATE graphs SET default_owner = ?2 WHERE id = ?1",
                    vec![text(id), text(role)],
                )
                .await?;
                Ok(())
            }
            GraphRecord::Participation { node, kind, source } => {
                self.node_exists(id, node).await?;
                self.put_participation(id, node, kind, source).await?;
                let select =
                    "SELECT count(*) FROM participations WHERE graph_id = ?1 AND node = ?2";
                let held = self.count(select, vec![text(id), text(node)]).await?;
                if held > i64::from(KIND_COUNT_MAX) {
                    return Err(malformed(format!("{node} has more than the kind limit")));
                }
                Ok(())
            }
            GraphRecord::Resource { node, resource } => {
                self.node_exists(id, node).await?;
                self.put_resource(id, node, resource).await
            }
            GraphRecord::RetiredKey(_)
            | GraphRecord::NodeState { .. }
            | GraphRecord::LocalEdit { .. }
            | GraphRecord::Answer { .. }
            | GraphRecord::RoleFill { .. }
            | GraphRecord::Pin { .. }
            | GraphRecord::Snooze { .. }
            | GraphRecord::Overrides { .. }
            | GraphRecord::Tombstone(_)
            | GraphRecord::Annotation(_) => crate::write_state::put(self, id, record).await,
        }
    }

    async fn put_edge(&self, id: &str, node: &NodeKey, requires: &NodeKey) -> Result<(), Abort> {
        self.node_exists(id, node).await?;
        self.run(
            "INSERT OR IGNORE INTO edges (graph_id, node, requires) VALUES (?1, ?2, ?3)",
            vec![text(id), text(node), text(requires)],
        )
        .await?;
        let select = "SELECT count(*) FROM edges WHERE graph_id = ?1 AND node = ?2";
        let held = self.count(select, vec![text(id), text(node)]).await?;
        if held > i64::from(EDGE_COUNT_PER_NODE_MAX) {
            return Err(malformed(format!(
                "{node} requires more than the edge limit"
            )));
        }
        Ok(())
    }

    /// Runs a statement for the state writes.
    pub(crate) async fn statement(&self, sql: &str, params: Vec<Value>) -> Result<u64, Abort> {
        self.run(sql, params).await
    }

    /// A role's or kind's row: its key, id, title, and multi columns.
    async fn put_labeled(&self, table: &str, id: &str, row: [Value; 4]) -> Result<(), Abort> {
        let sql = format!(
            "INSERT OR REPLACE INTO {table} (graph_id, key, id, title, multi) \
             VALUES (?1, ?2, ?3, ?4, ?5)"
        );
        let mut values = vec![text(id)];
        values.extend(row);
        self.run(&sql, values).await?;
        Ok(())
    }

    async fn put_node(&mut self, id: &str, node: &Node<KeyRefs>) -> Result<(), Abort> {
        let mut values = vec![text(id)];
        values.extend(graph::node_values(node)?);
        let slots: Vec<_> = (1..=values.len()).map(|slot| format!("?{slot}")).collect();
        let sql = format!(
            "INSERT OR REPLACE INTO nodes (graph_id, {}) VALUES ({})",
            NODE_COLUMNS.join(", "),
            slots.join(", ")
        );
        self.run(&sql, values).await?;
        let key = &node.key;
        let params = || vec![text(id), text(key)];
        for table in [
            "edges",
            "participation_entities",
            "participations",
            "resources",
        ] {
            let sql = format!("DELETE FROM {table} WHERE graph_id = ?1 AND node = ?2");
            self.run(&sql, params()).await?;
        }
        for requires in node.requires.iter() {
            self.run(
                "INSERT INTO edges (graph_id, node, requires) VALUES (?1, ?2, ?3)",
                vec![text(id), text(key), text(requires)],
            )
            .await?;
        }
        for (kind, source) in node.participations.as_map() {
            self.put_participation(id, key, kind, source).await?;
        }
        for (position, resource) in node.resources.iter().enumerate() {
            self.insert_resource(id, key, resource, position).await?;
        }
        Ok(())
    }

    async fn put_participation(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        kind: &cairn_schema::KindKey,
        source: &ParticipationSource<KeyRefs>,
    ) -> Result<(), Abort> {
        let params = || vec![text(id), text(node), text(kind)];
        self.run(
            "DELETE FROM participation_entities WHERE graph_id = ?1 AND node = ?2 AND kind = ?3",
            params(),
        )
        .await?;
        let role = match source {
            ParticipationSource::Role(role) => text(role),
            ParticipationSource::Entities(_) => Value::Null,
        };
        let mut values = params();
        values.push(role);
        self.run(
            "INSERT OR REPLACE INTO participations (graph_id, node, kind, role) \
             VALUES (?1, ?2, ?3, ?4)",
            values,
        )
        .await?;
        if let ParticipationSource::Entities(entities) = source {
            for entity in entities.iter() {
                let mut values = params();
                values.push(text(entity));
                self.run(
                    "INSERT INTO participation_entities (graph_id, node, kind, entity) \
                     VALUES (?1, ?2, ?3, ?4)",
                    values,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn put_resource(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        resource: &Resource<KeyRefs>,
    ) -> Result<(), Abort> {
        let select =
            "SELECT position FROM resources WHERE graph_id = ?1 AND node = ?2 AND key = ?3";
        let held = first(
            self.connection,
            select,
            vec![text(id), text(node), text(&resource.key)],
        )
        .await?;
        let position = if let Some(row) = held {
            row.int(0)?
        } else {
            let select = "SELECT coalesce(max(position) + 1, 0) FROM resources \
                          WHERE graph_id = ?1 AND node = ?2";
            self.count(select, vec![text(id), text(node)]).await?
        };
        let position = usize::try_from(position)
            .map_err(|_| malformed(format!("resource position {position}")))?;
        self.insert_resource(id, node, resource, position).await
    }

    async fn insert_resource(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        resource: &Resource<KeyRefs>,
        position: usize,
    ) -> Result<(), Abort> {
        let Resource {
            key,
            title,
            content,
        } = resource;
        let (kind, body) = graph::resource_columns(content);
        let position =
            i64::try_from(position).map_err(|_| malformed("resource position".to_owned()))?;
        self.run(
            "INSERT OR REPLACE INTO resources (graph_id, node, key, position, title, type, body) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            vec![
                text(id),
                text(node),
                text(key),
                int(position),
                opt_text(title.as_ref()),
                text(kind),
                text(&body),
            ],
        )
        .await?;
        Ok(())
    }

    async fn remove(&mut self, key: &RecordKey) -> Result<(), Abort> {
        match key {
            RecordKey::Domain(Domain::Journey(journey)) => self.remove_journey(journey).await?,
            RecordKey::Graph(graph @ (GraphId::Journey(_) | GraphId::RouteDraft(_))) => {
                self.clear_graph(graph).await?;
            }
            RecordKey::InGraph { graph, key } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return Err(malformed(format!(
                        "{graph:?} is published and never written again"
                    )));
                }
                crate::write_state::remove(self, &graph_id(graph), key).await?;
            }
            RecordKey::RouteDraft(route) => {
                self.clear_graph(&GraphId::RouteDraft(route.clone()))
                    .await?;
                self.run(
                    "DELETE FROM route_drafts WHERE route_id = ?1",
                    vec![text(route)],
                )
                .await?;
            }
            RecordKey::Entity(entity) => {
                self.run(
                    "DELETE FROM entity_emails WHERE entity = ?1",
                    vec![text(entity)],
                )
                .await?;
                self.run("DELETE FROM entities WHERE key = ?1", vec![text(entity)])
                    .await?;
            }
            RecordKey::EntityAlias(alias) => {
                self.run(
                    "DELETE FROM entity_aliases WHERE alias = ?1",
                    vec![text(alias)],
                )
                .await?;
            }
            RecordKey::Proposal { id, .. } => {
                self.run("DELETE FROM proposals WHERE id = ?1", vec![text(id)])
                    .await?;
            }
            RecordKey::Domain(_)
            | RecordKey::Graph(GraphId::RouteVersion { .. })
            | RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::DeletedJourney(_) => {
                return Err(malformed(format!("{key:?} is never removed (A11, A19)")));
            }
        }
        Ok(())
    }

    /// A19: a hard delete: the journey's fields, graph, proposals, and events.
    async fn remove_journey(&mut self, journey: &cairn_schema::JourneyId) -> Result<(), Abort> {
        self.clear_graph(&GraphId::Journey(journey.clone())).await?;
        let statements = [
            "DELETE FROM journeys WHERE id = ?1",
            "DELETE FROM proposals WHERE destination_kind = 'journey' AND destination_id = ?1",
            "DELETE FROM event_nodes WHERE seq IN \
             (SELECT seq FROM events WHERE log_kind = 'journey' AND log_id = ?1)",
            "DELETE FROM events WHERE log_kind = 'journey' AND log_id = ?1",
        ];
        for sql in statements {
            self.run(sql, vec![text(journey)]).await?;
        }
        Ok(())
    }

    /// Removes every record of a graph and its row.
    async fn clear_graph(&mut self, graph: &GraphId) -> Result<(), Abort> {
        let id = graph_id(graph);
        for table in GRAPH_TABLES {
            let sql = format!("DELETE FROM {table} WHERE graph_id = ?1");
            self.run(&sql, vec![text(&id)]).await?;
        }
        self.run("DELETE FROM graphs WHERE id = ?1", vec![text(&id)])
            .await?;
        self.graphs.remove(&id);
        Ok(())
    }

    async fn copy(&mut self, from: &GraphId, to: &GraphId) -> Result<(), Abort> {
        let target = graph_id(to);
        if matches!(to, GraphId::RouteVersion { .. })
            && self
                .exists("SELECT 1 FROM graphs WHERE id = ?1", vec![text(&target)])
                .await?
        {
            return Err(malformed(format!(
                "{to:?} is published and never written again"
            )));
        }
        let source = graph::load(self.connection, from).await?;
        let id = self.graph_row(to).await?;
        for record in cairn_store::backend::graph_records(&source) {
            self.put_graph_record(&id, &record).await?;
        }
        Ok(())
    }
}

/// A role's key, id, title, and multi columns.
fn role_row(role: &Role<KeyRefs>) -> [Value; 4] {
    let Role {
        key,
        id,
        title,
        multi,
    } = role;
    [text(key), text(id), opt_text(title.as_ref()), flag(*multi)]
}

/// A participation kind's key, id, title, and multi columns.
fn kind_row(kind: &ParticipationKind<KeyRefs>) -> [Value; 4] {
    let ParticipationKind {
        key,
        id,
        title,
        multi,
    } = kind;
    [text(key), text(id), opt_text(title.as_ref()), flag(*multi)]
}

/// The entities an answer names.
pub(crate) fn answer_entities(value: &AnswerValue) -> Vec<cairn_schema::EntityKey> {
    match value {
        AnswerValue::Entity(entity) => vec![entity.clone()],
        AnswerValue::EntityList(entities) => entities.iter().cloned().collect(),
        AnswerValue::Boolean(_)
        | AnswerValue::SingleChoice(_)
        | AnswerValue::MultiChoice(_)
        | AnswerValue::Text(_)
        | AnswerValue::Date(_) => Vec::new(),
    }
}

/// A snooze's date and node columns.
pub(crate) fn snooze_columns(until: &SnoozeTarget) -> (Value, Value) {
    match until {
        SnoozeTarget::Date(day) => (date(*day), Value::Null),
        SnoozeTarget::Node(node) => (Value::Null, text(node)),
    }
}

/// A retired key's kind column.
pub(crate) fn retired_columns(key: &RetiredKey) -> (&'static str, String) {
    match key {
        RetiredKey::Node(node) => ("node", node.to_string()),
        RetiredKey::Role(role) => ("role", role.to_string()),
        RetiredKey::Kind(kind) => ("kind", kind.to_string()),
    }
}
