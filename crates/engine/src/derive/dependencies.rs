//! Pass 2, the effective dependency graph (PRD Containment, Gating; ARCHITECTURE, Read path:
//! derive, pass 2): explicit edges plus the implicit gates (containment, inherited
//! requirements, condition gates, stage openings), each edge tagged with its source and typed
//! as a gate or date-only, in a full structural set and a relevance-pruned set.
//!
//! Instants: every node has a start and a finish (a decision or milestone has one instant,
//! so its finish waits on its start at no distance). A container also has two entries: the
//! requirement entry, when its work may begin as far as requirements go, and the condition
//! entry, the same for conditions. A requirement on a node (explicit, or a stage's opening)
//! waits at its requirement anchor: the requirement entry for a container, the start
//! otherwise; a condition gate waits at the condition anchor (the condition entry, or the
//! start). Dependents wait for a node's finish, so reaching a container's entry or start
//! never completes it. Containment is a child's anchors waiting on its parent's entries,
//! the parent's start on each child's finish, and the parent's finish on its start, so an
//! ancestor's requirement reaches every descendant through the chain of entries and is never
//! copied. Conditions travel their own chain, which stops at a force-included node: force
//! include drops its ancestors' and its own conditions but keeps their requirements.
//!
//! Sets: the full set is structural, the same for any relevance and any override, and is
//! what validation's cycle check and tracing read. The pruned set is what execution reads:
//! it leaves out every edge that waits on a not-relevant node or holds one's own work, and the
//! condition gates force include drops. A not-relevant container's requirement entry stays,
//! so a force-included node beneath it still waits on the container's requirements.
//! Undecided nodes stay in both. Gate edges block (D1); a stage opening with `gates: false`
//! is date-only: dates read it, blocking and the acyclicity invariant do not.
//!
//! Consuming it (2.3, 2.4): [`Dependencies::waits`] and [`Dependencies::waited_by`] give an
//! instant's edges in a set, for whole-graph passes in topological order;
//! [`Dependencies::of`] lists one node's effective dependencies with the source of each,
//! walking the entry chains on demand rather than materializing inherited edges per node.
//!
//! Cost at `node_count_max` (2,000 nodes, 64 explicit edges each in plus out, 16 condition
//! clauses each): 8,000 instants; edges are at most 6 per node for work, entries, and
//! containment, plus one per explicit edge (64,000 in all), one per condition decision
//! (32,000), and one per stage opening, about 110,000 edges of 16 bytes and a pruned flag
//! each, under 2 MiB. Building is O(nodes + edges) with one key lookup per reference
//! (O(log nodes)); sorting the edges into per-instant slices is O(edges log edges); pruning
//! is O(edges). Listing one node's dependencies walks at most 2 x 16 entry instants and
//! their edges, at most about 1,300 edges plus the node's children.

mod cycles;
mod listing;

use std::collections::BTreeMap;

use cairn_schema::limits::NODE_COUNT_MAX;
use cairn_schema::{DependencyVia, KeyRefs, Node, NodeKey, NodeKind, Payload, Relevance};

use super::forced;
use super::relevance::Relevances;
use crate::graph::{Document, Tree};

/// An instant of a node (ARCHITECTURE, Read path: derive).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Point {
    /// When the node's own work starts; a decision's or milestone's one instant.
    Start,
    /// When the node finishes; what dependents wait for.
    Finish,
    /// A container's requirement entry: requirements on it and its ancestors wait here.
    Entry,
    /// A container's condition entry: condition gates on it and its ancestors wait here.
    ConditionEntry,
}

impl Point {
    const ALL: [Point; 4] = [
        Point::Start,
        Point::Finish,
        Point::Entry,
        Point::ConditionEntry,
    ];

    fn offset(self) -> usize {
        match self {
            Point::Start => 0,
            Point::Finish => 1,
            Point::Entry => 2,
            Point::ConditionEntry => 3,
        }
    }
}

/// A node's position in the graph's key order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIndex(u32);

impl NodeIndex {
    /// The position, for indexing the caller's own per-node tables.
    #[must_use]
    pub fn get(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }

    /// The index at a position in key order.
    pub(crate) fn from_position(at: usize) -> Self {
        Self(u32::try_from(at).unwrap_or(u32::MAX))
    }
}

/// One instant: a node and a point of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    /// The node.
    pub node: NodeIndex,
    /// Which instant of it.
    pub point: Point,
}

impl Instant {
    /// The instant `point` of `node`.
    #[must_use]
    pub fn new(node: NodeIndex, point: Point) -> Self {
        Self { node, point }
    }

