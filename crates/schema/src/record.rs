//! Stored records and the writes that change them. An event's delta is the list of writes
//! its mutation made, as after-state (J1), so replay applies them in order with nothing
//! re-derived and is exact by construction (J3); a change set is the events of one patch,
//! so a store persists exactly what replay would rebuild. Each record has an address: by
//! key and, for node fields, by field, the granularity of the touched set (H5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::attachment::{Annotation, Resource};
use crate::domain::{Domain, Entity, GraphId, JourneyHeader, RouteHeader};
use crate::field::{NodeField, NodeFieldValue};
use crate::graph::{Edge, ParticipationKind, Role};
use crate::id::{
    AttachmentKey, EntityKey, JourneyId, KindKey, NodeKey, ProposalId, RoleKey, RouteId,
};
use crate::node::{EntitySet, Node, ParticipationSource};
use crate::number::VersionNumber;
use crate::proposal::Proposal;
use crate::refs::KeyRefs;
use crate::state::{AnswerValue, LocalEdit, NodeState, Overrides, SnoozeTarget};
use crate::text::Markdown;
use jiff::Timestamp;
use jiff::civil::Date;

/// A key a graph has retired (PRD Invariants: never reused).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RetiredKey {
    /// A node key.
    Node(NodeKey),
    /// A role key.
    Role(RoleKey),
    /// A participation kind key.
    Kind(KindKey),
}

/// A record inside one graph: its content or, for a journey, its state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum GraphRecord {
    /// A whole node.
    Node(Node<KeyRefs>),
    /// One field of a node.
    NodeField {
        /// The node.
        node: NodeKey,
        /// The field's value.
        value: NodeFieldValue<KeyRefs>,
    },
    /// An explicit edge.
    Edge(Edge),
    /// A role.
    Role(Role<KeyRefs>),
    /// A participation kind.
    Kind(ParticipationKind<KeyRefs>),
    /// The graph's `default_owner`.
    DefaultOwner(RoleKey),
    /// A node's participation of one kind.
    Participation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
        /// The source.
        source: ParticipationSource<KeyRefs>,
    },
    /// A resource on a node.
    Resource {
        /// The node.
        node: NodeKey,
        /// The resource.
        resource: Resource<KeyRefs>,
    },
    /// A retired key.
    RetiredKey(RetiredKey),
    /// A node's stored state.
    NodeState {
        /// The node.
        node: NodeKey,
        /// The state.
        state: NodeState,
    },
    /// A local-edit marker.
    LocalEdit {
        /// The node.
        node: NodeKey,
        /// The marker.
        edit: LocalEdit,
    },
    /// An answer, with the rationale it was given with (B2).
    Answer {
        /// The decision.
        decision: NodeKey,
        /// The answer.
        value: AnswerValue,
        /// Why it was given; none when the answer gave no reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rationale: Option<Markdown>,
    },
    /// A direct role fill.
    RoleFill {
        /// The role.
        role: RoleKey,
        /// The entities.
        entities: EntitySet,
    },
    /// A pin.
    Pin {
        /// The node.
        node: NodeKey,
        /// The date.
        date: Date,
    },
    /// A snooze.
    Snooze {
        /// The node.
        node: NodeKey,
        /// What it waits for.
        until: SnoozeTarget,
    },
    /// A node's overrides.
    Overrides {
        /// The node.
        node: NodeKey,
        /// The overrides.
        overrides: Overrides,
    },
    /// A tombstone: a removed route-copied node.
    Tombstone(NodeKey),
    /// A note or link.
    Annotation(Annotation),
}

