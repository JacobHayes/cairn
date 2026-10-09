//! The memory backend's records and how a write changes them: the reference semantics of a
//! change set every backend reproduces (the conformance suite holds them to it).
//!
//! A put stores a record's after-state at its address. A removal removes every record its
//! address covers: a whole node takes everything on it (its fields, edges at either end,
//! participations, resources, state, tombstone, and the notes on it), a draft takes its
//! graph, a journey's domain takes its fields, graph, and proposals. A copy puts every
//! record of one graph into another. Published versions are written once, by the copy that
//! publishes them, and routes and the deployment are never removed (A11, A19).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    BoundedSet, Deployment, Domain, Graph, GraphId, GraphKey, GraphRecord, Journey, JourneyHeader,
    JourneyId, Node, Proposal, ProposalId, Record, RecordKey, RetiredKey, Revision, Route,
    RouteDraft, RouteHeader, RouteId, RouteVersion, Timestamp, VersionNumber, Write, refs::KeyRefs,
};

use crate::commit::StoreError;

/// Every record inside the domains: what a commit stages and swaps in whole.
#[derive(Clone, Debug, Default)]
pub(crate) struct Domains {
    pub deployment: Deployment,
    pub journeys: BTreeMap<JourneyId, (JourneyHeader, Revision)>,
    pub routes: BTreeMap<RouteId, (RouteHeader, Revision)>,
    /// Open drafts and the version each extends.
    pub drafts: BTreeMap<RouteId, Option<VersionNumber>>,
    /// Published versions and when.
    pub versions: BTreeMap<(RouteId, VersionNumber), Timestamp>,
    pub graphs: BTreeMap<GraphId, Graph>,
    pub deleted_journeys: BTreeMap<JourneyId, Timestamp>,
    pub proposals: BTreeMap<ProposalId, Proposal>,
}

fn malformed<T>(reason: String) -> Result<T, StoreError> {
    Err(StoreError::Malformed(reason))
}

impl Domains {
    /// A journey as loaded.
    pub fn journey(&self, id: &JourneyId) -> Option<Journey> {
        let (header, revision) = self.journeys.get(id)?;
        Some(Journey {
            header: header.clone(),
            revision: *revision,
            graph: self.graph(&GraphId::Journey(id.clone())),
        })
    }

    /// A route with its draft, as loaded.
    pub fn route(&self, id: &RouteId) -> Option<Route> {
        let (header, revision) = self.routes.get(id)?;
        let versions = self
            .versions
            .keys()
            .filter(|(route, _)| route == id)
            .map(|(_, version)| *version)
            .collect();
        let draft = self.drafts.get(id).map(|extends| RouteDraft {
            extends: *extends,
            graph: self.graph(&GraphId::RouteDraft(id.clone())),
        });
        Some(Route {
            header: header.clone(),
            revision: *revision,
            versions,
            draft,
        })
    }

    /// A published version, as loaded.
    pub fn route_version(&self, route: &RouteId, version: VersionNumber) -> Option<RouteVersion> {
        let published_at = *self.versions.get(&(route.clone(), version))?;
        Some(RouteVersion {
            route: route.clone(),
            version,
            published_at,
            graph: self.graph(&GraphId::RouteVersion {
                route: route.clone(),
                version,
            }),
        })
    }

    pub fn graph(&self, id: &GraphId) -> Graph {
        self.graphs.get(id).cloned().unwrap_or_default()
    }

    /// A domain's or proposal's current revision; 0 when it does not exist.
    pub fn revision(&self, of: &cairn_schema::RevisionOf) -> Revision {
        use cairn_schema::RevisionOf;
        match of {
            RevisionOf::Domain(Domain::Journey(id)) => self.journeys.get(id).map(|row| row.1),
            RevisionOf::Domain(Domain::Route(id)) => self.routes.get(id).map(|row| row.1),
            RevisionOf::Domain(Domain::Deployment) => Some(self.deployment.revision),
            RevisionOf::Proposal(id) => self.proposals.get(id).map(|proposal| proposal.revision),
        }
        .unwrap_or_default()
    }

