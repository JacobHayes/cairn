//! Projections (ARCHITECTURE, Engine > Projections; I1, C2 risk mitigation): every read view
//! the PRD names, as pure functions of a derived journey, so the browser (which derives
//! locally) and the server (for agents and the API) produce the same values. Inputs and
//! outputs are schema types, so a projection serializes to the API and the wasm host
//! unchanged. Nothing here is stored (D3, D6).
//!
//! A [`DerivedJourney`] pairs a journey's graph with its [`Derived`]; each projection is a
//! method on it. [`document`] builds the domain document the browser derives from, and
//! [`history`] pages a journey's events. Each
//! module states its projection's cost at `node_count_max`.

mod draft;
mod explain;
mod level;
mod next;
mod rows;
mod snapshot;
mod trace;
mod views;

pub use draft::DraftContext;
pub use explain::history;

use std::collections::BTreeSet;
use std::fmt;

use cairn_schema::limits::NODE_COUNT_MAX;
use cairn_schema::{
    AnswerValue, DeriveInputs, DomainDocument, EngineVersion, EntitySet, Journey, KeyRefs, Node,
    NodeKey, State,
};

use crate::derive::{Derived, stored_state};
use crate::graph::Graph;

/// A journey's graph with its derived values: what every projection reads.
#[derive(Clone, Copy, Debug)]
pub struct DerivedJourney<'a> {
    graph: &'a Graph,
    derived: &'a Derived,
}

/// A projection's input names something the journey does not hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// No node has this key.
    UnknownNode(NodeKey),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectionError::UnknownNode(key) => write!(formatter, "no node {key}"),
        }
    }
}

impl std::error::Error for ProjectionError {}

impl<'a> DerivedJourney<'a> {
    /// The journey `graph` with `derived`, its derive.
    ///
    /// # Panics
    ///
    /// When `derived` is not a derive of `graph`.
    #[must_use]
    pub fn new(graph: &'a Graph, derived: &'a Derived) -> Self {
        assert_eq!(
            graph.document().nodes.len(),
            derived.dependencies().node_count(),
            "the graph derived"
        );
        Self { graph, derived }
    }

    /// The graph.
    #[must_use]
    pub fn graph(&self) -> &'a Graph {
        self.graph
    }

    /// The derived values.
    #[must_use]
    pub fn derived(&self) -> &'a Derived {
        self.derived
    }

    /// The node, or an error naming the key.
    fn known(&self, key: &NodeKey) -> Result<&'a Node<KeyRefs>, ProjectionError> {
        self.graph
            .node(key)
            .ok_or_else(|| ProjectionError::UnknownNode(key.clone()))
    }

    /// The node at a key every caller has read off the graph.
    fn node(&self, key: &NodeKey) -> &'a Node<KeyRefs> {
        self.graph
            .node(key)
            .unwrap_or_else(|| panic!("the graph holds {key}"))
    }

    /// The node's stored state, or its kind's initial state where none is stored (a route).
    fn state(&self, key: &NodeKey) -> State {
        stored_state(self.graph.document(), self.node(key))
    }

    /// The nodes beneath `container` (every node when none), in tree order: each node before
    /// its children, siblings in key order. An explicit stack (PRACTICES, No recursion).
    fn tree_order(&self, container: Option<&NodeKey>) -> Vec<&'a NodeKey> {
        let tree = self.graph.tree();
        let tops: &[NodeKey] = match container {
            Some(key) => tree.children(key),
            None => tree.roots(),
        };
        let mut order = Vec::new();
        let mut stack: Vec<&NodeKey> = tops.iter().rev().collect();
        while let Some(key) = stack.pop() {
            order.push(key);
            stack.extend(tree.children(key).iter().rev());
        }
        assert!(order.len() <= NODE_COUNT_MAX as usize, "each node once");
        order
    }

    /// The node's ancestors, root first.
    fn ancestors(&self, key: &NodeKey) -> Vec<NodeKey> {
        let tree = self.graph.tree();
        let mut ancestors: Vec<NodeKey> = Vec::new();
        let mut current = tree.parent(key);
        while let Some(parent) = current {
            ancestors.push(parent.clone());
            current = tree.parent(parent);
        }
        ancestors.reverse();
        ancestors
    }

    /// E3, E6: the decision's answer while in effect, its entities read through aliases so
    /// an answer given before a merge names the survivor.
    fn answer(&self, decision: &NodeKey) -> Option<AnswerValue> {
        let answer = self
            .derived
            .relevance()
            .answer_in_effect(self.graph.document(), decision)?;
        let participation = self.derived.participation();
        Some(match answer {
            AnswerValue::Entity(entity) => {
                AnswerValue::Entity(participation.canonical_entity(entity).clone())
            }
            AnswerValue::EntityList(entities) => {
                let resolved = entities
                    .iter()
                    .map(|entity| participation.canonical_entity(entity).clone());
                // Resolving only merges keys, so the list stays within its limit.
                AnswerValue::EntityList(
                    EntitySet::new(resolved).unwrap_or_else(|error| panic!("{error}")),
                )
            }
            other => other.clone(),
        })
    }

    /// The node's owners (E2).
    fn owners(&self, key: &NodeKey) -> &'a BTreeSet<cairn_schema::EntityKey> {
        self.derived
            .participation()
            .entities(key, &cairn_schema::KindKey::owner())
    }
}

/// The domain document (ARCHITECTURE, Terms): the journey's graph and state with the derive
/// inputs (the deployment context included) and the engine version, which the browser
/// derives and projects locally. Nothing derived is in it.
///
/// # Panics
///
/// Never: the crate's version is a valid engine version.
#[must_use]
pub fn document(journey: &Journey, inputs: &DeriveInputs) -> DomainDocument {
    let engine_version: EngineVersion = env!("CARGO_PKG_VERSION")
        .parse()
        .unwrap_or_else(|error| panic!("the crate version: {error}"));
    DomainDocument {
        journey: journey.clone(),
        inputs: inputs.clone(),
        engine_version,
    }
}