/// One stored record, as after-state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum Record {
    /// A record inside a graph.
    Graph {
        /// The graph.
        graph: GraphId,
        /// The record.
        record: GraphRecord,
    },
    /// A journey's own fields.
    JourneyHeader(JourneyHeader),
    /// A route's own fields.
    RouteHeader(RouteHeader),
    /// A route's open draft.
    RouteDraft {
        /// The route.
        route: RouteId,
        /// The version it extends.
        #[serde(skip_serializing_if = "Option::is_none")]
        extends: Option<VersionNumber>,
    },
    /// A published route version.
    RouteVersion {
        /// The route.
        route: RouteId,
        /// The version.
        version: VersionNumber,
        /// When it was published.
        published_at: Timestamp,
    },
    /// A hard-deleted journey's id, never reused (A19).
    DeletedJourney {
        /// The journey.
        journey: JourneyId,
        /// When it was deleted.
        deleted_at: Timestamp,
    },
    /// An entity.
    Entity(Entity),
    /// A merged entity's old key and the entity it resolves to (E6).
    EntityAlias {
        /// The old key.
        alias: EntityKey,
        /// The entity it resolves to.
        entity: EntityKey,
    },
    /// A proposal.
    Proposal(Proposal),
}

/// The address of a record inside a graph, or of a whole node with everything on it.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum GraphKey {
    /// A node and everything attached to it.
    Node(NodeKey),
    /// One field of a node.
    NodeField {
        /// The node.
        node: NodeKey,
        /// The field.
        field: NodeField,
    },
    /// An explicit edge.
    Edge(Edge),
    /// A role.
    Role(RoleKey),
    /// A participation kind.
    Kind(KindKey),
    /// The graph's `default_owner`.
    DefaultOwner,
    /// A node's participation of one kind.
    Participation {
        /// The node.
        node: NodeKey,
        /// The kind.
        kind: KindKey,
    },
    /// A resource.
    Resource {
        /// The node.
        node: NodeKey,
        /// The resource.
        resource: AttachmentKey,
    },
    /// A retired key.
    RetiredKey(RetiredKey),
    /// A node's stored state.
    NodeState(NodeKey),
    /// A local-edit marker.
    LocalEdit {
        /// The node.
        node: NodeKey,
        /// The marker.
        edit: LocalEdit,
    },
    /// An answer.
    Answer(NodeKey),
    /// A direct role fill.
    RoleFill(RoleKey),
    /// A pin.
    Pin(NodeKey),
    /// A snooze.
    Snooze(NodeKey),
    /// A node's overrides.
    Overrides(NodeKey),
    /// A tombstone.
    Tombstone(NodeKey),
    /// A note or link, with the node it is on when the address knows it.
    Annotation {
        /// The annotation.
        annotation: AttachmentKey,
        /// The node it annotates.
        #[serde(skip_serializing_if = "Option::is_none")]
        node: Option<NodeKey>,
    },
}

/// The address of a record, or of a set of records (a whole domain, graph, or node).
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RecordKey {
    /// Everything in a domain.
    Domain(Domain),
    /// Everything in a graph.
    Graph(GraphId),
    /// A record (or node) inside a graph.
    InGraph {
        /// The graph.
        graph: GraphId,
        /// The address inside it.
        key: GraphKey,
    },
    /// A journey's own fields.
    JourneyHeader(JourneyId),
    /// A route's own fields.
    RouteHeader(RouteId),
    /// A route's draft.
    RouteDraft(RouteId),
    /// A published route version.
    RouteVersion {
        /// The route.
        route: RouteId,
        /// The version.
        version: VersionNumber,
    },
    /// A deleted journey's id.
    DeletedJourney(JourneyId),
    /// An entity.
    Entity(EntityKey),
    /// An entity alias.
    EntityAlias(EntityKey),
    /// A proposal, within its destination domain.
    Proposal {
        /// The destination.
        destination: Domain,
        /// The proposal.
        id: ProposalId,
    },
}

impl GraphId {
    /// The domain a graph belongs to: its journey, or its route.
    #[must_use]
    pub fn domain(&self) -> Domain {
        match self {
            GraphId::Journey(journey) => Domain::Journey(journey.clone()),
            GraphId::RouteDraft(route) | GraphId::RouteVersion { route, .. } => {
                Domain::Route(route.clone())
            }
        }
    }
}

