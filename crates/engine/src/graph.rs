//! The in-memory graph model (ARCHITECTURE, Engine > Model): the graph document with its
//! containment tree indexed by parent and by path. [`Tree`] indexes any document, including
//! a candidate that breaks the tree (a missing parent, a cycle, a sibling id twice), and
//! records what it found for validation to report; [`Graph`] is a document that passed every
//! structural invariant.
//!
//! Cost at `node_count_max` (2,000 nodes): building the tree is one pass to link parents and
//! one traversal from the roots, O(n log n) for the sorted maps, a few hundred kilobytes of
//! keys and paths; it runs once per validation, not per mutation.

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::limits::{CONTAINMENT_DEPTH_MAX, NODE_COUNT_MAX};
use cairn_schema::{Deployment, KeyRefs, Node, NodeKey, Path, Violation, Violations};

use crate::validate;

/// The graph document (resolved form).
pub type Document = cairn_schema::Graph;

/// Something wrong with a document's containment, found while indexing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TreeIssue {
    /// The node's parent does not exist.
    MissingParent { node: NodeKey, parent: NodeKey },
    /// The node's parent is a decision or milestone, which have no children (A2).
    LeafParent { node: NodeKey, parent: NodeKey },
    /// These nodes are each other's ancestors.
    Cycle { members: Vec<NodeKey> },
    /// The node sits deeper than `containment_depth_max`.
    TooDeep { node: NodeKey },
    /// Two siblings share an id, so they share a path.
    DuplicateId { first: NodeKey, second: NodeKey },
}

/// The containment tree of a document: children by parent, the roots, and the path of every
/// node reachable from a root.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tree {
    parents: BTreeMap<NodeKey, NodeKey>,
    children: BTreeMap<NodeKey, Vec<NodeKey>>,
    roots: Vec<NodeKey>,
    paths: BTreeMap<NodeKey, Path>,
    by_path: BTreeMap<Path, NodeKey>,
    issues: Vec<TreeIssue>,
}

impl Tree {
    /// Indexes a document's containment, tolerating a broken tree.
    ///
    /// # Panics
    ///
    /// When the document holds more than `node_count_max` nodes, which its type prevents.
    #[must_use]
    pub fn build(document: &Document) -> Self {
        assert!(document.nodes.len() <= NODE_COUNT_MAX as usize);
        let mut tree = Tree::default();
        for node in document.nodes.values() {
            tree.link(document, node);
        }
        tree.walk_from_roots(document);
        tree.find_cycles(document);
        assert!(tree.paths.len() <= document.nodes.len());
        tree
    }

    fn link(&mut self, document: &Document, node: &Node<KeyRefs>) {
        let Some(parent_key) = &node.parent else {
            self.roots.push(node.key.clone());
            return;
        };
        let Some(parent) = document.nodes.get(parent_key) else {
            self.issues.push(TreeIssue::MissingParent {
                node: node.key.clone(),
                parent: parent_key.clone(),
            });
            return;
        };
        if parent.kind().is_leaf() {
            self.issues.push(TreeIssue::LeafParent {
                node: node.key.clone(),
                parent: parent_key.clone(),
            });
        }
        self.parents.insert(node.key.clone(), parent_key.clone());
        self.children
            .entry(parent_key.clone())
            .or_default()
            .push(node.key.clone());
    }

    /// Gives every node reachable from a root its path, depth first with an explicit stack
    /// (PRACTICES, No recursion); a path past the depth limit stops the descent there.
    fn walk_from_roots(&mut self, document: &Document) {
        let mut stack: Vec<(NodeKey, Option<Path>)> = self
            .roots
            .iter()
            .rev()
            .map(|root| (root.clone(), None))
            .collect();
        while let Some((key, parent_path)) = stack.pop() {
            let Some(node) = document.nodes.get(&key) else {
                continue;
            };
            let path = match &parent_path {
                None => Path::root(node.id.clone()),
                Some(parent) => {
                    let Ok(path) = parent.child(node.id.clone()) else {
                        self.issues.push(TreeIssue::TooDeep { node: key });
                        continue;
                    };
                    path
                }
            };
            if let Some(first) = self.by_path.get(&path) {
                self.issues.push(TreeIssue::DuplicateId {
                    first: first.clone(),
                    second: key.clone(),
                });
            } else {
                self.by_path.insert(path.clone(), key.clone());
            }
            for child in self.children.get(&key).into_iter().flatten().rev() {
                stack.push((child.clone(), Some(path.clone())));
            }
            self.paths.insert(key, path);
        }
    }

    /// Finds parent cycles among the nodes no root reaches: each walk up the parent chain
    /// either ends at a node already classified or closes a loop, which is reported once.
    fn find_cycles(&mut self, document: &Document) {
        let mut settled: BTreeSet<NodeKey> = self.paths.keys().cloned().collect();
        for start in document.nodes.as_map().keys() {
            let mut walk: Vec<NodeKey> = Vec::new();
            let mut current = start.clone();
            // A chain longer than the node count must have closed a loop already.
            for _ in 0..=document.nodes.len() {
                if settled.contains(&current) {
                    break;
                }
                if let Some(position) = walk.iter().position(|key| *key == current) {
                    let mut members: Vec<NodeKey> = walk.get(position..).unwrap_or(&[]).to_vec();
                    members.sort();
                    self.issues.push(TreeIssue::Cycle { members });
                    break;
                }
                walk.push(current.clone());
                match self.parents.get(&current) {
                    Some(parent) => current = parent.clone(),
                    None => break,
                }
            }
            settled.extend(walk);
        }
    }