    /// E6: the journeys referring to any of `entities`, directly or through an alias, at
    /// their current revisions.
    pub fn journeys_referencing(
        &self,
        entities: &BTreeSet<cairn_schema::EntityKey>,
    ) -> BTreeMap<JourneyId, Revision> {
        let keys = crate::backend::with_aliases(entities, &self.deployment.aliases);
        self.journeys
            .iter()
            .filter(|(id, _)| {
                let graph = self.graph(&GraphId::Journey((*id).clone()));
                !crate::backend::entity_references(&graph).is_disjoint(&keys)
            })
            .map(|(id, (_, revision))| (id.clone(), *revision))
            .collect()
    }

    /// J5: the nodes an event is about or wrote on, with the nodes whose edges a whole-node
    /// removal in it takes, read before the commit.
    pub fn event_nodes(&self, event: &cairn_schema::Event) -> BTreeSet<cairn_schema::NodeKey> {
        let mut nodes = crate::backend::event_nodes(event);
        for (graph, removed) in crate::backend::removed_nodes(event) {
            if let Some(content) = self.graphs.get(&graph) {
                let requiring = content
                    .nodes
                    .values()
                    .filter(|node| node.requires.as_set().contains(&removed));
                nodes.extend(requiring.map(|node| node.key.clone()));
            }
        }
        nodes
    }

    /// A19: the proposals a commit's hard delete takes with its journey.
    pub fn proposals_of_deleted(&self, shape: &crate::backend::Shape) -> Vec<ProposalId> {
        let Some(journey) = &shape.deletes_journey else {
            return Vec::new();
        };
        let destination = Domain::Journey(journey.clone());
        self.proposals
            .values()
            .filter(|proposal| proposal.destination == destination)
            .map(|proposal| proposal.id.clone())
            .collect()
    }

    /// Applies one write.
    pub fn apply(&mut self, write: &Write) -> Result<(), StoreError> {
        match write {
            Write::Put(record) => self.put(record),
            Write::Remove(key) => self.remove(key),
            Write::CopyGraph { from, to } => self.copy(from, to),
        }
    }

