//! The touched set (PRD glossary; H5): everything a patch or event writes or removes, by key
//! and field, including a removed node's subtree and every incident edge. Two changes are
//! safe to reorder when their touched sets do not overlap, which is what lets the service
//! apply a stale patch on its own.
//!
//! A touched set is only ever read off writes ([`crate::Write::keys`]): an event's delta, or
//! the change set the engine's apply builds. What a patch touches depends on what it finds
//! (removing a role clears its fill, reopening a node its bypass), so nothing predicts it
//! from the mutations alone; the service reads the patch's own off applying it at the
//! current revisions
//! (decisions/2026-10-21-a-patchs-touched-set-comes-from-what-apply-writes.md).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::{Domain, GraphId};
use crate::id::NodeKey;
use crate::record::{GraphKey, RecordKey};

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

    /// Everything in `domain`: what a change touches when what it writes is not known, which
    /// overlaps anything written there, so nothing beside it is retried on its own.
    #[must_use]
    pub fn whole(domain: Domain) -> Self {
        std::iter::once(RecordKey::Domain(domain)).collect()
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