    /// The node's parent, when it exists.
    #[must_use]
    pub fn parent(&self, key: &NodeKey) -> Option<&NodeKey> {
        self.parents.get(key)
    }

    /// The node's children, in key order.
    #[must_use]
    pub fn children(&self, key: &NodeKey) -> &[NodeKey] {
        self.children.get(key).map_or(&[], Vec::as_slice)
    }

    /// True when the node has children (PRD glossary, Container).
    #[must_use]
    pub fn is_container(&self, key: &NodeKey) -> bool {
        !self.children(key).is_empty()
    }

    /// The root nodes, in key order.
    #[must_use]
    pub fn roots(&self) -> &[NodeKey] {
        &self.roots
    }

    /// The node's path, when a root reaches it.
    #[must_use]
    pub fn path(&self, key: &NodeKey) -> Option<&Path> {
        self.paths.get(key)
    }

    /// The node at a path.
    #[must_use]
    pub fn key_at(&self, path: &Path) -> Option<&NodeKey> {
        self.by_path.get(path)
    }

    /// Every node below `key`, depth first, with an explicit stack bounded by the node count.
    ///
    /// # Panics
    ///
    /// Never: each node is visited once.
    #[must_use]
    pub fn descendants(&self, key: &NodeKey) -> Vec<NodeKey> {
        let mut found = Vec::new();
        let mut seen = BTreeSet::from([key.clone()]);
        let mut stack: Vec<&NodeKey> = self.children(key).iter().collect();
        while let Some(next) = stack.pop() {
            // A cycle in a broken candidate must not loop forever.
            if !seen.insert(next.clone()) {
                continue;
            }
            found.push(next.clone());
            stack.extend(self.children(next));
        }
        assert!(found.len() <= self.parents.len());
        found
    }

    /// True when `ancestor` is a proper ancestor of `key`. The walk is bounded by the depth
    /// limit plus one, so a cycle in a broken candidate ends it.
    #[must_use]
    pub fn is_ancestor(&self, ancestor: &NodeKey, key: &NodeKey) -> bool {
        let mut current = key;
        for _ in 0..=CONTAINMENT_DEPTH_MAX {
            match self.parents.get(current) {
                Some(parent) if parent == ancestor => return true,
                Some(parent) => current = parent,
                None => return false,
            }
        }
        false
    }

    pub(crate) fn issues(&self) -> &[TreeIssue] {
        &self.issues
    }
}

/// A graph that holds every structural invariant (PRD Invariants: graph), indexed by key,
/// parent, and path. A route version or draft has empty state; a journey's graph carries
/// its state, whose invariants hold too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    document: Document,
    tree: Tree,
}

impl Graph {
    /// Validates a graph document (A15: every violation listed), its plan layer included
    /// (F5). Relevance, which decides which date constraints apply, reads the deployment's
    /// entity aliases, so a journey is checked against the deployment it lives in; a route,
    /// which has no answers, against any.
    ///
    /// # Errors
    ///
    /// Every structural or state invariant the document breaks, or its contradictory chains.
    pub fn new(document: Document, deployment: &Deployment) -> Result<Self, Violations> {
        Self::checked(document, Vec::new(), deployment)
    }

    /// Validates a document, reporting `earlier` violations (found building it) first. The
    /// plan check needs derived relevance, so it runs once everything else holds.
    pub(crate) fn checked(
        document: Document,
        earlier: Vec<Violation>,
        deployment: &Deployment,
    ) -> Result<Self, Violations> {
        let tree = Tree::build(&document);
        let mut violations = earlier;
        violations.extend(validate::graph(&document, &tree));
        if let Ok(violations) = Violations::new(violations) {
            return Err(violations);
        }
        let graph = Self { document, tree };
        match crate::derive::dates::plan_violation(&graph, deployment) {
            Some(contradiction) => Err(Violations::new(vec![contradiction])
                .unwrap_or_else(|_| unreachable!("one violation"))),
            None => Ok(graph),
        }
    }

    /// A graph from a document whose graph stages the caller has just run clean.
    pub(crate) fn trusted(document: Document, tree: Tree) -> Self {
        Self { document, tree }
    }

    /// The document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The document, given up.
    #[must_use]
    pub fn into_document(self) -> Document {
        self.document
    }

    /// The containment tree.
    #[must_use]
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The node with `key`.
    #[must_use]
    pub fn node(&self, key: &NodeKey) -> Option<&Node<KeyRefs>> {
        self.document.nodes.get(key)
    }

    /// The node at `path`.
    #[must_use]
    pub fn node_at(&self, path: &Path) -> Option<&Node<KeyRefs>> {
        self.tree.key_at(path).and_then(|key| self.node(key))
    }
}
