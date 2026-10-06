//! The touched set (PRD glossary; H5): everything a patch or event writes or removes, by key
//! and field, including a removed node's subtree and every incident edge. Two changes are
//! safe to reorder when their touched sets do not overlap, which is what lets a client
//! retry a stale patch on its own.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::{Domain, GraphId};
use crate::id::NodeKey;
use crate::patch::Removal;
use crate::patch::{Mutation, Patch, PatchTarget};
use crate::record::{GraphKey, RecordKey, RetiredKey};
use crate::state::LocalEdit;

/// A set of record addresses.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct TouchedSet(BTreeSet<RecordKey>);

impl TouchedSet {
    /// The addresses, sorted.
    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<RecordKey> {
        &self.0
    }

    /// Adds an address.
    pub fn insert(&mut self, key: RecordKey) {
        self.0.insert(key);
    }

    /// Adds every address of another set.
    pub fn extend(&mut self, other: TouchedSet) {
        self.0.extend(other.0);
    }

    /// H5: whether any address here can name a record any address there names. Disjoint
    /// sets are safe to reorder. Equivalent to asking [`RecordKey::overlaps`] of every pair,
    /// but by lookup: O((n + m) log n) at the mutation limit rather than n x m.
    #[must_use]
    pub fn overlaps(&self, other: &TouchedSet) -> bool {
        self.reaches(other) || other.reaches(self)
    }

    /// Whether some address of `other` is one of these, or lies under one of these that
    /// covers a whole domain, graph, node, or annotation.
    fn reaches(&self, other: &TouchedSet) -> bool {
        let covers = Covers::of(self);
        other
            .0
            .iter()
            .any(|key| self.0.contains(key) || covers.contains(key))
    }

    /// True when nothing is touched.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The wider addresses in a touched set, indexed: whole domains and graphs, whole nodes,
/// and annotations by key (an annotation is one record however its address names its node).
#[derive(Default)]
struct Covers<'a> {
    domains: BTreeSet<Domain>,
    graphs: BTreeSet<&'a GraphId>,
    nodes: BTreeSet<(&'a GraphId, &'a NodeKey)>,
    annotations: BTreeSet<(&'a GraphId, &'a crate::id::AttachmentKey)>,
}

impl<'a> Covers<'a> {
    fn of(set: &'a TouchedSet) -> Self {
        let mut covers = Covers::default();
        for key in &set.0 {
            match key {
                RecordKey::Domain(domain) => {
                    covers.domains.insert(domain.clone());
                }
                RecordKey::Graph(graph) => {
                    covers.graphs.insert(graph);
                }
                RecordKey::InGraph {
                    graph,
                    key: GraphKey::Node(node),
                } => {
                    covers.nodes.insert((graph, node));
                }
                RecordKey::InGraph {
                    graph,
                    key: GraphKey::Annotation { annotation, .. },
                } => {
                    covers.annotations.insert((graph, annotation));
                }
                _ => {}
            }
        }
        covers
    }

    /// Whether `key` lies under one of the covering addresses (by [`RecordKey::overlaps`]'s
    /// rules: a domain holds everything in it, a graph everything in it, a node everything
    /// on it and both ends of its edges).
    fn contains(&self, key: &RecordKey) -> bool {
        if self.domains.contains(&key.domain()) {
            return true;
        }
        let RecordKey::InGraph { graph, key } = key else {
            return matches!(key, RecordKey::Graph(graph) if self.graphs.contains(graph));
        };
        let on_node = |node: &NodeKey| self.nodes.contains(&(graph, node));
        self.graphs.contains(graph)
            || key.node_scope().is_some_and(on_node)
            || matches!(key, GraphKey::Edge(edge) if on_node(&edge.requires))
            || matches!(key, GraphKey::Annotation { annotation, .. } if self.annotations.contains(&(graph, annotation)))
    }
}

impl FromIterator<RecordKey> for TouchedSet {
    fn from_iter<I: IntoIterator<Item = RecordKey>>(keys: I) -> Self {
        Self(keys.into_iter().collect())
    }
}

/// The graph a mutation in a patch to `target` changes: a journey's own, or a route's draft.
fn target_graph(target: &PatchTarget) -> Option<GraphId> {
    match target {
        PatchTarget::Journey(journey) => Some(GraphId::Journey(journey.clone())),
        PatchTarget::Route(route) => Some(GraphId::RouteDraft(route.clone())),
        PatchTarget::Deployment | PatchTarget::Proposal { .. } => None,
    }
}

