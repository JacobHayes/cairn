//! A change set's writes as rows, inside the commit's transaction: the same semantics as
//! the memory backend (`cairn_store::memory`), which the conformance suite holds both to.

use std::collections::BTreeSet;

use cairn_schema::{
    AnswerValue, DeletedId, Domain, Edge, GraphId, GraphRecord, Identity, JourneyHeader, Lineage,
    Node, NodeKey, ParticipationKind, ParticipationSource, Proposal, ProposalDraft, Record,
    RecordKey, Resource, RetiredKey, Revision, Role, RouteHeader, RouteId, SnoozeTarget, Write,
    limits::{EDGE_COUNT_PER_NODE_MAX, KIND_COUNT_MAX},
    refs::KeyRefs,
};
use cairn_store::{CommitError, StoreError};
use rusqlite::Connection;
use rusqlite::types::Value;

use crate::graph::{self, NODE_COLUMNS};
use crate::sql::{
    date, domain_columns, execute, first, flag, graph_id, int, json, json_enum, opt_int, opt_text,
    text, time,
};

/// The tables holding one graph's records, children before parents.
pub(crate) const GRAPH_TABLES: [&str; 19] = [
    "participation_identities",
    "condition_identities",
    "answer_identities",
    "role_fill_identities",
    "edges",
    "participations",
    "resources",
    "nodes",
    "roles",
    "participation_kinds",
    "retired_keys",
    "node_states",
    "ticks",
    "answers",
    "role_fills",
    "pins",
    "snoozes",
    "overrides",
    "annotations",
];

