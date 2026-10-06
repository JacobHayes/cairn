//! The stored records a patch reads and writes (ARCHITECTURE, Storage > Schema outline), held
//! in memory: the deployment, journeys, routes with their drafts, published route versions,
//! proposals, and deleted journey ids. A host fills a [`Records`] with what it loaded for one
//! patch; the scenario harness and replay hold every domain in one.
//!
//! [`Records::write`] applies one [`Write`] the way a store persists it, and
//! [`Records::advance`] moves the revisions one patch's events advance. Apply builds its
//! candidate with exactly these two functions and replay rebuilds state with them, so replay
//! is exact by construction (J3) and a store that maps the same writes to rows holds the
//! same state.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    Deployment, Domain, EventType, Graph, GraphId, GraphKey, GraphRecord, Journey, JourneyId,
    KeyRefs, Lineage, Node, NodeKey, Proposal, ProposalId, Record, RecordKey, RetiredKey, Route,
    RouteDraft, RouteId, RouteVersion, Timestamp, Write,
};

use crate::edit;

/// Every stored record a patch may read or write, by domain. For one patch a host loads the
/// target domain (absent when the patch creates it), the deployment (always: entity
/// references resolve through it), the route versions the patch reads, the proposal it edits
/// or applies, and for an entity merge the journeys referencing either entity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    /// The deployment: entities, aliases, revision (E6).
    pub deployment: Deployment,
    /// Journeys by id, each with its graph and state.
    pub journeys: BTreeMap<JourneyId, Journey>,
    /// Routes by id, each with its draft; versions are kept apart.
    pub routes: BTreeMap<RouteId, Route>,
    /// Published route versions, by route and number (immutable, A11).
    pub versions: BTreeMap<Lineage, RouteVersion>,
    /// Proposals by id (I6).
    pub proposals: BTreeMap<ProposalId, Proposal>,
    /// Hard-deleted journey ids and when they were deleted (A19).
    pub deleted_journeys: BTreeMap<JourneyId, Timestamp>,
}

impl Records {
    /// The graph a [`GraphId`] names, if it exists.
    #[must_use]
    pub fn graph(&self, id: &GraphId) -> Option<&Graph> {
        match id {
            GraphId::Journey(journey) => self.journeys.get(journey).map(|journey| &journey.graph),
            GraphId::RouteDraft(route) => self
                .routes
                .get(route)
                .and_then(|route| route.draft.as_ref())
                .map(|draft| &draft.graph),
            GraphId::RouteVersion { route, version } => self
                .versions
                .get(&Lineage {
                    route: route.clone(),
                    version: *version,
                })
                .map(|version| &version.graph),
        }
    }

    fn graph_mut(&mut self, id: &GraphId) -> &mut Graph {
        let graph = match id {
            GraphId::Journey(journey) => self
                .journeys
                .get_mut(journey)
                .map(|journey| &mut journey.graph),
            GraphId::RouteDraft(route) => self
                .routes
                .get_mut(route)
                .and_then(|route| route.draft.as_mut())
                .map(|draft| &mut draft.graph),
            GraphId::RouteVersion { route, version } => self
                .versions
                .get_mut(&Lineage {
                    route: route.clone(),
                    version: *version,
                })
                .map(|version| &mut version.graph),
        };
        match graph {
            Some(graph) => graph,
            None => panic!("a write addresses graph {id:?}, which does not exist"),
        }
    }

    /// A domain's revision: 0 when it does not exist yet (A17).
    #[must_use]
    pub fn revision(&self, domain: &Domain) -> cairn_schema::Revision {
        match domain {
            Domain::Journey(id) => self
                .journeys
                .get(id)
                .map_or(cairn_schema::Revision::NONE, |journey| journey.revision),
            Domain::Route(id) => self
                .routes
                .get(id)
                .map_or(cairn_schema::Revision::NONE, |route| route.revision),
            Domain::Deployment => self.deployment.revision,
        }
    }