    /// The instant's position among all instants, for the caller's per-instant tables.
    #[must_use]
    pub fn slot(self) -> usize {
        self.node.get() * Point::ALL.len() + self.point.offset()
    }
}

/// Whether an edge blocks (ARCHITECTURE, Read path: every edge is typed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeClass {
    /// Blocks (D1): blocking, the acyclicity invariant, trace, and gravity read it.
    Gate,
    /// Holds dates only: a stage opening with `gates: false` (F4).
    DateOnly,
}

/// Why an edge exists. The dependent instant's node is the one the source belongs to: the
/// node whose `requires`, condition, or opening it is, or the container whose entry it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeSource {
    /// A node's finish waits for its own start.
    Work,
    /// A container's start waits for its entries: its own work follows its children's.
    Entry,
    /// A node's anchor waits for its parent's entry of the same kind: the link that carries
    /// ancestors' requirements (requirement chain) and conditions (condition chain) down.
    Chain,
    /// A parent's start waits for a child's finish: the parent implicitly requires it.
    Containment,
    /// An explicit `requires` edge (A3).
    Explicit,
    /// A condition gate: the node's condition reads the decision (Gating).
    Condition,
    /// A stage's opening milestone (F4).
    StageOpening,
}

/// One edge: `dependent` waits for `requirement`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    /// The waiting instant.
    pub dependent: Instant,
    /// The instant it waits for.
    pub requirement: Instant,
    /// Gate or date-only.
    pub class: EdgeClass,
    /// Why it exists.
    pub source: EdgeSource,
}

/// Which edge set to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeSet {
    /// Every structural edge, whatever relevance and overrides say.
    Full,
    /// What execution reads: not-relevant work and force-dropped condition gates left out.
    Pruned,
}

/// One effective dependency of a node, with how it arose (PRD Containment: each names its
/// source).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EffectiveDependency {
    /// The node depended on.
    pub node: NodeKey,
    /// How the dependency arose.
    pub via: DependencyVia,
    /// Gate or date-only.
    pub class: EdgeClass,
}

/// The effective dependency graph of one journey (or route).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dependencies {
    keys: Vec<NodeKey>,
    index: BTreeMap<NodeKey, NodeIndex>,
    containers: Vec<bool>,
    /// Sorted by dependent instant; `waits[slot]..waits[slot + 1]` are an instant's edges.
    edges: Vec<Edge>,
    waits: Vec<usize>,
    /// Edge positions sorted by requirement instant, with per-instant offsets.
    dependents: Vec<usize>,
    dependents_at: Vec<usize>,
    /// Per edge: in the pruned set. Until pruned, every edge is.
    pruned: Vec<bool>,
}

impl Dependencies {
    /// The full structural graph of a document, tolerating a candidate that breaks other
    /// invariants: references that do not resolve, or that other stages report (an edge to
    /// self, an ancestor, or a descendant; a condition on the node's own subtree or on a
    /// non-decision; a stage bound inside the stage or not a milestone), are left out, and
    /// only children a root reaches are linked to their parent, so each cause is reported
    /// once, by its own stage.
    #[must_use]
    pub(crate) fn build(document: &Document, tree: &Tree) -> Self {
        assert!(document.nodes.len() <= NODE_COUNT_MAX as usize);
        let keys: Vec<NodeKey> = document.nodes.as_map().keys().cloned().collect();
        let index: BTreeMap<NodeKey, NodeIndex> = keys
            .iter()
            .enumerate()
            .map(|(i, key)| (key.clone(), NodeIndex(u32::try_from(i).unwrap_or(u32::MAX))))
            .collect();
        let containers = keys.iter().map(|key| tree.is_container(key)).collect();
        let mut graph = Dependencies {
            keys,
            index,
            containers,
            ..Dependencies::default()
        };
        let mut edges = Vec::new();
        for node in document.nodes.values() {
            graph.node_edges(document, tree, node, &mut edges);
        }
        graph.settle(edges);
        graph
    }