fn malformed(reason: String) -> CommitError {
    CommitError::Failed(StoreError::Malformed(reason))
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

    fn run(&self, sql: &str, params: Vec<Value>) -> Result<u64, CommitError> {
        Ok(execute(self.connection, sql, params)?)
    }

    fn exists(&self, sql: &str, params: Vec<Value>) -> Result<bool, CommitError> {
        Ok(first(self.connection, sql, params)?.is_some())
    }

    pub fn apply(&mut self, write: &Write) -> Result<(), CommitError> {
        match write {
            Write::Put(record) => self.put(record),
            Write::Remove(key) => self.remove(key),
            Write::CopyGraph { from, to } => self.copy(from, to),
        }
    }

    /// The graph's row, created on the first write into it.
    fn graph_row(&mut self, graph: &GraphId) -> Result<String, CommitError> {
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
        )?;
        self.graphs.insert(id.clone());
        Ok(id)
    }

    fn put(&mut self, record: &Record) -> Result<(), CommitError> {
        match record {
            Record::Graph { graph, record } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return Err(malformed(format!(
                        "{graph:?} is published and never written again"
                    )));
                }
                let id = self.graph_row(graph)?;
                self.put_graph_record(&id, record)?;
            }
            Record::JourneyHeader(header) => self.put_journey_header(header)?,
            Record::RouteHeader(header) => self.put_route_header(header)?,
            Record::RouteDraft { route, extends } => {
                self.run(
                    "INSERT OR REPLACE INTO route_drafts (route_id, extends) VALUES (?1, ?2)",
                    vec![
                        text(route),
                        opt_int(extends.map(cairn_schema::VersionNumber::get)),
                    ],
                )?;
            }
            Record::RouteVersion {
                route,
                version,
                published_at,
            } => {
                let params = vec![text(route), int(version.get())];
                let select =
                    "SELECT 1 FROM route_versions WHERE route_id = ?1 AND version_number = ?2";
                if self.exists(select, params.clone())? {
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
                )?;
            }
            Record::Deleted { id, deleted_at } => {
                let (kind, held) = deleted_columns(id);
                self.run(
                    "INSERT OR REPLACE INTO deleted_ids (kind, id, deleted_at) VALUES (?1, ?2, ?3)",
                    vec![text(kind), text(&held), time(*deleted_at)?],
                )?;
            }
            Record::Identity(identity) => self.put_identity(identity)?,
            Record::IdentityAlias { alias, identity } => {
                self.run(
                    "INSERT OR REPLACE INTO identity_aliases (alias, identity) VALUES (?1, ?2)",
                    vec![text(alias), text(identity)],
                )?;
            }
            Record::Proposal(proposal) => self.put_proposal(proposal)?,
        }
        Ok(())
    }

    fn put_journey_header(&mut self, header: &JourneyHeader) -> Result<(), CommitError> {
        let JourneyHeader {
            id,
            name,
            description,
            owners,
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
        )?;
        self.run(
            "DELETE FROM journey_owners WHERE journey_id = ?1",
            vec![text(id)],
        )?;
        for owner in owners.iter() {
            self.run(
                "INSERT INTO journey_owners (journey_id, identity) VALUES (?1, ?2)",
                vec![text(id), text(owner)],
            )?;
        }
        Ok(())
    }

    fn put_route_header(&mut self, header: &RouteHeader) -> Result<(), CommitError> {
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
        )?;
        Ok(())
    }

    fn put_identity(&mut self, identity: &Identity) -> Result<(), CommitError> {
        let Identity { key, name, emails } = identity;
        self.run(
            "INSERT INTO identities (key, name) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET name = excluded.name",
            vec![text(key), opt_text(name.as_ref())],
        )?;
        self.run(
            "DELETE FROM identity_emails WHERE identity = ?1",
            vec![text(key)],
        )?;
        for email in emails {
            self.run(
                "INSERT INTO identity_emails (identity, email) VALUES (?1, ?2)",
                vec![text(key), text(email)],
            )?;
        }
        Ok(())
    }

    fn put_proposal(&mut self, proposal: &Proposal) -> Result<(), CommitError> {
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
        )?;
        Ok(())
    }

    fn node_exists(&self, id: &str, node: &cairn_schema::NodeKey) -> Result<(), CommitError> {
        let select = "SELECT 1 FROM nodes WHERE graph_id = ?1 AND key = ?2";
        if self.exists(select, vec![text(id), text(node)])? {
            Ok(())
        } else {
            Err(malformed(format!("node {node} is not in the graph")))
        }
    }

    fn count(&self, sql: &str, params: Vec<Value>) -> Result<i64, CommitError> {
        let row = first(self.connection, sql, params)?;
        Ok(row.map(|row| row.int(0)).transpose()?.unwrap_or(0))
    }

    fn put_graph_record(&mut self, id: &str, record: &GraphRecord) -> Result<(), CommitError> {
        match record {
            GraphRecord::Node(node) => self.put_node(id, node),
            GraphRecord::NodeField { node, value } => {
                let held = graph::load_node(self.connection, id, node.as_str())?;
                let Some(mut changed) = held else {
                    return Err(malformed(format!("node {node} is not in the graph")));
                };
                if !value.clone().write(&mut changed) {
                    return Err(malformed(format!(
                        "node {node} has no field {:?}",
                        value.field()
                    )));
                }
                self.put_node(id, &changed)
            }
            GraphRecord::Edge(Edge { node, requires }) => self.put_edge(id, node, requires),
            GraphRecord::Role(role) => self.put_labeled("roles", id, role_row(role)),
            GraphRecord::Kind(kind) => self.put_labeled("participation_kinds", id, kind_row(kind)),
            GraphRecord::DefaultOwner(role) => {
                self.run(
                    "UPDATE graphs SET default_owner = ?2 WHERE id = ?1",
                    vec![text(id), text(role)],
                )?;
                Ok(())
            }
            GraphRecord::Participation { node, kind, source } => {
                self.node_exists(id, node)?;
                self.put_participation(id, node, kind, source)?;
                let select =
                    "SELECT count(*) FROM participations WHERE graph_id = ?1 AND node = ?2";
                let held = self.count(select, vec![text(id), text(node)])?;
                if held > i64::from(KIND_COUNT_MAX) {
                    return Err(malformed(format!("{node} has more than the kind limit")));
                }
                Ok(())
            }
            GraphRecord::Resource { node, resource } => {
                self.node_exists(id, node)?;
                self.put_resource(id, node, resource)
            }
            GraphRecord::RetiredKey(_)
            | GraphRecord::NodeState { .. }
            | GraphRecord::Tick { .. }
            | GraphRecord::Answer { .. }
            | GraphRecord::RoleFill { .. }
            | GraphRecord::Pin { .. }
            | GraphRecord::Snooze { .. }
            | GraphRecord::Overrides { .. }
            | GraphRecord::Annotation(_) => crate::write_state::put(self, id, record),
        }
    }

    fn put_edge(&self, id: &str, node: &NodeKey, requires: &NodeKey) -> Result<(), CommitError> {
        self.node_exists(id, node)?;
        self.run(
            "INSERT OR IGNORE INTO edges (graph_id, node, requires) VALUES (?1, ?2, ?3)",
            vec![text(id), text(node), text(requires)],
        )?;
        let select = "SELECT count(*) FROM edges WHERE graph_id = ?1 AND node = ?2";
        let held = self.count(select, vec![text(id), text(node)])?;
        if held > i64::from(EDGE_COUNT_PER_NODE_MAX) {
            return Err(malformed(format!(
                "{node} requires more than the edge limit"
            )));
        }
        Ok(())
    }

    /// Runs a statement for the state writes.
    pub(crate) fn statement(&self, sql: &str, params: Vec<Value>) -> Result<u64, CommitError> {
        self.run(sql, params)
    }

    /// A role's or kind's row: its key, id, title, and multi columns.
    fn put_labeled(&self, table: &str, id: &str, row: [Value; 4]) -> Result<(), CommitError> {
        let sql = format!(
            "INSERT OR REPLACE INTO {table} (graph_id, key, id, title, multi) \
             VALUES (?1, ?2, ?3, ?4, ?5)"
        );
        let mut values = vec![text(id)];
        values.extend(row);
        self.run(&sql, values)?;
        Ok(())
    }

    fn put_node(&mut self, id: &str, node: &Node<KeyRefs>) -> Result<(), CommitError> {
        let mut values = vec![text(id)];
        values.extend(graph::node_values(node)?);
        let slots: Vec<_> = (1..=values.len()).map(|slot| format!("?{slot}")).collect();
        let sql = format!(
            "INSERT OR REPLACE INTO nodes (graph_id, {}) VALUES ({})",
            NODE_COLUMNS.join(", "),
            slots.join(", ")
        );
        self.run(&sql, values)?;
        let key = &node.key;
        let params = || vec![text(id), text(key)];
        for table in [
            "edges",
            "participation_identities",
            "participations",
            "resources",
        ] {
            let sql = format!("DELETE FROM {table} WHERE graph_id = ?1 AND node = ?2");
            self.run(&sql, params())?;
        }
        for requires in node.requires.iter() {
            self.run(
                "INSERT INTO edges (graph_id, node, requires) VALUES (?1, ?2, ?3)",
                vec![text(id), text(key), text(requires)],
            )?;
        }
        for (kind, source) in node.participations.as_map() {
            self.put_participation(id, key, kind, source)?;
        }
        for (position, resource) in node.resources.iter().enumerate() {
            self.insert_resource(id, key, resource, position)?;
        }
        Ok(())
    }

    fn put_participation(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        kind: &cairn_schema::KindKey,
        source: &ParticipationSource<KeyRefs>,
    ) -> Result<(), CommitError> {
        let params = || vec![text(id), text(node), text(kind)];
        self.run(
            "DELETE FROM participation_identities WHERE graph_id = ?1 AND node = ?2 AND kind = ?3",
            params(),
        )?;
        let role = match source {
            ParticipationSource::Role(role) => text(role),
            ParticipationSource::Identities(_) => Value::Null,
        };
        let mut values = params();
        values.push(role);
        self.run(
            "INSERT OR REPLACE INTO participations (graph_id, node, kind, role) \
             VALUES (?1, ?2, ?3, ?4)",
            values,
        )?;
        if let ParticipationSource::Identities(identities) = source {
            for identity in identities.iter() {
                let mut values = params();
                values.push(text(identity));
                self.run(
                    "INSERT INTO participation_identities (graph_id, node, kind, identity) \
                     VALUES (?1, ?2, ?3, ?4)",
                    values,
                )?;
            }
        }
        Ok(())
    }

    fn put_resource(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        resource: &Resource<KeyRefs>,
    ) -> Result<(), CommitError> {
        let select =
            "SELECT position FROM resources WHERE graph_id = ?1 AND node = ?2 AND key = ?3";
        let held = first(
            self.connection,
            select,
            vec![text(id), text(node), text(&resource.key)],
        )?;
        let position = if let Some(row) = held {
            row.int(0)?
        } else {
            let select = "SELECT coalesce(max(position) + 1, 0) FROM resources \
                          WHERE graph_id = ?1 AND node = ?2";
            self.count(select, vec![text(id), text(node)])?
        };
        let position = usize::try_from(position)
            .map_err(|_| malformed(format!("resource position {position}")))?;
        self.insert_resource(id, node, resource, position)
    }

    fn insert_resource(
        &self,
        id: &str,
        node: &cairn_schema::NodeKey,
        resource: &Resource<KeyRefs>,
        position: usize,
    ) -> Result<(), CommitError> {
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
        )?;
        Ok(())
    }

    fn remove(&mut self, key: &RecordKey) -> Result<(), CommitError> {
        match key {
            RecordKey::Domain(Domain::Journey(journey)) => self.remove_journey(journey)?,
            RecordKey::Domain(Domain::Route(route)) => self.remove_route(route)?,
            RecordKey::Graph(graph @ (GraphId::Journey(_) | GraphId::RouteDraft(_))) => {
                self.clear_graph(graph)?;
            }
            RecordKey::InGraph { graph, key } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return Err(malformed(format!(
                        "{graph:?} is published and never written again"
                    )));
                }
                crate::write_state::remove(self, &graph_id(graph), key)?;
            }
            RecordKey::RouteDraft(route) => {
                self.clear_graph(&GraphId::RouteDraft(route.clone()))?;
                self.run(
                    "DELETE FROM route_drafts WHERE route_id = ?1",
                    vec![text(route)],
                )?;
            }
            RecordKey::Identity(identity) => {
                self.run(
                    "DELETE FROM identity_emails WHERE identity = ?1",
                    vec![text(identity)],
                )?;
                self.run(
                    "DELETE FROM identities WHERE key = ?1",
                    vec![text(identity)],
                )?;
            }
            RecordKey::IdentityAlias(alias) => {
                self.run(
                    "DELETE FROM identity_aliases WHERE alias = ?1",
                    vec![text(alias)],
                )?;
            }
            RecordKey::Proposal { id, .. } => {
                self.run("DELETE FROM proposals WHERE id = ?1", vec![text(id)])?;
            }
            RecordKey::Domain(Domain::Deployment)
            | RecordKey::Graph(GraphId::RouteVersion { .. })
            | RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::Deleted(_) => {
                return Err(malformed(format!("{key:?} is never removed")));
            }
        }
        Ok(())
    }

    /// A19: a hard delete: the journey's fields, graph, proposals, and events.
    fn remove_journey(&mut self, journey: &cairn_schema::JourneyId) -> Result<(), CommitError> {
        self.clear_graph(&GraphId::Journey(journey.clone()))?;
        let statements = [
            "DELETE FROM journey_owners WHERE journey_id = ?1",
            "DELETE FROM journeys WHERE id = ?1",
            "DELETE FROM proposals WHERE destination_kind = 'journey' AND destination_id = ?1",
            "DELETE FROM event_nodes WHERE seq IN \
             (SELECT seq FROM events WHERE log_kind = 'journey' AND log_id = ?1)",
            "DELETE FROM events WHERE log_kind = 'journey' AND log_id = ?1",
        ];
        for sql in statements {
            self.run(sql, vec![text(journey)])?;
        }
        Ok(())
    }

    /// A19: a hard delete of a route: its fields, draft, every version and its
    /// graph, its proposals, and its events.
    fn remove_route(&mut self, route: &RouteId) -> Result<(), CommitError> {
        let mut graphs = vec![GraphId::RouteDraft(route.clone())];
        let select = "SELECT version_number FROM route_versions WHERE route_id = ?1";
        for row in crate::sql::rows(self.connection, select, vec![text(route)])? {
            graphs.push(GraphId::RouteVersion {
                route: route.clone(),
                version: row.number(0)?,
            });
        }
        for graph in &graphs {
            self.clear_graph(graph)?;
        }
        let statements = [
            "DELETE FROM route_drafts WHERE route_id = ?1",
            "DELETE FROM route_versions WHERE route_id = ?1",
            "DELETE FROM routes WHERE id = ?1",
            "DELETE FROM proposals WHERE destination_kind = 'route' AND destination_id = ?1",
            "DELETE FROM event_nodes WHERE seq IN \
             (SELECT seq FROM events WHERE log_kind = 'route' AND log_id = ?1)",
            "DELETE FROM events WHERE log_kind = 'route' AND log_id = ?1",
        ];
        for sql in statements {
            self.run(sql, vec![text(route)])?;
        }
        Ok(())
    }

    /// Removes every record of a graph and its row.
    fn clear_graph(&mut self, graph: &GraphId) -> Result<(), CommitError> {
        let id = graph_id(graph);
        for table in GRAPH_TABLES {
            let sql = format!("DELETE FROM {table} WHERE graph_id = ?1");
            self.run(&sql, vec![text(&id)])?;
        }
        self.run("DELETE FROM graphs WHERE id = ?1", vec![text(&id)])?;
        self.graphs.remove(&id);
        Ok(())
    }

    fn copy(&mut self, from: &GraphId, to: &GraphId) -> Result<(), CommitError> {
        let target = graph_id(to);
        if matches!(to, GraphId::RouteVersion { .. })
            && self.exists("SELECT 1 FROM graphs WHERE id = ?1", vec![text(&target)])?
        {
            return Err(malformed(format!(
                "{to:?} is published and never written again"
            )));
        }
        let source = graph::load(self.connection, from)?;
        let id = self.graph_row(to)?;
        for record in cairn_store::backend::graph_records(&source) {
            self.put_graph_record(&id, &record)?;
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

/// The identities an answer names.
pub(crate) fn answer_identities(value: &AnswerValue) -> Vec<cairn_schema::IdentityKey> {
    match value {
        AnswerValue::Identity(identity) => vec![identity.clone()],
        AnswerValue::IdentityList(identities) => identities.iter().cloned().collect(),
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

/// A19: a deleted id's kind and id columns.
pub(crate) fn deleted_columns(id: &DeletedId) -> (&'static str, String) {
    match id {
        DeletedId::Journey(journey) => ("journey", journey.to_string()),
        DeletedId::Route(route) => ("route", route.to_string()),
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