    /// Applies one write as a store persists it: a put replaces the record at its address, a
    /// removal drops every record at or under its address, and a copy duplicates a graph.
    ///
    /// # Panics
    ///
    /// When a write addresses a graph or record that cannot exist at that point (a write
    /// list the engine never produces).
    pub fn write(&mut self, write: &Write) {
        match write {
            Write::Put(record) => self.put(record),
            Write::Remove(key) => self.remove(key),
            Write::CopyGraph { from, to } => {
                let copied = match self.graph(from) {
                    Some(graph) => graph.clone(),
                    None => panic!("a copy reads graph {from:?}, which does not exist"),
                };
                *self.graph_mut(to) = copied;
            }
        }
    }

    fn put(&mut self, record: &Record) {
        match record {
            Record::Graph { graph, record } => put_in_graph(self.graph_mut(graph), record),
            Record::JourneyHeader(header) => {
                let journey = self
                    .journeys
                    .entry(header.id.clone())
                    .or_insert_with(|| Journey {
                        header: header.clone(),
                        revision: cairn_schema::Revision::NONE,
                        graph: Graph::default(),
                    });
                journey.header = header.clone();
            }
            Record::RouteHeader(header) => {
                let route = self
                    .routes
                    .entry(header.id.clone())
                    .or_insert_with(|| Route {
                        header: header.clone(),
                        revision: cairn_schema::Revision::NONE,
                        versions: BTreeSet::new(),
                        draft: None,
                    });
                route.header = header.clone();
            }
            Record::RouteDraft { route, extends } => {
                let route = self.route_mut(route);
                let graph = route
                    .draft
                    .take()
                    .map(|draft| draft.graph)
                    .unwrap_or_default();
                route.draft = Some(RouteDraft {
                    extends: *extends,
                    graph,
                });
            }
            other => self.put_outside_graphs(other),
        }
    }

    fn put_outside_graphs(&mut self, record: &Record) {
        match record {
            Record::RouteVersion {
                route,
                version,
                published_at,
            } => {
                self.route_mut(route).versions.insert(*version);
                let lineage = Lineage {
                    route: route.clone(),
                    version: *version,
                };
                self.versions.insert(
                    lineage,
                    RouteVersion {
                        route: route.clone(),
                        version: *version,
                        published_at: *published_at,
                        graph: Graph::default(),
                    },
                );
            }
            Record::DeletedJourney {
                journey,
                deleted_at,
            } => {
                self.deleted_journeys.insert(journey.clone(), *deleted_at);
            }
            Record::Entity(entity) => {
                let put = self.deployment.entities.put(entity.clone());
                assert!(put.is_ok(), "an entity write past the document cap");
            }
            Record::EntityAlias { alias, entity } => {
                self.deployment
                    .aliases
                    .insert(alias.clone(), entity.clone());
            }
            Record::Proposal(proposal) => {
                self.proposals.insert(proposal.id.clone(), proposal.clone());
            }
            Record::Graph { .. }
            | Record::JourneyHeader(_)
            | Record::RouteHeader(_)
            | Record::RouteDraft { .. } => unreachable!("handled by Records::put"),
        }
    }

    fn route_mut(&mut self, route: &RouteId) -> &mut Route {
        match self.routes.get_mut(route) {
            Some(route) => route,
            None => panic!("a write addresses route {route}, which does not exist"),
        }
    }

    fn remove(&mut self, key: &RecordKey) {
        match key {
            RecordKey::Domain(domain) => self.remove_domain(domain),
            RecordKey::Graph(graph) => *self.graph_mut(graph) = Graph::default(),
            RecordKey::InGraph { graph, key } => remove_in_graph(self.graph_mut(graph), key),
            RecordKey::RouteDraft(route) => self.route_mut(route).draft = None,
            RecordKey::Entity(entity) => {
                self.deployment.entities.remove(entity);
            }
            RecordKey::EntityAlias(alias) => {
                self.deployment.aliases.remove(alias);
            }
            RecordKey::Proposal { id, .. } => {
                self.proposals.remove(id);
            }
            RecordKey::JourneyHeader(_)
            | RecordKey::RouteHeader(_)
            | RecordKey::RouteVersion { .. }
            | RecordKey::DeletedJourney(_) => {
                panic!("{key:?} is never removed on its own (A11, A19)")
            }
        }
    }