impl Patch {
    /// H5: the patch's own touched set, the union of its mutations'.
    #[must_use]
    pub fn touched(&self) -> TouchedSet {
        let mut touched = TouchedSet::default();
        for mutation in self.mutations.as_slice() {
            touched.extend(mutation.touched(&self.target));
        }
        touched
    }
}

impl Mutation {
    /// H5: what this mutation writes or removes in a patch to `target`, known from the
    /// mutation alone (A18 makes a removal name everything it removes). A mutation whose
    /// effect depends on stored content it does not name (creating, deleting, publishing,
    /// upgrading, applying a proposal) touches its whole domain, and so does any mutation
    /// not listed here: a coarse touch is always safe, since it only stops an automatic
    /// retry.
    #[must_use]
    pub fn touched(&self, target: &PatchTarget) -> TouchedSet {
        if let PatchTarget::Proposal { id, destination } = target {
            let proposal = RecordKey::Proposal {
                destination: destination.clone(),
                id: id.clone(),
            };
            return std::iter::once(proposal).collect();
        }
        let graph_keys = target_graph(target).and_then(|graph| {
            let keys = self.structure_keys().or_else(|| self.state_keys())?;
            Some(
                keys.into_iter()
                    .map(|key| RecordKey::InGraph {
                        graph: graph.clone(),
                        key,
                    })
                    .collect(),
            )
        });
        graph_keys.unwrap_or_else(|| self.domain_keys(target.domain()))
    }

    /// Records outside the graph: headers, entities, proposals, or the whole domain.
    fn domain_keys(&self, domain: Domain) -> TouchedSet {
        let keys = match (self, &domain) {
            (
                Mutation::EditJourney { .. } | Mutation::SetJourneyStatus { .. },
                Domain::Journey(journey),
            ) => {
                vec![RecordKey::JourneyHeader(journey.clone())]
            }
            (
                Mutation::EditRoute { .. } | Mutation::SetRouteRetired { .. },
                Domain::Route(route),
            ) => {
                vec![RecordKey::RouteHeader(route.clone())]
            }
            (Mutation::CreateEntity { entity } | Mutation::EditEntity { entity }, _) => {
                vec![RecordKey::Entity(entity.key.clone())]
            }
            (
                Mutation::MergeEntities {
                    survivor, merged, ..
                },
                _,
            ) => vec![
                RecordKey::Entity(survivor.clone()),
                RecordKey::Entity(merged.clone()),
                RecordKey::EntityAlias(merged.clone()),
            ],
            (Mutation::ApplyProposal { proposal, .. }, _) => vec![
                RecordKey::Domain(domain.clone()),
                RecordKey::Proposal {
                    destination: domain.clone(),
                    id: proposal.clone(),
                },
            ],
            _ => vec![RecordKey::Domain(domain)],
        };
        keys.into_iter().collect()
    }

    /// Graph structure: the node, edge, role, kind, participation, or resource changed,
    /// with the local-edit marker a journey's edit sets beside it (B4).
    fn structure_keys(&self) -> Option<Vec<GraphKey>> {
        let marker = |node: &NodeKey, edit: LocalEdit| GraphKey::LocalEdit {
            node: node.clone(),
            edit,
        };
        Some(match self {
            Mutation::AddNode { node } | Mutation::ReplaceNode { node } => {
                vec![GraphKey::Node(node.key.clone())]
            }
            Mutation::SetNodeField { node, value } => vec![
                GraphKey::NodeField {
                    node: node.clone(),
                    field: value.field(),
                },
                marker(node, LocalEdit::Field(value.field())),
            ],
            Mutation::RemoveNode { removal } => removal_keys(removal),
            Mutation::AddEdge { edge } | Mutation::RemoveEdge { edge } => {
                vec![
                    GraphKey::Edge(edge.clone()),
                    marker(&edge.node, LocalEdit::Requires(edge.requires.clone())),
                ]
            }
            Mutation::AddRole { role } | Mutation::EditRole { role } => {
                vec![GraphKey::Role(role.key.clone())]
            }
            Mutation::RemoveRole { role } => {
                vec![
                    GraphKey::Role(role.clone()),
                    GraphKey::RetiredKey(RetiredKey::Role(role.clone())),
                ]
            }
            Mutation::AddParticipationKind { kind } | Mutation::EditParticipationKind { kind } => {
                vec![GraphKey::Kind(kind.key.clone())]
            }
            Mutation::RemoveParticipationKind { kind } => {
                vec![
                    GraphKey::Kind(kind.clone()),
                    GraphKey::RetiredKey(RetiredKey::Kind(kind.clone())),
                ]
            }
            Mutation::SetDefaultOwner { .. } => vec![GraphKey::DefaultOwner],
            Mutation::SetParticipation { node, kind, .. }
            | Mutation::ClearParticipation { node, kind } => vec![
                GraphKey::Participation {
                    node: node.clone(),
                    kind: kind.clone(),
                },
                marker(node, LocalEdit::Participation(kind.clone())),
            ],
            Mutation::AddResource { node, resource }
            | Mutation::EditResource { node, resource } => vec![
                GraphKey::Resource {
                    node: node.clone(),
                    resource: resource.key.clone(),
                },
                marker(node, LocalEdit::Resource(resource.key.clone())),
            ],
            Mutation::RemoveResource { node, resource } => vec![
                GraphKey::Resource {
                    node: node.clone(),
                    resource: resource.clone(),
                },
                marker(node, LocalEdit::Resource(resource.clone())),
            ],
            _ => return None,
        })
    }