    /// The edges a node's own structure adds: its work, its entries, its links to its
    /// parent, and its requirements, opening, and condition gates.
    fn node_edges(
        &self,
        document: &Document,
        tree: &Tree,
        node: &Node<KeyRefs>,
        out: &mut Vec<Edge>,
    ) {
        let Some(at) = self.index.get(&node.key).copied() else {
            return;
        };
        let instant = Instant::new;
        out.push(gate(
            instant(at, Point::Finish),
            instant(at, Point::Start),
            EdgeSource::Work,
        ));
        if self.is_container(at) {
            for entry in [Point::Entry, Point::ConditionEntry] {
                out.push(gate(
                    instant(at, Point::Start),
                    instant(at, entry),
                    EdgeSource::Entry,
                ));
            }
        }
        let parent = node
            .parent
            .as_ref()
            .and_then(|parent| self.index.get(parent));
        // Only a node a root reaches is linked: a parent cycle is reported as containment.
        if let (Some(&parent), Some(_)) = (parent, tree.path(&node.key)) {
            let links = [
                (
                    self.anchor(at),
                    instant(parent, Point::Entry),
                    EdgeSource::Chain,
                ),
                (
                    self.condition_anchor(at),
                    instant(parent, Point::ConditionEntry),
                    EdgeSource::Chain,
                ),
                (
                    instant(parent, Point::Start),
                    instant(at, Point::Finish),
                    EdgeSource::Containment,
                ),
            ];
            out.extend(
                links.map(|(dependent, requirement, source)| gate(dependent, requirement, source)),
            );
        }
        for (requirement, class, source) in references(document, tree, node) {
            let Some(&required) = self.index.get(requirement) else {
                continue;
            };
            let dependent = match source {
                EdgeSource::Condition => self.condition_anchor(at),
                EdgeSource::Explicit
                | EdgeSource::StageOpening
                | EdgeSource::Work
                | EdgeSource::Entry
                | EdgeSource::Chain
                | EdgeSource::Containment => self.anchor(at),
            };
            out.push(Edge {
                dependent,
                requirement: instant(required, Point::Finish),
                class,
                source,
            });
        }
    }

    /// Sorts the edges into per-instant slices both ways.
    fn settle(&mut self, mut edges: Vec<Edge>) {
        edges.sort();
        edges.dedup();
        let slots = self.keys.len() * Point::ALL.len();
        self.waits = offsets(slots, edges.iter().map(|edge| edge.dependent.slot()));
        let mut dependents: Vec<usize> = (0..edges.len()).collect();
        dependents.sort_by_key(|&at| edges.get(at).map(|edge| (edge.requirement, edge.dependent)));
        self.dependents_at = offsets(
            slots,
            dependents
                .iter()
                .filter_map(|&at| edges.get(at))
                .map(|edge| edge.requirement.slot()),
        );
        self.dependents = dependents;
        self.pruned = vec![true; edges.len()];
        self.edges = edges;
        assert_eq!(self.waits.len(), slots + 1);
        assert_eq!(self.dependents.len(), self.edges.len());
    }

    /// Marks the pruned set: every edge but those that wait on a not-relevant node or hold
    /// one's own work (a requirement entry excepted, as a pass-through for force-included
    /// descendants), and the condition gates and condition-chain links into a force-included
    /// node.
    pub(crate) fn prune(&mut self, document: &Document, relevances: &Relevances) {
        let usable = |graph: &Self, instant: Instant| {
            instant.point == Point::Entry
                || graph
                    .key(instant.node)
                    .is_some_and(|key| relevances.value(key) != Relevance::NotRelevant)
        };
        let dropped_by_force = |graph: &Self, edge: &Edge| {
            let condition_chain = edge.source == EdgeSource::Condition
                || (edge.source == EdgeSource::Chain
                    && edge.requirement.point == Point::ConditionEntry);
            condition_chain
                && graph
                    .key(edge.dependent.node)
                    .is_some_and(|key| forced(document, key))
        };
        let pruned: Vec<bool> = self
            .edges
            .iter()
            .map(|edge| {
                usable(self, edge.dependent)
                    && usable(self, edge.requirement)
                    && !dropped_by_force(self, edge)
            })
            .collect();
        self.pruned = pruned;
        assert_eq!(self.pruned.len(), self.edges.len());
    }
}

/// A gate edge.
fn gate(dependent: Instant, requirement: Instant, source: EdgeSource) -> Edge {
    Edge {
        dependent,
        requirement,
        class: EdgeClass::Gate,
        source,
    }
}

/// Offsets for slices of a sorted sequence of slots: slot `s` holds positions
/// `offsets[s]..offsets[s + 1]`.
fn offsets(slots: usize, sorted: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut counts = vec![0_usize; slots + 1];
    for slot in sorted {
        if let Some(count) = counts.get_mut(slot + 1) {
            *count += 1;
        }
    }
    for at in 1..counts.len() {
        let before = counts.get(at - 1).copied().unwrap_or(0);
        if let Some(count) = counts.get_mut(at) {
            *count += before;
        }
    }
    counts
}