    fn remove_domain(&mut self, domain: &Domain) {
        match domain {
            Domain::Journey(journey) => {
                self.journeys.remove(journey);
            }
            Domain::Route(route) => {
                self.routes.remove(route);
                self.versions.retain(|lineage, _| lineage.route != *route);
            }
            Domain::Deployment => {
                let revision = self.deployment.revision;
                self.deployment = Deployment {
                    revision,
                    ..Deployment::default()
                };
            }
        }
    }

    /// Advances the revisions one patch's events move (A17, H5; DECISIONS.md, entity creates
    /// in another domain's patch): the patch's domain advances by one, unless the patch only
    /// edits a proposal, whose record carries its own revision; a patch to another domain
    /// that writes deployment records (an entity create, a deleted journey id) also advances
    /// the deployment by one.
    pub fn advance(&mut self, events: &[cairn_schema::Event]) {
        let Some(first) = events.first() else {
            return;
        };
        let proposal_edit = matches!(
            first.event_type,
            EventType::ProposalCreated | EventType::ProposalEdited | EventType::ProposalDiscarded
        );
        if proposal_edit {
            return;
        }
        // A deletion's event sits in the deployment log; the patch's domain is the journey.
        let domain = events
            .iter()
            .find(|event| event.event_type != EventType::JourneyDeleted)
            .map(|event| event.log.clone());
        match &domain {
            Some(Domain::Journey(id)) => {
                if let Some(journey) = self.journeys.get_mut(id) {
                    journey.revision = journey.revision.next();
                }
            }
            Some(Domain::Route(id)) => {
                if let Some(route) = self.routes.get_mut(id) {
                    route.revision = route.revision.next();
                }
            }
            Some(Domain::Deployment) | None => {}
        }
        let writes_deployment = events
            .iter()
            .flat_map(|event| &event.delta)
            .any(writes_deployment_record);
        if domain == Some(Domain::Deployment) || writes_deployment {
            self.deployment.revision = self.deployment.revision.next();
        }
    }
}

/// True for a put of a record in the deployment domain.
fn writes_deployment_record(write: &Write) -> bool {
    match write {
        Write::Put(record) => record.key().domain() == Domain::Deployment,
        Write::Remove(_) | Write::CopyGraph { .. } => false,
    }
}

fn put_in_graph(graph: &mut Graph, record: &GraphRecord) {
    match record {
        GraphRecord::Node(node) => {
            let put = graph.nodes.put(node.clone());
            assert!(put.is_ok(), "a node write past node_count_max");
        }
        GraphRecord::NodeField { node, value } => {
            let written = edit::update_node(graph, node, |node| value.clone().write(node));
            assert!(
                written,
                "a field write on a node whose kind lacks the field"
            );
        }
        GraphRecord::Edge(edge) => {
            let added = edit::update_node(graph, &edge.node, |node| {
                edit::add_requirement(node, &edge.requires)
            });
            assert!(added, "an edge write past edge_count_per_node_max");
        }
        GraphRecord::Role(role) => {
            let put = graph.roles.put(role.clone());
            assert!(put.is_ok(), "a role write past role_count_max");
        }
        GraphRecord::Kind(kind) => {
            let put = graph.participation_kinds.put(kind.clone());
            assert!(put.is_ok(), "a kind write past kind_count_max");
        }
        GraphRecord::DefaultOwner(role) => graph.default_owner = Some(role.clone()),
        GraphRecord::Participation { node, kind, source } => {
            let set = edit::update_node(graph, node, |node| {
                edit::set_participation(node, kind, Some(source.clone()))
            });
            assert!(set, "a participation write past kind_count_max");
        }
        GraphRecord::Resource { node, resource } => {
            edit::update_node(graph, node, |node| {
                edit::put_resource(node, resource.clone());
            });
        }
        GraphRecord::RetiredKey(key) => {
            match key {
                RetiredKey::Node(key) => graph.retired_keys.nodes.insert(key.clone()),
                RetiredKey::Role(key) => graph.retired_keys.roles.insert(key.clone()),
                RetiredKey::Kind(key) => graph.retired_keys.kinds.insert(key.clone()),
            };
        }
        state => put_state(graph, state),
    }
}

