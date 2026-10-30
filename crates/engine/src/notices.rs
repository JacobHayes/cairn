//! Notices (A20; ARCHITECTURE, Engine > Date network and Conditions): the advisory findings
//! about a route graph. In a graph with a `final` milestone, a non-group node with no chain
//! to or from it is listed, because neither its priority (gravity reaches only what sits
//! upstream of a node) nor its dates (bounds read along constraints) feel that milestone.
//!
//! A chain follows the date network's constraints, read over the graph's structural scope
//! (every node relevant, nothing skipped, the full set of dependency edges: explicit
//! requirements, implicit gates, containment) and so its date rules too, each from the
//! instant that must come first to the one that follows. Every condition counts as relevant:
//! the full set ignores relevance, and no rule is dropped for a not-relevant node, so a conditional branch is checked as if it applied.
//! The one link the network does not hold is a `feeds_milestone` pin, which ties the date
//! decision's answer to the milestone's date both ways. A decision that fills a role is
//! exempt, since it acts through participations. Notices are computed, never stored, and
//! reject nothing (A15).
//!
//! Cost at `node_count_max` (2,000 nodes): the network's 10,001 instants and, on the date
//! limits graph, 310,241 constraints, held once at 36 bytes each (11 MB) while two adjacency
//! lists over the instants are built from them (5 MB), then dropped; the two breadth-first
//! searches each visit every instant and constraint once: O(nodes + edges). The slot pairs
//! this replaced were about 6 MB with 2.5 MB of lists, so peak memory is about 1.9 times what it
//! was.

use std::collections::VecDeque;

use cairn_schema::{AnswerSpec, KeyRefs, Node, NodeKind, Notice, NoticeCode, Payload};

use crate::derive::dates::network::{Constraint, Network, Scope};
use crate::derive::dependencies::{Dependencies, Instant, Point};
use crate::graph::{Document, Graph};

/// A20: the notices of a route graph, in path order; none when it has no `final` milestone.
///
/// # Panics
///
/// Never: every index is a node position of the graph's own dependency graph.
#[must_use]
pub fn notices(graph: &Graph) -> Vec<Notice> {
    let document = graph.document();
    let Some(last) = document.nodes.values().find(|node| is_final(node)) else {
        return Vec::new();
    };
    let dependencies = Dependencies::build(document, graph.tree());
    let Some(last_at) = dependencies.node_index(&last.key) else {
        return Vec::new();
    };
    let (network, constraints) =
        Network::emit(document, &dependencies, &Scope::structural(&dependencies));
    let chains = Chains::build(document, &dependencies, &network, &constraints);
    drop(constraints);
    let last_finish = network.finish(last_at).index();
    let mut felt = chains.reached(last_finish, Way::After);
    for (at, reached) in chains
        .reached(last_finish, Way::Before)
        .into_iter()
        .enumerate()
    {
        if let Some(held) = felt.get_mut(at) {
            *held |= reached;
        }
    }
    let mut found: Vec<Notice> = document
        .nodes
        .values()
        .filter(|node| node.kind() != NodeKind::Group && !fills_role(node))
        .filter_map(|node| {
            let at = dependencies.node_index(&node.key)?;
            let slots = Point::ALL
                .map(|point| network.slot(Instant::new(at, point)))
                .into_iter()
                .chain([network.answer(at)]);
            let connected = slots
                .into_iter()
                .any(|slot| felt.get(slot.index()) == Some(&true));
            let path = graph.tree().path(&node.key)?;
            (!connected).then(|| Notice {
                code: NoticeCode::Unanchored,
                node: node.key.clone(),
                path: path.clone(),
                message: format!(
                    "{} has no chain to or from the final milestone {}, so neither its priority \
                     nor its dates feel it",
                    node.title.as_str(),
                    last.title.as_str(),
                ),
            })
        })
        .collect();
    found.sort_by(|left, right| left.path.cmp(&right.path));
    found
}

fn is_final(node: &Node<KeyRefs>) -> bool {
    matches!(&node.payload, Payload::Milestone(milestone) if milestone.is_final)
}

fn fills_role(node: &Node<KeyRefs>) -> bool {
    matches!(
        &node.payload,
        Payload::Decision(decision)
            if matches!(
                decision.answer,
                AnswerSpec::Identity { fills_role: Some(_) } | AnswerSpec::IdentityList { fills_role: Some(_) }
            )
    )
}

/// Which way a search follows the chains from a slot.
#[derive(Clone, Copy)]
enum Way {
    /// To what comes after it.
    After,
    /// To what comes before it.
    Before,
}

/// The chains between slots, by slot: what comes after each and what comes before it.
struct Chains {
    after: Vec<Vec<usize>>,
    before: Vec<Vec<usize>>,
}

impl Chains {
    /// The network's constraints, each from the slot that must come first to the one that
    /// follows, and the one link the network does not hold: a date decision's pin.
    fn build(
        document: &Document,
        dependencies: &Dependencies,
        network: &Network,
        constraints: &[Constraint],
    ) -> Self {
        let mut chains = Self {
            after: vec![Vec::new(); network.slot_count()],
            before: vec![Vec::new(); network.slot_count()],
        };
        for constraint in constraints {
            chains.link(constraint.before.index(), constraint.after.index());
        }
        for node in document.nodes.values() {
            if let Payload::Decision(decision) = &node.payload
                && let AnswerSpec::Date {
                    feeds_milestone: Some(milestone),
                } = &decision.answer
                && let Some(at) = dependencies.node_index(&node.key)
                && let Some(pinned) = dependencies.node_index(milestone)
            {
                // The pin makes the answer and the milestone's date one: a chain runs both ways.
                let (answer, date) = (network.answer(at).index(), network.finish(pinned).index());
                chains.link(answer, date);
                chains.link(date, answer);
            }
        }
        chains
    }

    fn link(&mut self, from: usize, to: usize) {
        if let Some(list) = self.after.get_mut(from) {
            list.push(to);
        }
        if let Some(list) = self.before.get_mut(to) {
            list.push(from);
        }
    }

    /// Every slot with a chain from `start` (after) or to it (before), `start` included.
    fn reached(&self, start: usize, way: Way) -> Vec<bool> {
        let next = match way {
            Way::After => &self.after,
            Way::Before => &self.before,
        };
        let mut seen = vec![false; next.len()];
        let mut queue = VecDeque::from([start]);
        while let Some(current) = queue.pop_front() {
            if let Some(held) = seen.get_mut(current) {
                if *held {
                    continue;
                }
                *held = true;
            }
            queue.extend(next.get(current).into_iter().flatten().copied());
        }
        seen
    }
}
