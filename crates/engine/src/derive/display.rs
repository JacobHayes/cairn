//! D8, display state: the one state every surface shows for a node, composed from relevance,
//! stored state, effective skip, auto-reach, blocking and snooze, and from the D3 flags it
//! leaves in place. Stored state is what transitions act on; display state is what people and
//! agents read.
//!
//! First match wins:
//!
//! 1. `not_relevant`: relevance is `not_relevant` and settled (nothing pending).
//! 2. `skipped`: stored `skipped`, or effectively skipped (D1a).
//! 3. `done`: stored `done`, `decided` or `reached`, auto-reached, or a group that satisfies
//!    dependencies.
//! 4. `snoozed`: a snooze holds.
//! 5. `active`: stored `active`, or a container whose only unsatisfied waits are its own
//!    children and some descendant has started (when it waits on nothing, it is `ready` to
//!    finish).
//! 6. `conditional`: relevance `undecided`; or `not_relevant` but pending on an undecided
//!    decision (`pending_on`), which relevance itself leaves unchanged.
//! 7. `blocked`: in scope, unfinished, and its own or an inherited gate is unsatisfied; for a
//!    container only its own gates count, never its children.
//! 8. `scheduled`: an `auto_reach` milestone, unblocked, whose date is ahead.
//! 9. `ready`: everything else.
//!
//! The legacy group state (D1) is kept beside it, deprecated, on the same helpers.
//!
//! Cost at `node_count_max`: a node reads its passes' flags; a container also reads its
//! descendants, 2,000 x 2,000 at worst over every node, O(nodes) per container.

use cairn_schema::{DisplayState, GroupState, NodeKey, NodeKind, Payload, Relevance, State};

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
        if stored == State::Skipped || self.skips.skipped_by(key).is_some() {
            return DisplayState::Skipped;
        }
        let group = node.kind() == NodeKind::Group;
        let finished = matches!(stored, State::Done | State::Decided | State::Reached);
        let satisfied_group =
            group && relevance.value != Relevance::NotRelevant && blocking.satisfies(key);
        if finished || blocking.auto_reached(key) || satisfied_group {
            return DisplayState::Done;
        }
        if blocking.snoozed(key).is_some() {
            return DisplayState::Snoozed;
        }
        let container = graph.tree().is_container(key);
        // A container that waits on nothing is ready to finish, not active.
        let working =
            container && self.entered(key) && !blocking.deps_done(key) && Self::started(graph, key);
        let started = stored == State::Active || working;
        if started {
            return DisplayState::Active;
        }
        if relevance.value != Relevance::Relevant {
            return DisplayState::Conditional;
        }
        let waiting = if container {
            !self.entered(key)
        } else {
            blocking.blocked(key)
        };
        if waiting {
            return DisplayState::Blocked;
        }
        if self.reach_is_ahead(graph, key) {
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
            && self.relevance.in_scope(key)
            && !self.blocking.closed(key)
    }

    /// D1: a group's legacy display state from its children and dependencies. Deprecated:
    /// [`Derived::display_state`] is the state of every node.
    ///
    /// # Panics
    ///
    /// When `graph` is not the one derived, or does not hold `key`.
    #[must_use]
    pub fn group_state(&self, graph: &Graph, key: &NodeKey) -> GroupState {
        let state = |key: &NodeKey| {
            let Some(node) = graph.node(key) else {
                panic!("the graph holds {key}");
            };
            stored_state(graph.document(), node)
        };
        if self.relevance.value(key) == Relevance::NotRelevant {
            return GroupState::NotRelevant;
        }
        if state(key) == State::Skipped || self.skips.skipped_by(key).is_some() {
            return GroupState::Skipped;
        }
        if self.blocking.satisfies(key) {
            return GroupState::Done;
        }
        if !self.entered(key) {
            return GroupState::Waiting;
        }
        if Self::started(graph, key) {
            GroupState::Active
        } else {
            GroupState::NotStarted
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

    /// Some node beneath it has started or finished.
    fn started(graph: &Graph, key: &NodeKey) -> bool {
        let document = graph.document();
        graph.tree().descendants(key).iter().any(|below| {
            graph.node(below).is_some_and(|node| {
                matches!(
                    stored_state(document, node),
                    State::Active | State::Done | State::Decided | State::Reached
                )
            })
        })
    }

    /// F1: an unblocked, pending `auto_reach` milestone whose effective date is ahead.
    fn reach_is_ahead(&self, graph: &Graph, key: &NodeKey) -> bool {
        let Some(node) = graph.node(key) else {
            return false;
        };
        let Payload::Milestone(milestone) = &node.payload else {
            return false;
        };
        milestone.auto_reach
            && self.blocking.actionable(key)
            && self
                .dates
                .effective_date(key)
                .is_some_and(|effective| effective.date > self.today)
    }
}
