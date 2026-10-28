//! D8, display state: the one state every surface shows for a node, composed from relevance,
//! stored state, effective skip, auto-done, blocking and snooze, and from the D3 flags it
//! leaves in place. Stored state is what transitions act on; display state is what people and
//! agents read.
//!
//! First match wins:
//!
//! 1. `not_relevant`: relevance is `not_relevant` and settled (nothing pending).
//! 2. `skipped`: stored `skipped`, or effectively skipped (D1a).
//! 3. `done`: stored `done` or `decided`, done by its date, or a group that satisfies
//!    dependencies.
//! 4. `snoozed`: a snooze holds.
//! 5. `active`: stored `active`, or a container whose only unsatisfied waits are its own
//!    children and work beneath it has started (`work_has_started`; when it waits on
//!    nothing, it is `ready` to finish).
//! 6. `conditional`: relevance `undecided`; or `not_relevant` but pending on an undecided
//!    decision (`pending_on`), which relevance itself leaves unchanged.
//! 7. `blocked`: in scope, unfinished, and its own or an inherited gate is unsatisfied; for a
//!    container only its own gates count, never its children.
//! 8. `scheduled`: unblocked and held off the acting frontier by a date ahead (pass 5's
//!    [`super::blocking::Scheduled`]): an `auto_done` milestone's effective date, another
//!    milestone's pinned date, or the own not-before date of unstarted work or a decision.
//! 9. `ready`: everything else.
//!
//! Cost at `node_count_max`: a node reads its passes' flags; a container also reads its
//! descendants, 2,000 x 2,000 at worst over every node, O(nodes) per container.

use cairn_schema::{DisplayState, NodeKey, NodeKind, Relevance, State};

use super::dependencies::{Instant, Point};
use super::{Derived, stored_state};
use crate::graph::Graph;

impl Derived {
    /// D8: the state every surface shows for the node, `graph` being the one derived.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived, or does not hold `key`.
    #[must_use]
    pub fn display_state(&self, graph: &Graph, key: &NodeKey) -> DisplayState {
        let Some(node) = graph.node(key) else {
            panic!("the graph holds {key}");
        };
        let Some(relevance) = self.relevance.get(key) else {
            panic!("the graph derived holds {key}");
        };
        let stored = stored_state(graph.document(), node);
        let blocking = &self.blocking;
        let pending = !relevance.pending_on.is_empty();
        if relevance.value == Relevance::NotRelevant && !pending {
            return DisplayState::NotRelevant;
        }
        if self.skips.skipped(stored, key) {
            return DisplayState::Skipped;
        }
        let group = node.kind() == NodeKind::Group;
        let finished = matches!(stored, State::Done | State::Decided);
        let satisfied_group =
            group && relevance.value != Relevance::NotRelevant && blocking.satisfies(key);
        if finished || blocking.done_by_date(key) || satisfied_group {
            return DisplayState::Done;
        }
        if blocking.snoozed(key).is_some() {
            return DisplayState::Snoozed;
        }
        let container = graph.tree().is_container(key);
        // A container that waits on nothing is ready to finish, not active.
        let working = container
            && self.entered(key)
            && !blocking.deps_done(key)
            && Self::work_has_started(graph, key);
        let started = stored == State::Active || working;
        if started {
            return DisplayState::Active;
        }
        if relevance.value != Relevance::Relevant {
            return DisplayState::Conditional;
        }
        if self.waiting(graph, key) {
            return DisplayState::Blocked;
        }
        if blocking.scheduled(key).is_some() {
            return DisplayState::Scheduled;
        }
        DisplayState::Ready
    }

    /// E1: no one owns the node, for a non-group node that is in scope and unfinished; a
    /// not-relevant or finished node, and a group, never is.
    #[must_use]
    pub fn is_unassigned(&self, graph: &Graph, key: &NodeKey) -> bool {
        self.participation.is_unassigned(key)
            && graph
                .node(key)
                .is_some_and(|node| node.kind() != NodeKind::Group)
            && self.open(key)
    }

    /// D8: the node is still to do and in scope: nothing has closed it (skipped, finished, done
    /// by its date, or a group whose finish is satisfied) and an answer has not ruled it out.
    #[must_use]
    pub fn open(&self, key: &NodeKey) -> bool {
        self.relevance.in_scope(key) && !self.blocking.closed(key)
    }

    /// B6: a snooze can sit on the node: it is in scope with something left to do, and a group
    /// also has work in scope left beneath it. The snooze check and the web app's offers read this.
    #[must_use]
    pub fn snoozable(&self, graph: &Graph, key: &NodeKey) -> bool {
        self.open(key) && !self.group_without_work(graph, key)
    }

    /// B6: a group with no node in scope and unfinished beneath it, so nothing is left for a
    /// snooze of it to hold back.
    #[must_use]
    pub fn group_without_work(&self, graph: &Graph, key: &NodeKey) -> bool {
        let is_group = |key: &NodeKey| {
            graph
                .node(key)
                .is_some_and(|node| node.kind() == NodeKind::Group)
        };
        is_group(key)
            && !graph
                .tree()
                .descendants(key)
                .iter()
                .any(|below| !is_group(below) && self.open(below))
    }

    /// D8: the node waits on a gate of its own or one it inherits: a container, on its
    /// entries (its children are not a gate); anything else, on being blocked.
    pub(super) fn waiting(&self, graph: &Graph, key: &NodeKey) -> bool {
        if graph.tree().is_container(key) {
            !self.entered(key)
        } else {
            self.blocking.blocked(key)
        }
    }

    /// Its own and inherited gates are satisfied: a container's entries (its requirements,
    /// opening and conditions), never its children; anything else's start.
    fn entered(&self, key: &NodeKey) -> bool {
        let dependencies = &self.dependencies;
        dependencies.node_index(key).is_some_and(|at| {
            let points: &[Point] = if dependencies.is_container(at) {
                &[Point::Entry, Point::ConditionEntry]
            } else {
                &[Point::Start]
            };
            points
                .iter()
                .all(|&point| self.blocking.instant(Instant::new(at, point)))
        })
    }

    /// D8: work has started beneath a container: some deliverable or action under it is
    /// stored `active` or `done`. A decision answered or a milestone done is not work
    /// started (it opens or closes a gate), and a skipped node did none. Derived on read, so
    /// stopping the only started node takes the container back to what its waits say.
    fn work_has_started(graph: &Graph, key: &NodeKey) -> bool {
        let document = graph.document();
        graph.tree().descendants(key).iter().any(|below| {
            graph.node(below).is_some_and(|node| {
                matches!(node.kind(), NodeKind::Deliverable | NodeKind::Action)
                    && matches!(stored_state(document, node), State::Active | State::Done)
            })
        })
    }
}
