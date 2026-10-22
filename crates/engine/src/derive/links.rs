//! The date relationships a graph's authors wrote (PRD C1, C15): a node's due-by and
//! not-before rules, a stage's close, a stage opening that holds dates only, and a date
//! decision that sets a milestone. They are not edges: nothing waits on them. The canvas draws
//! them for the selected node, and the node's dates list them.
//!
//! The rules and closes are read from the date network's retained constraints (their origin
//! and ends; the network already left out whatever relevance ruled out), the openings from the
//! pruned dependency edges, and the fed dates from the document with both ends in scope. The
//! rule's direction and days come from the node's own rule, so a zero offset reads as authored.
//!
//! Cost: one pass over the retained constraints (bounded by the cost test of the date
//! network), one over the pruned edges, one over the nodes; the result is sorted once.

use std::collections::BTreeMap;

use cairn_schema::{
    AnswerSpec, DateLink, DateLinkKind, DateRuleKind, DateSource, LevelDateLink, NodeKey, NodeKind,
    Payload,
};

use super::Derived;
use super::dates::network::{Constraint, Origin, Place, Rule};
use super::dependencies::{EdgeClass, EdgeSet, EdgeSource, NodeIndex};
use crate::graph::Graph;

/// One authored date relationship with its ends: `from` and `to` are the earlier and later end
/// where `orders`, else the source and the node whose dates they are.
pub(crate) struct Authored<'a> {
    pub node: &'a NodeKey,
    pub source: &'a NodeKey,
    pub kind: DateLinkKind,
    pub from: &'a NodeKey,
    pub to: &'a NodeKey,
    /// The relationship fixes the order: `to` can never come before `from`.
    pub orders: bool,
}

impl Authored<'_> {
    /// The schema value, without its ends.
    pub(crate) fn link(&self) -> DateLink {
        DateLink {
            node: self.node.clone(),
            source: self.source.clone(),
            kind: self.kind.clone(),
        }
    }
}

impl Derived {
    /// Every authored date relationship of `graph`, the graph derived, in no order.
    pub(crate) fn authored_links<'a>(&'a self, graph: &'a Graph) -> Vec<Authored<'a>> {
        let mut found = Vec::new();
        for constraint in self.dates.network().constraints() {
            let link = match constraint.origin {
                Origin::Rule { node, rule } => self.rule_link(graph, constraint, node, rule),
                Origin::StageClose { group } => self.close_link(constraint, group),
                Origin::Dependency(_) => None,
            };
            found.extend(link);
        }
        found.extend(self.opening_links());
        found.extend(self.fed_links(graph));
        found
    }

