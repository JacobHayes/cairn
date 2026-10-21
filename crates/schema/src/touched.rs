//! The touched set (PRD glossary; H5): everything a patch or event writes or removes, by key
//! and field, including a removed node's subtree and every incident edge. Two changes are
//! safe to reorder when their touched sets do not overlap, which is what lets a client
//! retry a stale patch on its own.
//!
//! A touched set is only ever read off writes ([`crate::Write::keys`]): an event's delta, or
//! the change set the engine's apply builds. What a patch touches depends on what it finds
//! (removing a role clears its fill, reopening a node its bypass), so nothing predicts it
//! from the mutations alone; a stale answer carries the patch's own, from applying it at the
//! current revisions
//! (decisions/2026-10-21-a-patchs-touched-set-comes-from-what-apply-writes.md).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::{Domain, GraphId};
use crate::id::NodeKey;
use crate::patch::Patch;
use crate::record::{GraphKey, RecordKey};
use crate::rejection::{RevisionConflict, RevisionOf};

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

impl Patch {
    /// H5's safe retry: the patch to resubmit after it was answered stale with `conflicts`,
    /// `intervening`, and `touched` (what it would write at the current revisions), or none
    /// when it is not safe: what intervened overlaps what it would write, or a revision moved
    /// that the patch names nowhere it could be rebased (a merge's journeys, a proposal's
    /// editing revision), or a conflict names a current revision not past the one the patch
    /// named (rebasing would move it backward).
    ///
    /// The resubmission names the revisions the rejection reports, never a newer one read
    /// since, because only the events up to those were compared; if more landed in between,
    /// it is stale again and compared again. It keeps its patch id: a stale patch left no
    /// receipt, so the id still names one change. Guards are evaluated at apply either way,
    /// so a retry that would now break an invariant is rejected, never widened. It only ever
    /// moves forward, so no answer can send it back and forth between two revisions
    /// (decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md).
    /// Every client asks this one rule: the Rust clients directly, the browser through the
    /// wasm module.
    #[must_use]
    pub fn rebased(
        &self,
        conflicts: &[RevisionConflict],
        intervening: &TouchedSet,
        touched: &TouchedSet,
    ) -> Option<Patch> {
        if conflicts.is_empty() || intervening.overlaps(touched) {
            return None;
        }
        let target = self.target.domain();
        let mut rebased = self.clone();
        for conflict in conflicts {
            if conflict.current <= conflict.expected {
                return None;
            }
            match &conflict.of {
                RevisionOf::Domain(domain) if *domain == target => {
                    rebased.base_revision = conflict.current;
                }
                RevisionOf::Domain(Domain::Deployment) if self.deployment_revision.is_some() => {
                    rebased.deployment_revision = Some(conflict.current);
                }
                RevisionOf::Domain(_) | RevisionOf::Proposal(_) => return None,
            }
        }
        Some(rebased)
    }
}

impl FromIterator<RecordKey> for TouchedSet {
    fn from_iter<I: IntoIterator<Item = RecordKey>>(keys: I) -> Self {
        Self(keys.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Revision;

    fn patch(target: &str, deployment_revision: Option<u32>) -> Patch {
        let yaml = format!(
            "id: p_one\ntarget: {target}\nbase_revision: 2\nmutations:\n- op: transition\n  node: n_a\n  transition: start\n"
        );
        let mut patch: Patch = crate::from_yaml(&yaml).unwrap();
        patch.deployment_revision = deployment_revision.map(|number| number.try_into().unwrap());
        patch
    }

    fn conflict(of: RevisionOf, expected: u32, current: u32) -> RevisionConflict {
        RevisionConflict {
            of,
            expected: expected.try_into().unwrap(),
            current: current.try_into().unwrap(),
        }
    }

    fn journey(id: &str) -> RevisionOf {
        RevisionOf::Domain(Domain::Journey(id.parse().unwrap()))
    }

    /// What the patch writes: node `n_a`'s state.
    fn writes() -> TouchedSet {
        on_node("n_a")
    }

    /// What touches node `node` of journey `j_one` alone.
    fn on_node(node: &str) -> TouchedSet {
        let graph = GraphId::Journey("j_one".parse().unwrap());
        [RecordKey::InGraph {
            graph,
            key: GraphKey::Node(node.parse().unwrap()),
        }]
        .into_iter()
        .collect()
    }

    fn revision(number: u32) -> Revision {
        number.try_into().unwrap()
    }

    #[test]
    fn a_patch_is_rebased_onto_the_revisions_the_rejection_reports() {
        let deployment = RevisionOf::Domain(Domain::Deployment);
        let original = patch("{journey: j_one}", Some(1));
        let conflicts = [conflict(journey("j_one"), 2, 4), conflict(deployment, 1, 3)];
        let retry = original
            .rebased(&conflicts, &on_node("n_b"), &writes())
            .unwrap();
        assert_eq!(retry.base_revision, revision(4));
        assert_eq!(retry.deployment_revision, Some(revision(3)));
        assert_eq!(
            (retry.id, retry.mutations),
            (original.id, original.mutations)
        );
    }

    #[test]
    fn an_unsafe_retry_is_none() {
        let original = patch("{journey: j_one}", None);
        let mine = [conflict(journey("j_one"), 2, 3)];
        let cases = [
            (
                &mine[..],
                on_node("n_a"),
                "what intervened touched the node",
            ),
            (&[][..], on_node("n_b"), "nothing moved"),
            (
                &[conflict(journey("j_two"), 1, 2)][..],
                on_node("n_b"),
                "another journey moved",
            ),
            (
                &[conflict(RevisionOf::Domain(Domain::Deployment), 1, 2)][..],
                on_node("n_b"),
                "the patch names no deployment revision to move",
            ),
            (
                &[conflict(
                    RevisionOf::Proposal("pr_one".parse().unwrap()),
                    1,
                    2,
                )][..],
                on_node("n_b"),
                "a proposal's revision moved",
            ),
            (
                &[conflict(journey("j_one"), 2, 1)][..],
                TouchedSet::default(),
                "the patch names a revision past the current one",
            ),
            (
                &[conflict(journey("j_one"), 2, 2)][..],
                TouchedSet::default(),
                "the revision the patch names has not moved",
            ),
        ];
        for (conflicts, intervening, why) in cases {
            assert_eq!(
                original.rebased(conflicts, &intervening, &writes()),
                None,
                "{why}"
            );
        }
    }
}