fn put_state(graph: &mut Graph, record: &GraphRecord) {
    let state = &mut graph.state;
    match record {
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
        GraphRecord::Answer { decision, value } => {
            state.answers.insert(decision.clone(), value.clone());
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
            let put = state.annotations.put(annotation.clone());
            assert!(put.is_ok(), "an annotation write past the document cap");
        }
        GraphRecord::Node(_)
        | GraphRecord::NodeField { .. }
        | GraphRecord::Edge(_)
        | GraphRecord::Role(_)
        | GraphRecord::Kind(_)
        | GraphRecord::DefaultOwner(_)
        | GraphRecord::Participation { .. }
        | GraphRecord::Resource { .. }
        | GraphRecord::RetiredKey(_) => unreachable!("handled by put_in_graph"),
    }
}

fn remove_in_graph(graph: &mut Graph, key: &GraphKey) {
    match key {
        GraphKey::Node(node) => remove_node(graph, node),
        GraphKey::Edge(edge) => {
            edit::update_node(graph, &edge.node, |node| {
                edit::remove_requirement(node, &edge.requires);
            });
        }
        GraphKey::Role(role) => {
            graph.roles.remove(role);
        }
        GraphKey::Kind(kind) => {
            graph.participation_kinds.remove(kind);
        }
        GraphKey::DefaultOwner => graph.default_owner = None,
        GraphKey::Participation { node, kind } => {
            edit::update_node(graph, node, |node| {
                edit::set_participation(node, kind, None)
            });
        }
        GraphKey::Resource { node, resource } => {
            edit::update_node(graph, node, |node| {
                node.resources.retain(|existing| existing.key != *resource);
            });
        }
        GraphKey::NodeField { .. } | GraphKey::RetiredKey(_) => {
            panic!("{key:?} is never removed: a field is written, a retired key is kept")
        }
        state => remove_state(graph, state),
    }
}

fn remove_state(graph: &mut Graph, key: &GraphKey) {
    let state = &mut graph.state;
    match key {
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
        GraphKey::Annotation { annotation, .. } => {
            state.annotations.remove(annotation);
        }
        GraphKey::Node(_)
        | GraphKey::NodeField { .. }
        | GraphKey::Edge(_)
        | GraphKey::Role(_)
        | GraphKey::Kind(_)
        | GraphKey::DefaultOwner
        | GraphKey::Participation { .. }
        | GraphKey::Resource { .. }
        | GraphKey::RetiredKey(_) => unreachable!("handled by remove_in_graph"),
    }
}

/// Removes a node and every record under it: its edges at both ends (an edge belongs to
/// both, ARCHITECTURE: touched sets) and its state, notes, and links. Its retired key and any
/// tombstone are written after.
fn remove_node(graph: &mut Graph, key: &NodeKey) {
    graph.nodes.remove(key);
    let dependents: Vec<NodeKey> = graph
        .nodes
        .values()
        .filter(|node: &&Node<KeyRefs>| node.requires.as_set().contains(key))
        .map(|node| node.key.clone())
        .collect();
    for dependent in dependents {
        edit::update_node(graph, &dependent, |node| {
            edit::remove_requirement(node, key);
        });
    }
    let state = &mut graph.state;
    state.nodes.remove(key);
    state.local_edits.remove(key);
    state.answers.remove(key);
    state.pins.remove(key);
    state.snoozes.remove(key);
    state.overrides.remove(key);
    state.tombstones.remove(key);
    let notes: Vec<_> = state
        .annotations
        .values()
        .filter(|annotation| annotation.body.node.as_ref() == Some(key))
        .map(|annotation| annotation.body.key.clone())
        .collect();
    for note in notes {
        state.annotations.remove(&note);
    }
}