    fn put(&mut self, record: &Record) -> Result<(), StoreError> {
        match record {
            Record::Graph { graph, record } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return malformed(format!("{graph:?} is published and never written again"));
                }
                put_graph_record(self.graphs.entry(graph.clone()).or_default(), record)?;
            }
            Record::JourneyHeader(header) => {
                let revision = self.journeys.get(&header.id).map(|row| row.1);
                self.journeys.insert(
                    header.id.clone(),
                    (header.clone(), revision.unwrap_or_default()),
                );
            }
            Record::RouteHeader(header) => {
                let revision = self.routes.get(&header.id).map(|row| row.1);
                self.routes.insert(
                    header.id.clone(),
                    (header.clone(), revision.unwrap_or_default()),
                );
            }
            Record::RouteDraft { route, extends } => {
                self.drafts.insert(route.clone(), *extends);
            }
            Record::RouteVersion {
                route,
                version,
                published_at,
            } => {
                if self
                    .versions
                    .insert((route.clone(), *version), *published_at)
                    .is_some()
                {
                    return malformed(format!("{route} version {version} is already published"));
                }
            }
            Record::DeletedJourney {
                journey,
                deleted_at,
            } => {
                self.deleted_journeys.insert(journey.clone(), *deleted_at);
            }
            Record::Entity(entity) => {
                self.deployment
                    .entities
                    .put(entity.clone())
                    .map_err(|error| StoreError::Malformed(format!("entities: {error}")))?;
            }
            Record::EntityAlias { alias, entity } => {
                self.deployment
                    .aliases
                    .insert(alias.clone(), entity.clone());
            }
            Record::Proposal(proposal) => {
                self.proposals.insert(proposal.id.clone(), proposal.clone());
            }
        }
        Ok(())
    }

    fn remove(&mut self, key: &RecordKey) -> Result<(), StoreError> {
        match key {
            RecordKey::Domain(Domain::Journey(journey)) => {
                self.journeys.remove(journey);
                self.graphs.remove(&GraphId::Journey(journey.clone()));
                let domain = Domain::Journey(journey.clone());
                self.proposals
                    .retain(|_, proposal| proposal.destination != domain);
            }
            RecordKey::Graph(graph @ (GraphId::Journey(_) | GraphId::RouteDraft(_))) => {
                self.graphs.remove(graph);
            }
            RecordKey::InGraph { graph, key } => {
                if matches!(graph, GraphId::RouteVersion { .. }) {
                    return malformed(format!("{graph:?} is published and never written again"));
                }
                if let Some(content) = self.graphs.get_mut(graph) {
                    remove_graph_key(content, key)?;
                }
            }
            RecordKey::RouteDraft(route) => {
                self.drafts.remove(route);
                self.graphs.remove(&GraphId::RouteDraft(route.clone()));
            }
            RecordKey::Entity(entity) => {
                self.deployment.entities.remove(entity);
            }
            RecordKey::EntityAlias(alias) => {
                self.deployment.aliases.remove(alias);
            }
            RecordKey::Proposal { id, .. } => {
                self.proposals.remove(id);
            }
            RecordKey::Domain(_)
            | RecordKey::Graph(GraphId::RouteVersion { .. })
            | RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::DeletedJourney(_) => {
                return malformed(format!("{key:?} is never removed (A11, A19)"));
            }
        }
        Ok(())
    }

    fn copy(&mut self, from: &GraphId, to: &GraphId) -> Result<(), StoreError> {
        if matches!(to, GraphId::RouteVersion { .. }) && self.graphs.contains_key(to) {
            return malformed(format!("{to:?} is published and never written again"));
        }
        let source = self.graph(from);
        let target = self.graphs.entry(to.clone()).or_default();
        for record in crate::backend::graph_records(&source) {
            put_graph_record(target, &record)?;
        }
        Ok(())
    }
}

/// A copy of a node to change and put back.
fn node_copy(graph: &Graph, key: &cairn_schema::NodeKey) -> Result<Node<KeyRefs>, StoreError> {
    match graph.nodes.get(key) {
        Some(node) => Ok(node.clone()),
        None => malformed(format!("node {key} is not in the graph")),
    }
}

fn put_node(graph: &mut Graph, node: Node<KeyRefs>) -> Result<(), StoreError> {
    graph
        .nodes
        .put(node)
        .map(|_| ())
        .map_err(|error| StoreError::Malformed(format!("nodes: {error}")))
}

