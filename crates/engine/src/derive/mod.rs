//! Derive (ARCHITECTURE, Read path: derive; D3, D6): the read path, computing every derived
//! value of a journey from its graph, its state, and the derive inputs, on every read and
//! never stored. Passes run in dependency order, each reading only earlier passes' output:
//!
//! 1. Relevance ([`relevance`]): three-valued, with the condition or override that produced
//!    each value.
//! 2. Effective dependencies ([`dependencies`]): the full and pruned dependency graphs over
//!    start, finish, and entry instants, every implicit edge tagged with its source; then
//!    effective skip and kept work ([`skip`]).
//! 3. Participation ([`participation`]): E2's resolution per node and kind, `unassigned`,
//!    and the membership-loss flag.
//!
//! [`Derived`] holds the output of the passes that exist; later passes add their fields. It
//! is the engine's own type: the schema's `Derived`, which crosses the API, is projected
//! from it once every field it carries is derived (DECISIONS.md).

mod condition;
pub mod dependencies;
pub mod participation;
pub mod relevance;
pub mod skip;

use cairn_schema::{DeriveInputs, KeyRefs, Node, NodeKey, State};

use crate::graph::{Document, Graph};
pub use dependencies::{Dependencies, EdgeSet, EffectiveDependency};
pub use participation::Participation;
pub use relevance::{NodeRelevance, Producer, Relevances};
pub use skip::Skips;

/// A journey's derived values (D3): the output of every pass, with the inputs that explain
/// each value. Never stored (D6, J1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derived {
    relevance: Relevances,
    dependencies: Dependencies,
    skips: Skips,
    participation: Participation,
}

impl Derived {
    /// Pass 1: relevance.
    #[must_use]
    pub fn relevance(&self) -> &Relevances {
        &self.relevance
    }

    /// Pass 2: the effective dependency graph, full and pruned.
    #[must_use]
    pub fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    /// Pass 2: effective skip and kept work.
    #[must_use]
    pub fn skips(&self) -> &Skips {
        &self.skips
    }

    /// Pass 3: participation.
    #[must_use]
    pub fn participation(&self) -> &Participation {
        &self.participation
    }
}

/// Derives a journey (ARCHITECTURE, Read path: derive): a pure function of the graph, its
/// state, and the inputs (D6). A route version or draft derives as a journey just created
/// from it would, every node in its kind's initial state.
///
/// # Panics
///
/// When the graph holds state for some nodes and not others, which validation rejects.
#[must_use]
pub fn derive(journey: &Graph, inputs: &DeriveInputs) -> Derived {
    let document = journey.document();
    let state = &document.state.nodes;
    assert!(
        state.is_empty() || state.len() == document.nodes.len(),
        "every node of a journey has a stored state"
    );
    let inherited = skip::inherited(journey);
    let relevance = relevance::pass(journey, &inputs.deployment, &inherited);
    let mut dependencies = Dependencies::build(document, journey.tree());
    dependencies.prune(document, &relevance);
    assert_eq!(dependencies.node_count(), document.nodes.len());
    let skips = skip::pass(journey, &relevance, &inherited);
    assert!(skips.effective().all(|(key, _)| relevance.in_scope(key)));
    let participation = participation::pass(journey, &relevance, &inputs.deployment);
    Derived {
        relevance,
        dependencies,
        skips,
        participation,
    }
}

/// A node's stored state, or its kind's initial state where nothing is stored (a route).
#[must_use]
pub(crate) fn stored_state(document: &Document, node: &Node<KeyRefs>) -> State {
    document
        .state
        .nodes
        .get(&node.key)
        .map_or(State::initial(node.kind()), |stored| stored.state)
}

/// Gating: the node is force-included.
#[must_use]
pub(crate) fn forced(document: &Document, key: &NodeKey) -> bool {
    document
        .state
        .overrides
        .get(key)
        .is_some_and(|overrides| overrides.force_include.is_some())
}

/// D1a: the node is kept under a skipped ancestor.
#[must_use]
pub(crate) fn kept(document: &Document, key: &NodeKey) -> bool {
    document
        .state
        .overrides
        .get(key)
        .is_some_and(|overrides| overrides.keep.is_some())
}