    /// Journey state: one record per node and kind of state.
    fn state_keys(&self) -> Option<Vec<GraphKey>> {
        Some(match self {
            // Any transition clears the node's snooze (B6); reopening a decision clears its
            // answer (D1); answering is a transition too.
            Mutation::Transition { node, .. } | Mutation::Answer { decision: node, .. } => vec![
                GraphKey::NodeState(node.clone()),
                GraphKey::Snooze(node.clone()),
                GraphKey::Answer(node.clone()),
            ],
            Mutation::SetRecordedDate { node, .. }
            | Mutation::SetAtomic { node, .. }
            | Mutation::SetProvenance { node, .. } => {
                vec![GraphKey::NodeState(node.clone())]
            }
            Mutation::FillRole { role, .. } | Mutation::ClearRoleFill { role } => {
                vec![GraphKey::RoleFill(role.clone())]
            }
            Mutation::SetPin { node, .. }
            | Mutation::ShiftPin { node, .. }
            | Mutation::ClearPin { node } => {
                vec![GraphKey::Pin(node.clone())]
            }
            Mutation::Snooze { node, .. } | Mutation::Unsnooze { node } => {
                vec![GraphKey::Snooze(node.clone())]
            }
            Mutation::ApplyOverride { node, .. } | Mutation::RemoveOverride { node, .. } => {
                vec![GraphKey::Overrides(node.clone())]
            }
            Mutation::AddAnnotation { annotation } | Mutation::EditAnnotation { annotation } => {
                vec![GraphKey::Annotation {
                    annotation: annotation.key.clone(),
                    node: annotation.node.clone(),
                }]
            }
            Mutation::RemoveAnnotation { annotation } => vec![GraphKey::Annotation {
                annotation: annotation.clone(),
                node: None,
            }],
            Mutation::MarkLocalEdit { node, edit, .. } => vec![GraphKey::LocalEdit {
                node: node.clone(),
                edit: edit.clone(),
            }],
            _ => return None,
        })
    }
}

/// A18: a removal touches its nodes (and everything on them), retires their keys, and names
/// its edges and annotations, which may hang off nodes outside the subtree.
fn removal_keys(removal: &Removal) -> Vec<GraphKey> {
    let mut keys: Vec<GraphKey> = removal
        .nodes()
        .map(|node| GraphKey::Node(node.clone()))
        .collect();
    keys.extend(
        removal
            .nodes()
            .map(|node| GraphKey::RetiredKey(RetiredKey::Node(node.clone()))),
    );
    keys.extend(removal.edges.iter().cloned().map(GraphKey::Edge));
    // A node that stays and loses a route-copied edge to the subtree is marked (B4).
    let removed: Vec<&NodeKey> = removal.nodes().collect();
    keys.extend(
        removal
            .edges
            .iter()
            .filter(|edge| !removed.contains(&&edge.node))
            .map(|edge| GraphKey::LocalEdit {
                node: edge.node.clone(),
                edit: LocalEdit::Requires(edge.requires.clone()),
            }),
    );
    keys.extend(
        removal
            .annotations
            .iter()
            .map(|annotation| GraphKey::Annotation {
                annotation: annotation.clone(),
                node: None,
            }),
    );
    keys
}