impl GraphRecord {
    /// The record's address inside its graph.
    #[must_use]
    pub fn key(&self) -> GraphKey {
        match self {
            GraphRecord::Node(node) => GraphKey::Node(node.key.clone()),
            GraphRecord::NodeField { node, value } => GraphKey::NodeField {
                node: node.clone(),
                field: value.field(),
            },
            GraphRecord::Edge(edge) => GraphKey::Edge(edge.clone()),
            GraphRecord::Role(role) => GraphKey::Role(role.key.clone()),
            GraphRecord::Kind(kind) => GraphKey::Kind(kind.key.clone()),
            GraphRecord::DefaultOwner(_) => GraphKey::DefaultOwner,
            GraphRecord::Participation { node, kind, .. } => GraphKey::Participation {
                node: node.clone(),
                kind: kind.clone(),
            },
            GraphRecord::Resource { node, resource } => GraphKey::Resource {
                node: node.clone(),
                resource: resource.key.clone(),
            },
            GraphRecord::RetiredKey(key) => GraphKey::RetiredKey(key.clone()),
            GraphRecord::NodeState { node, .. } => GraphKey::NodeState(node.clone()),
            GraphRecord::LocalEdit { node, edit } => GraphKey::LocalEdit {
                node: node.clone(),
                edit: edit.clone(),
            },
            GraphRecord::Answer { decision, .. } => GraphKey::Answer(decision.clone()),
            GraphRecord::RoleFill { role, .. } => GraphKey::RoleFill(role.clone()),
            GraphRecord::Pin { node, .. } => GraphKey::Pin(node.clone()),
            GraphRecord::Snooze { node, .. } => GraphKey::Snooze(node.clone()),
            GraphRecord::Overrides { node, .. } => GraphKey::Overrides(node.clone()),
            GraphRecord::Tombstone(node) => GraphKey::Tombstone(node.clone()),
            GraphRecord::Annotation(annotation) => GraphKey::Annotation {
                annotation: annotation.body.key.clone(),
                node: annotation.body.node.clone(),
            },
        }
    }
}

impl GraphKey {
    /// The node the addressed records hang off, if any: what a node's removal reaches. An
    /// edge hangs off its dependent; [`GraphKey::overlaps`] also gives it to its
    /// requirement.
    #[must_use]
    pub fn node_scope(&self) -> Option<&NodeKey> {
        match self {
            GraphKey::Node(node)
            | GraphKey::NodeField { node, .. }
            | GraphKey::Participation { node, .. }
            | GraphKey::Resource { node, .. }
            | GraphKey::NodeState(node)
            | GraphKey::LocalEdit { node, .. }
            | GraphKey::Answer(node)
            | GraphKey::Pin(node)
            | GraphKey::Snooze(node)
            | GraphKey::Overrides(node)
            | GraphKey::Tombstone(node)
            | GraphKey::Annotation {
                node: Some(node), ..
            }
            | GraphKey::Edge(Edge { node, .. }) => Some(node),
            GraphKey::Role(_)
            | GraphKey::Kind(_)
            | GraphKey::DefaultOwner
            | GraphKey::RetiredKey(_)
            | GraphKey::RoleFill(_)
            | GraphKey::Annotation { node: None, .. } => None,
        }
    }

    /// Whether two addresses in one graph can name a common record: equal, the same
    /// annotation however addressed, or a whole node and something on it (an edge is on
    /// both its ends).
    #[must_use]
    pub fn overlaps(&self, other: &GraphKey) -> bool {
        let same_annotation = matches!(
            (self, other),
            (GraphKey::Annotation { annotation: first, .. }, GraphKey::Annotation { annotation: second, .. }) if first == second
        );
        self == other || same_annotation || self.contains(other) || other.contains(self)
    }

    fn contains(&self, other: &GraphKey) -> bool {
        let GraphKey::Node(node) = self else {
            return false;
        };
        let requirement_end = matches!(other, GraphKey::Edge(edge) if edge.requires == *node);
        requirement_end || other.node_scope() == Some(node)
    }
}