fn put_graph_record(graph: &mut Graph, record: &GraphRecord) -> Result<(), StoreError> {
    let collection =
        |error: cairn_schema::CollectionError| StoreError::Malformed(error.to_string());
    match record {
        GraphRecord::Node(node) => put_node(graph, node.clone())?,
        GraphRecord::NodeField { node, value } => {
            let mut changed = node_copy(graph, node)?;
            if !value.clone().write(&mut changed) {
                return malformed(format!("node {node} has no field {:?}", value.field()));
            }
            put_node(graph, changed)?;
        }
        GraphRecord::Edge(edge) => {
            let mut changed = node_copy(graph, &edge.node)?;
            let mut requires = changed.requires.as_set().clone();
            requires.insert(edge.requires.clone());
            changed.requires = BoundedSet::new(requires).map_err(collection)?;
            put_node(graph, changed)?;
        }
        GraphRecord::Role(role) => {
            graph.roles.put(role.clone()).map_err(collection)?;
        }
        GraphRecord::Kind(kind) => {
            graph
                .participation_kinds
                .put(kind.clone())
                .map_err(collection)?;
        }
        GraphRecord::DefaultOwner(role) => graph.default_owner = Some(role.clone()),
        GraphRecord::Participation { node, kind, source } => {
            let mut changed = node_copy(graph, node)?;
            let mut participations = changed.participations.as_map().clone();
            participations.insert(kind.clone(), source.clone());
            changed.participations =
                participations
                    .try_into()
                    .map_err(|error: cairn_schema::LimitExceeded| {
                        StoreError::Malformed(error.to_string())
                    })?;
            put_node(graph, changed)?;
        }
        GraphRecord::Resource { node, resource } => {
            let mut changed = node_copy(graph, node)?;
            match changed
                .resources
                .iter_mut()
                .find(|held| held.key == resource.key)
            {
                Some(held) => *held = resource.clone(),
                None => changed.resources.push(resource.clone()),
            }
            put_node(graph, changed)?;
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
        | GraphRecord::Annotation(_) => put_state_record(graph, record)?,
    }
    Ok(())
}

fn put_state_record(graph: &mut Graph, record: &GraphRecord) -> Result<(), StoreError> {
    let state = &mut graph.state;
    match record {
        GraphRecord::RetiredKey(RetiredKey::Node(key)) => {
            graph.retired_keys.nodes.insert(key.clone());
        }
        GraphRecord::RetiredKey(RetiredKey::Role(key)) => {
            graph.retired_keys.roles.insert(key.clone());
        }
        GraphRecord::RetiredKey(RetiredKey::Kind(key)) => {
            graph.retired_keys.kinds.insert(key.clone());
        }
        GraphRecord::NodeState { node, state: value } => {
            state.nodes.insert(node.clone(), value.clone());
        }
        GraphRecord::LocalEdit { node, edit } => {
            state
                .local_edits
                .entry(node.clone())
                .or_default()
                .insert(edit.clone());
        }
        GraphRecord::Answer {
            decision,
            value,
            rationale,
        } => {
            state.answers.insert(decision.clone(), value.clone());
            match rationale {
                Some(rationale) => state.rationales.insert(decision.clone(), rationale.clone()),
                None => state.rationales.remove(decision),
            };
        }
        GraphRecord::RoleFill { role, entities } => {
            state.role_fills.insert(role.clone(), entities.clone());
        }
        GraphRecord::Pin { node, date } => {
            state.pins.insert(node.clone(), *date);
        }
        GraphRecord::Snooze { node, until } => {
            state.snoozes.insert(node.clone(), until.clone());
        }
        GraphRecord::Overrides { node, overrides } => {
            state.overrides.insert(node.clone(), overrides.clone());
        }
        GraphRecord::Tombstone(node) => {
            state.tombstones.insert(node.clone());
        }
        GraphRecord::Annotation(annotation) => {
            state
                .annotations
                .put(annotation.clone())
                .map_err(|error| StoreError::Malformed(format!("annotations: {error}")))?;
        }
        GraphRecord::Node(_)
        | GraphRecord::NodeField { .. }
        | GraphRecord::Edge(_)
        | GraphRecord::Role(_)
        | GraphRecord::Kind(_)
        | GraphRecord::DefaultOwner(_)
        | GraphRecord::Participation { .. }
        | GraphRecord::Resource { .. } => {
            return malformed(format!("{record:?} is graph content, not state"));
        }
    }
    Ok(())
}

fn remove_graph_key(graph: &mut Graph, key: &GraphKey) -> Result<(), StoreError> {
    match key {
        GraphKey::Node(node) => remove_node(graph, node)?,
        GraphKey::NodeField { node, field } => {
            return malformed(format!("field {field:?} of {node} is written, not removed"));
        }
        GraphKey::Edge(edge) => {
            if let Some(held) = graph.nodes.get(&edge.node) {
                let mut changed = held.clone();
                let mut requires = changed.requires.as_set().clone();
                requires.remove(&edge.requires);
                changed.requires = BoundedSet::new(requires)
                    .map_err(|error| StoreError::Malformed(error.to_string()))?;
                put_node(graph, changed)?;
            }
        }
        GraphKey::Role(role) => {
            graph.roles.remove(role);
        }
        GraphKey::Kind(kind) => {
            graph.participation_kinds.remove(kind);
        }
        GraphKey::DefaultOwner => graph.default_owner = None,
        GraphKey::Participation { node, kind } => {
            if let Some(held) = graph.nodes.get(node) {
                let mut changed = held.clone();
                let mut participations = changed.participations.as_map().clone();
                participations.remove(kind);
                changed.participations =
                    participations
                        .try_into()
                        .map_err(|error: cairn_schema::LimitExceeded| {
                            StoreError::Malformed(error.to_string())
                        })?;
                put_node(graph, changed)?;
            }
        }
        GraphKey::Resource { node, resource } => {
            if let Some(held) = graph.nodes.get(node) {
                let mut changed = held.clone();
                changed.resources.retain(|held| held.key != *resource);
                put_node(graph, changed)?;
            }
        }
        GraphKey::RetiredKey(_)
        | GraphKey::NodeState(_)
        | GraphKey::LocalEdit { .. }
        | GraphKey::Answer(_)
        | GraphKey::RoleFill(_)
        | GraphKey::Pin(_)
        | GraphKey::Snooze(_)
        | GraphKey::Overrides(_)
        | GraphKey::Tombstone(_)
        | GraphKey::Annotation { .. } => remove_state_key(graph, key),
    }
    Ok(())
}

fn remove_state_key(graph: &mut Graph, key: &GraphKey) {
    let state = &mut graph.state;
    match key {
        GraphKey::RetiredKey(RetiredKey::Node(node)) => {
            graph.retired_keys.nodes.remove(node);
        }
        GraphKey::RetiredKey(RetiredKey::Role(role)) => {
            graph.retired_keys.roles.remove(role);
        }
        GraphKey::RetiredKey(RetiredKey::Kind(kind)) => {
            graph.retired_keys.kinds.remove(kind);
        }
        GraphKey::NodeState(node) => {
            state.nodes.remove(node);
        }
        GraphKey::LocalEdit { node, edit } => {
            if let Some(edits) = state.local_edits.get_mut(node) {
                edits.remove(edit);
                if edits.is_empty() {
                    state.local_edits.remove(node);
                }
            }
        }
        GraphKey::Answer(node) => {
            state.answers.remove(node);
            state.rationales.remove(node);
        }
        GraphKey::RoleFill(role) => {
            state.role_fills.remove(role);
        }
        GraphKey::Pin(node) => {
            state.pins.remove(node);
        }
        GraphKey::Snooze(node) => {
            state.snoozes.remove(node);
        }
        GraphKey::Overrides(node) => {
            state.overrides.remove(node);
        }
        GraphKey::Tombstone(node) => {
            state.tombstones.remove(node);
        }
        GraphKey::Annotation {
            annotation,
            node: _,
        } => {
            state.annotations.remove(annotation);
        }
        // Content keys are handled by remove_graph_key.
        GraphKey::Node(_)
        | GraphKey::NodeField { .. }
        | GraphKey::Edge(_)
        | GraphKey::Role(_)
        | GraphKey::Kind(_)
        | GraphKey::DefaultOwner
        | GraphKey::Participation { .. }
        | GraphKey::Resource { .. } => {}
    }
}

/// A whole node's removal: the node, the edges into it, and everything on it.
fn remove_node(graph: &mut Graph, node: &cairn_schema::NodeKey) -> Result<(), StoreError> {
    graph.nodes.remove(node);
    let requiring: Vec<Node<KeyRefs>> = graph
        .nodes
        .values()
        .filter(|held| held.requires.as_set().contains(node))
        .cloned()
        .collect();
    for mut held in requiring {
        let mut requires = held.requires.as_set().clone();
        requires.remove(node);
        held.requires =
            BoundedSet::new(requires).map_err(|error| StoreError::Malformed(error.to_string()))?;
        put_node(graph, held)?;
    }
    let state = &mut graph.state;
    state.nodes.remove(node);
    state.local_edits.remove(node);
    state.answers.remove(node);
    state.rationales.remove(node);
    state.pins.remove(node);
    state.snoozes.remove(node);
    state.overrides.remove(node);
    state.tombstones.remove(node);
    let notes: BTreeSet<_> = state
        .annotations
        .values()
        .filter(|annotation| annotation.body.node.as_ref() == Some(node))
        .map(|annotation| annotation.body.key.clone())
        .collect();
    for key in notes {
        state.annotations.remove(&key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{self, id, node_key};
    use cairn_schema::{Edge, GraphKey, NodeState, Provenance, State};

    fn journey_with_two_nodes() -> Domains {
        let graph = build::journey_graph("j_one");
        let mut domains = Domains::default();
        let writes = [
            Write::Put(Record::JourneyHeader(build::journey_header(
                "j_one", "One", None,
            ))),
            build::put_in(&graph, GraphRecord::Node(build::action("n_a", "a", None))),
            build::put_in(&graph, GraphRecord::Node(build::action("n_b", "b", None))),
            build::put_in(
                &graph,
                GraphRecord::Edge(Edge {
                    node: node_key("n_b"),
                    requires: node_key("n_a"),
                }),
            ),
            build::put_in(
                &graph,
                GraphRecord::NodeState {
                    node: node_key("n_a"),
                    state: NodeState {
                        state: State::Active,
                        provenance: Provenance::Local,
                        atomic: false,
                        started_on: None,
                        finished_on: None,
                        skip_reason: None,
                    },
                },
            ),
            build::put_in(
                &graph,
                GraphRecord::Annotation(build::note("a_on_a", Some("n_a"), "x")),
            ),
            build::put_in(
                &graph,
                GraphRecord::Annotation(build::note("a_journey", None, "y")),
            ),
        ];
        for write in &writes {
            domains.apply(write).unwrap();
        }
        domains
    }

    #[test]
    fn removing_a_node_takes_its_incoming_edges_state_and_notes() {
        let mut domains = journey_with_two_nodes();
        let graph = build::journey_graph("j_one");
        domains
            .apply(&build::remove_in(&graph, GraphKey::Node(node_key("n_a"))))
            .unwrap();
        let after = domains.graph(&graph);
        let remaining: Vec<_> = after.nodes.values().map(|node| node.key.clone()).collect();
        assert_eq!(remaining, vec![node_key("n_b")]);
        assert!(
            after
                .nodes
                .get(&node_key("n_b"))
                .unwrap()
                .requires
                .is_empty()
        );
        assert!(after.state.nodes.is_empty());
        let notes: Vec<_> = after
            .state
            .annotations
            .values()
            .map(|note| note.body.key.clone())
            .collect();
        assert_eq!(notes, vec![id("a_journey")]);
    }

    #[test]
    fn a_field_write_to_a_kind_without_the_field_is_malformed() {
        let mut domains = journey_with_two_nodes();
        let write = build::put_in(
            &build::journey_graph("j_one"),
            GraphRecord::NodeField {
                node: node_key("n_a"),
                value: cairn_schema::NodeFieldValue::Final(true),
            },
        );
        assert!(matches!(
            domains.apply(&write),
            Err(StoreError::Malformed(_))
        ));
    }

    #[test]
    fn a_published_version_is_written_once() {
        let mut domains = Domains::default();
        let route: RouteId = id("vendor");
        let copy = Write::CopyGraph {
            from: GraphId::RouteDraft(route.clone()),
            to: GraphId::RouteVersion {
                route: route.clone(),
                version: build::version(1),
            },
        };
        domains.apply(&copy).unwrap();
        assert!(matches!(
            domains.apply(&copy),
            Err(StoreError::Malformed(_))
        ));
        let rewrite = build::put_in(
            &GraphId::RouteVersion {
                route,
                version: build::version(1),
            },
            GraphRecord::Tombstone(node_key("n_a")),
        );
        assert!(matches!(
            domains.apply(&rewrite),
            Err(StoreError::Malformed(_))
        ));
    }
}
