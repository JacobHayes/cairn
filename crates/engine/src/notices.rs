//! Notices (A20; ARCHITECTURE, Engine > Date network and Conditions): the advisory findings
//! about a route graph. In a graph with a `final` milestone, a non-group node with no chain
//! to or from it is listed, because neither its priority (gravity reaches only what sits
//! upstream of a node) nor its dates (bounds read along constraints) feel that milestone.
//!
//! A chain follows the effective dependency graph's full structural set (explicit
//! requirements, implicit gates, stage openings, containment) and the date constraints (date
//! rules, stage closes, `feeds_milestone` pins), each from the instant that must come first
//! to the one that follows. Every condition counts as relevant: the full set ignores
//! relevance, and no rule or close is dropped for a not-relevant node, so a conditional
//! branch is checked as if it applied. A decision that fills a role is exempt, since it acts
//! through participations. Notices are computed, never stored, and reject nothing (A15).
//!
//! Cost at `node_count_max` (2,000 nodes, about 110,000 dependency edges and 258,000 rule
//! constraints): one adjacency build over 10,000 slots, then two breadth-first searches, each
//! visiting every slot and constraint once: O(nodes + edges), a few megabytes.

use std::collections::VecDeque;

use cairn_schema::{
    AnswerSpec, DateSource, KeyRefs, Node, NodeKey, NodeKind, Notice, NoticeCode, Payload,
};

use crate::derive::dependencies::{Dependencies, EdgeSet, Instant, Point};
use crate::graph::{Document, Graph, Tree};

/// Slots per node: the four instants, then the answer of a date decision; one more at the
/// end for the journey's `created_at`, which rules may measure from.
const SLOTS: usize = 5;
const ANSWER: usize = 4;

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
    let chains = Chains::build(document, graph.tree(), &dependencies);
    let last_finish = slot(Instant::new(last_at, Point::Finish));
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
            let connected =
                (0..SLOTS).any(|point| felt.get(at.get() * SLOTS + point) == Some(&true));
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
                AnswerSpec::Entity { fills_role: Some(_) } | AnswerSpec::EntityList { fills_role: Some(_) }
            )
    )
}

/// The slot of an instant of a node.
fn slot(instant: Instant) -> usize {
    let point = match instant.point {
        Point::Start => 0,
        Point::Finish => 1,
        Point::Entry => 2,
        Point::ConditionEntry => 3,
    };
    instant.node.get() * SLOTS + point
}

/// Which way a search follows the chains from a slot.
#[derive(Clone, Copy)]
enum Way {
    /// To what comes after it.
    After,
    /// To what comes before it.
    Before,
}

/// The chains between slots: `from` must come before `to`.
struct Chains {
    slots: usize,
    links: Vec<(usize, usize)>,
}

impl Chains {
    fn build(document: &Document, tree: &Tree, dependencies: &Dependencies) -> Self {
        let created_at = dependencies.node_count() * SLOTS;
        let mut links: Vec<(usize, usize)> = dependencies
            .edges(EdgeSet::Full)
            .map(|edge| (slot(edge.requirement), slot(edge.dependent)))
            .collect();
        for node in document.nodes.values() {
            let Some(at) = dependencies.node_index(&node.key) else {
                continue;
            };
            let own = |point| slot(Instant::new(at, point));
            for (rule, is_due) in [(&node.due_by, true), (&node.not_before, false)] {
                for source in rule.iter().flat_map(|rule| rule.sources.as_set()) {
                    let source = match source {
                        DateSource::CreatedAt => Some(created_at),
                        DateSource::Node(key) => date_slot(document, dependencies, key),
                    };
                    let Some(source) = source else {
                        continue;
                    };
                    links.push(if is_due {
                        (own(Point::Finish), source)
                    } else {
                        (source, own(Point::Start))
                    });
                }
            }
            if let Payload::Group(group) = &node.payload
                && group.closes
                && let Some(closes_at) = &group.closes_at
                && let Some(closing) = dependencies.node_index(closes_at)
                && matches!(
                    document.nodes.get(closes_at).map(Node::kind),
                    Some(NodeKind::Milestone)
                )
                && !tree.is_ancestor(&node.key, closes_at)
            {
                links.push((
                    own(Point::Finish),
                    slot(Instant::new(closing, Point::Finish)),
                ));
            }
            if let Payload::Decision(decision) = &node.payload
                && let AnswerSpec::Date {
                    feeds_milestone: Some(milestone),
                } = &decision.answer
                && let Some(pinned) = dependencies.node_index(milestone)
            {
                // The pin makes the answer and the milestone's date one: a chain runs both ways.
                let (answer, date) = (
                    at.get() * SLOTS + ANSWER,
                    slot(Instant::new(pinned, Point::Finish)),
                );
                links.extend([(answer, date), (date, answer)]);
            }
        }
        Self {
            slots: created_at + 1,
            links,
        }
    }

    /// Every slot with a chain from `start` (after) or to it (before), `start` included.
    fn reached(&self, start: usize, way: Way) -> Vec<bool> {
        let mut next: Vec<Vec<usize>> = vec![Vec::new(); self.slots];
        for &(from, to) in &self.links {
            let (key, value) = match way {
                Way::After => (from, to),
                Way::Before => (to, from),
            };
            if let Some(list) = next.get_mut(key) {
                list.push(value);
            }
        }
        let mut seen = vec![false; self.slots];
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

/// The slot a date rule's source names: a milestone's finish or a date decision's answer.
fn date_slot(document: &Document, dependencies: &Dependencies, source: &NodeKey) -> Option<usize> {
    let at = dependencies.node_index(source)?;
    match &document.nodes.get(source)?.payload {
        Payload::Milestone(_) => Some(slot(Instant::new(at, Point::Finish))),
        Payload::Decision(decision) if matches!(decision.answer, AnswerSpec::Date { .. }) => {
            Some(at.get() * SLOTS + ANSWER)
        }
        Payload::Decision(_) | Payload::Deliverable(_) | Payload::Action(_) | Payload::Group(_) => {
            None
        }
    }
}