impl Record {
    /// The record's address.
    #[must_use]
    pub fn key(&self) -> RecordKey {
        match self {
            Record::Graph { graph, record } => RecordKey::InGraph {
                graph: graph.clone(),
                key: record.key(),
            },
            Record::JourneyHeader(header) => RecordKey::JourneyHeader(header.id.clone()),
            Record::RouteHeader(header) => RecordKey::RouteHeader(header.id.clone()),
            Record::RouteDraft { route, .. } => RecordKey::RouteDraft(route.clone()),
            Record::RouteVersion { route, version, .. } => RecordKey::RouteVersion {
                route: route.clone(),
                version: *version,
            },
            Record::DeletedJourney { journey, .. } => RecordKey::DeletedJourney(journey.clone()),
            Record::Entity(entity) => RecordKey::Entity(entity.key.clone()),
            Record::EntityAlias { alias, .. } => RecordKey::EntityAlias(alias.clone()),
            Record::Proposal(proposal) => RecordKey::Proposal {
                destination: proposal.destination.clone(),
                id: proposal.id.clone(),
            },
        }
    }
}

impl RecordKey {
    /// The domain the addressed records belong to.
    #[must_use]
    pub fn domain(&self) -> Domain {
        match self {
            RecordKey::Domain(domain)
            | RecordKey::Proposal {
                destination: domain,
                ..
            } => domain.clone(),
            RecordKey::Graph(graph) | RecordKey::InGraph { graph, .. } => graph.domain(),
            RecordKey::JourneyHeader(journey) => Domain::Journey(journey.clone()),
            RecordKey::RouteHeader(route)
            | RecordKey::RouteDraft(route)
            | RecordKey::RouteVersion { route, .. } => Domain::Route(route.clone()),
            RecordKey::DeletedJourney(_) | RecordKey::Entity(_) | RecordKey::EntityAlias(_) => {
                Domain::Deployment
            }
        }
    }

    /// Whether two addresses can name a common record: equal, one a whole domain or graph
    /// holding the other, or overlapping inside one graph ([`GraphKey::overlaps`]).
    #[must_use]
    pub fn overlaps(&self, other: &RecordKey) -> bool {
        match (self, other) {
            (RecordKey::Domain(domain), key) | (key, RecordKey::Domain(domain)) => {
                key.domain() == *domain
            }
            (
                RecordKey::Graph(graph),
                RecordKey::Graph(other_graph)
                | RecordKey::InGraph {
                    graph: other_graph, ..
                },
            )
            | (
                RecordKey::InGraph {
                    graph: other_graph, ..
                },
                RecordKey::Graph(graph),
            ) => graph == other_graph,
            (
                RecordKey::InGraph { graph, key },
                RecordKey::InGraph {
                    graph: other_graph,
                    key: other_key,
                },
            ) => graph == other_graph && key.overlaps(other_key),
            _ => self == other,
        }
    }
}

/// One change to the stored records: a record's after-state, a removal, or a whole-graph
/// copy (publishing copies a draft into a version; a removal of a whole graph clears it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum Write {
    /// Store this record, replacing any at its address.
    Put(Record),
    /// Remove the record at this address, or every record under a domain, graph, or node.
    Remove(RecordKey),
    /// Copy every record of one graph into another.
    CopyGraph {
        /// The graph copied.
        from: GraphId,
        /// The graph written.
        to: GraphId,
    },
}

impl Write {
    /// The addresses this write changes, and for a copy, reads.
    #[must_use]
    pub fn keys(&self) -> Vec<RecordKey> {
        match self {
            Write::Put(record) => vec![record.key()],
            Write::Remove(key) => vec![key.clone()],
            Write::CopyGraph { from, to } => {
                vec![RecordKey::Graph(from.clone()), RecordKey::Graph(to.clone())]
            }
        }
    }
}