    /// A due-by or not-before rule's relationship, from the constraint the network holds it as.
    fn rule_link<'a>(
        &'a self,
        graph: &'a Graph,
        constraint: &Constraint,
        node: NodeIndex,
        rule: Rule,
    ) -> Option<Authored<'a>> {
        let dependencies = &self.dependencies;
        let network = self.dates.network();
        let node_key = dependencies.key(node)?;
        let rule_node = graph.document().nodes.get(node_key)?;
        let authored = match rule {
            Rule::DueBy => rule_node.due_by.as_ref(),
            Rule::NotBefore => rule_node.not_before.as_ref(),
        }?;
        // The source is the end the rule's node is not on, whatever instant of the node the
        // network holds the rule at.
        let source_slot = match rule {
            Rule::DueBy => constraint.after,
            Rule::NotBefore => constraint.before,
        };
        let (source, answer) = match network.place(source_slot) {
            Place::Node(instant) => (instant.node, false),
            Place::Answer(decision) => (decision, true),
            Place::CreatedAt => return None,
        };
        let source_key = dependencies.key(source)?;
        debug_assert!(
            authored.sources.as_set().iter().any(|found| matches!(
                found, DateSource::Node(key) if key == source_key
            )),
            "the constraint's source is one of the rule's"
        );
        let orders = constraint.offset_days >= 0 && !answer;
        let (from, to) = match (orders, rule) {
            (true, Rule::DueBy) => (node_key, source_key),
            (true, Rule::NotBefore) | (false, _) => (source_key, node_key),
        };
        Some(Authored {
            node: node_key,
            source: source_key,
            kind: DateLinkKind::Rule {
                rule: match rule {
                    Rule::DueBy => DateRuleKind::DueBy,
                    Rule::NotBefore => DateRuleKind::NotBefore,
                },
                direction: authored.direction,
                days: authored.offset.get(),
                answer,
            },
            from,
            to,
            orders,
        })
    }

    /// A stage's close: from the stage to the milestone that closes it.
    fn close_link(&self, constraint: &Constraint, group: NodeIndex) -> Option<Authored<'_>> {
        let Place::Node(closing) = self.dates.network().place(constraint.after) else {
            return None;
        };
        let stage = self.dependencies.key(group)?;
        let milestone = self.dependencies.key(closing.node)?;
        Some(Authored {
            node: stage,
            source: milestone,
            kind: DateLinkKind::StageClose,
            from: stage,
            to: milestone,
            orders: true,
        })
    }

    /// The stage openings that gate nothing: from the opening milestone to the stage.
    fn opening_links(&self) -> impl Iterator<Item = Authored<'_>> {
        let dependencies = &self.dependencies;
        dependencies
            .edges(EdgeSet::Pruned)
            .filter(|edge| {
                edge.source == EdgeSource::StageOpening && edge.class == EdgeClass::DateOnly
            })
            .filter_map(|edge| {
                let milestone = dependencies.key(edge.requirement.node)?;
                let stage = dependencies.key(edge.dependent.node)?;
                Some(Authored {
                    node: stage,
                    source: milestone,
                    kind: DateLinkKind::StageOpening,
                    from: milestone,
                    to: stage,
                    orders: true,
                })
            })
    }

    /// The date decisions that set a milestone's date, both ends in scope.
    fn fed_links<'a>(&'a self, graph: &'a Graph) -> impl Iterator<Item = Authored<'a>> {
        let document = graph.document();
        let in_scope = |key: &NodeKey| self.relevance.in_scope(key);
        document.nodes.values().filter_map(move |node| {
            let Payload::Decision(decision) = &node.payload else {
                return None;
            };
            let AnswerSpec::Date {
                feeds_milestone: Some(milestone),
            } = &decision.answer
            else {
                return None;
            };
            let is_milestone = document
                .nodes
                .get(milestone)
                .is_some_and(|found| found.kind() == NodeKind::Milestone);
            (is_milestone && in_scope(&node.key) && in_scope(milestone)).then_some(Authored {
                node: &node.key,
                source: milestone,
                kind: DateLinkKind::FedDate,
                from: milestone,
                to: &node.key,
                orders: false,
            })
        })
    }

    /// The node's direct date relationships, in both directions, sorted.
    pub(crate) fn date_links_of(&self, graph: &Graph, key: &NodeKey) -> Vec<DateLink> {
        let mut links: Vec<DateLink> = self
            .authored_links(graph)
            .iter()
            .filter(|found| found.node == key || found.source == key)
            .map(Authored::link)
            .collect();
        links.sort();
        links.dedup();
        links
    }
}

/// Every node's direct date relationships, from one enumeration: for the projection of all
/// nodes.
pub(crate) fn date_links_by_node(authored: &[Authored<'_>]) -> BTreeMap<NodeKey, Vec<DateLink>> {
    let mut by_node: BTreeMap<NodeKey, Vec<DateLink>> = BTreeMap::new();
    for found in authored {
        let link = found.link();
        by_node
            .entry(found.node.clone())
            .or_default()
            .push(link.clone());
        by_node.entry(found.source.clone()).or_default().push(link);
    }
    for links in by_node.values_mut() {
        links.sort();
        links.dedup();
    }
    by_node
}

/// The authored relationships re-targeted to the stand-ins of their ends: ends with no
/// stand-in, or both on one node, are dropped. Ordering relationships collapse per ordered
/// pair; the others join a pair they share with another relationship, in either direction,
/// else make their own.
pub(crate) fn collapse<'a>(
    authored: &[Authored<'a>],
    stand_in: impl Fn(&NodeKey) -> Option<&'a NodeKey>,
) -> Vec<LevelDateLink> {
    type Pair<'k> = (&'k NodeKey, &'k NodeKey);
    let mut groups: BTreeMap<Pair<'a>, (bool, Vec<DateLink>)> = BTreeMap::new();
    let ends = |found: &Authored<'a>| -> Option<Pair<'a>> {
        let (from, to) = (stand_in(found.from)?, stand_in(found.to)?);
        (from != to).then_some((from, to))
    };
    for found in authored.iter().filter(|found| found.orders) {
        if let Some(pair) = ends(found) {
            let group = groups.entry(pair).or_default();
            group.0 = true;
            group.1.push(found.link());
        }
    }
    for found in authored.iter().filter(|found| !found.orders) {
        let Some((from, to)) = ends(found) else {
            continue;
        };
        let pair = [(from, to), (to, from)]
            .into_iter()
            .find(|pair| groups.contains_key(pair))
            .unwrap_or((from, to));
        groups.entry(pair).or_default().1.push(found.link());
    }
    groups
        .into_iter()
        .map(|((from, to), (orders, mut links))| {
            links.sort();
            links.dedup();
            assert_ne!(links, [] as [cairn_schema::DateLink; 0]);
            LevelDateLink {
                from: from.clone(),
                to: to.clone(),
                orders,
                links,
            }
        })
        .collect()
}