/// A node's references that become edges: explicit requirements, a stage's opening, and the
/// decisions its condition reads, each with its class and source. References another stage
/// reports are left out (see [`Dependencies::build`]).
fn references<'d>(
    document: &Document,
    tree: &Tree,
    node: &'d Node<KeyRefs>,
) -> Vec<(&'d NodeKey, EdgeClass, EdgeSource)> {
    let related = |other: &NodeKey| {
        *other == node.key
            || tree.is_ancestor(other, &node.key)
            || tree.is_ancestor(&node.key, other)
    };
    let kind_of = |other: &NodeKey| document.nodes.get(other).map(Node::kind);
    let mut found: Vec<(&NodeKey, EdgeClass, EdgeSource)> = node
        .requires
        .iter()
        .filter(|requirement| !related(requirement))
        .map(|requirement| (requirement, EdgeClass::Gate, EdgeSource::Explicit))
        .collect();
    if let Payload::Group(group) = &node.payload
        && let Some(opens_at) = &group.opens_at
        && kind_of(opens_at) == Some(NodeKind::Milestone)
        && !tree.is_ancestor(&node.key, opens_at)
    {
        let class = if group.gates {
            EdgeClass::Gate
        } else {
            EdgeClass::DateOnly
        };
        found.push((opens_at, class, EdgeSource::StageOpening));
    }
    let decisions = node
        .relevant_when
        .iter()
        .flat_map(|condition| condition.decisions());
    for decision in decisions {
        if kind_of(decision) == Some(NodeKind::Decision) && !related(decision) {
            found.push((decision, EdgeClass::Gate, EdgeSource::Condition));
        }
    }
    found
}

impl Dependencies {
    /// The node's index, when it is in the graph.
    #[must_use]
    pub fn node_index(&self, key: &NodeKey) -> Option<NodeIndex> {
        self.index.get(key).copied()
    }

    /// Every node's key, in index order.
    #[must_use]
    pub(crate) fn keys(&self) -> &[NodeKey] {
        &self.keys
    }

    /// The key of the node at `index`.
    #[must_use]
    pub fn key(&self, index: NodeIndex) -> Option<&NodeKey> {
        self.keys.get(index.get())
    }

    /// How many nodes the graph holds.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.keys.len()
    }

    /// True when the node has children (its entries exist).
    #[must_use]
    pub fn is_container(&self, index: NodeIndex) -> bool {
        self.containers.get(index.get()).copied().unwrap_or(false)
    }

    /// Where requirements on the node wait: a container's requirement entry, else its start.
    #[must_use]
    pub fn anchor(&self, node: NodeIndex) -> Instant {
        if self.is_container(node) {
            Instant::new(node, Point::Entry)
        } else {
            Instant::new(node, Point::Start)
        }
    }

    /// Where condition gates on the node wait: a container's condition entry, else its start.
    #[must_use]
    pub fn condition_anchor(&self, node: NodeIndex) -> Instant {
        if self.is_container(node) {
            Instant::new(node, Point::ConditionEntry)
        } else {
            Instant::new(node, Point::Start)
        }
    }

    /// Every edge of the set.
    pub fn edges(&self, set: EdgeSet) -> impl Iterator<Item = &Edge> {
        self.edges
            .iter()
            .zip(&self.pruned)
            .filter(move |(_, pruned)| set == EdgeSet::Full || **pruned)
            .map(|(edge, _)| edge)
    }

    /// The edges `instant` waits on, in the set.
    pub fn waits(&self, instant: Instant, set: EdgeSet) -> impl Iterator<Item = &Edge> {
        let slot = instant.slot();
        let from = self.waits.get(slot).copied().unwrap_or(0);
        let to = self.waits.get(slot + 1).copied().unwrap_or(from);
        (from..to).filter_map(move |at| self.edge_in(at, set))
    }

    /// The edges that wait on `instant`, in the set.
    pub fn waited_by(&self, instant: Instant, set: EdgeSet) -> impl Iterator<Item = &Edge> {
        let slot = instant.slot();
        let from = self.dependents_at.get(slot).copied().unwrap_or(0);
        let to = self.dependents_at.get(slot + 1).copied().unwrap_or(from);
        let positions = self.dependents.get(from..to).unwrap_or(&[]);
        positions
            .iter()
            .filter_map(move |&at| self.edge_in(at, set))
    }

    fn edge_in(&self, at: usize, set: EdgeSet) -> Option<&Edge> {
        let in_set = set == EdgeSet::Full || self.pruned.get(at) == Some(&true);
        self.edges.get(at).filter(|_| in_set)
    }
}
