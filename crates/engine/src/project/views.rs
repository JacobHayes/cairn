//! C12's decision view, C13's timeline, and C18's status summary.
//!
//! The decision view is the level with only decisions shown (C2's rules decide its edges and
//! markers), with each decision's answer in effect and what it affects: the nodes whose
//! relevance its answer can change (a condition on the node or an ancestor reads it, up to
//! the nearest force include, or reads a decision it affects), the milestone it pins, and the role it fills.
//! The timeline has each in-scope milestone at its effective date (actual, pin, or derived
//! due), each other pinned node at its pin, and each other open node with a due date at it.
//! The status summary counts and lists over the in-scope nodes.
//!
//! Cost at `node_count_max`: the decision view is a level (see `level`) plus one pass over
//! every node's ancestors' conditions, O(nodes x depth x clauses), and the closure over the
//! decisions they read, O(decisions x nodes); the timeline and the summary
//! are a pass over the nodes and a sort, O(nodes log nodes).

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{
    AnswerSpec, Condition, DateOrigin, DecisionEntry, DecisionView, DisplayState, LevelQuery,
    NodeKey, NodeKind, OpenDecision, Payload, State, StatusSummary, Timeline, TimelineEntry,
    UpcomingMilestone,
};

use super::DerivedJourney;
use super::rows::count;
use crate::derive::forced;

impl DerivedJourney<'_> {
    /// C12: the decisions and their gating edges, with answers and what each answer affects.
    ///
    /// # Panics
    ///
    /// Never: the level of the whole journey names no container.
    #[must_use]
    pub fn decision_view(&self) -> DecisionView {
        let query = LevelQuery::of_kinds(BTreeSet::from([NodeKind::Decision]), None);
        let level = self
            .level_with(&query, None)
            .unwrap_or_else(|error| panic!("{error}"));
        let mut affects = self.conditioned_by();
        let decisions = level
            .nodes
            .into_iter()
            .map(|node| {
                let key = node.key;
                let (pins, fills) = match &self.node(&key).payload {
                    Payload::Decision(decision) => match &decision.answer {
                        AnswerSpec::Date { feeds_milestone } => (feeds_milestone.clone(), None),
                        AnswerSpec::Entity { fills_role }
                        | AnswerSpec::EntityList { fills_role } => (None, fills_role.clone()),
                        AnswerSpec::Boolean
                        | AnswerSpec::SingleChoice(_)
                        | AnswerSpec::MultiChoice(_)
                        | AnswerSpec::Text => (None, None),
                    },
                    Payload::Deliverable(_)
                    | Payload::Action(_)
                    | Payload::Milestone(_)
                    | Payload::Group(_) => (None, None),
                };
                DecisionEntry {
                    state: self.state(&key),
                    display_state: node.display_state,
                    relevance: self.derived.relevance().value(&key),
                    answer: self.answer(&key),
                    rationale: self.rationale(&key),
                    owners: self.owners(&key).clone(),
                    affects: affects.remove(&key).into_iter().flatten().collect(),
                    pins,
                    fills,
                    hidden_prerequisites: node.hidden_prerequisites,
                    node: key,
                }
            })
            .collect();
        DecisionView {
            decisions,
            edges: level.edges,
        }
    }

    /// C12: for each decision, the nodes whose relevance its answer can change: those whose
    /// own condition or an ancestor's reads it, up to the nearest force include, which drops
    /// its own and its ancestors' conditions (Gating); and, since a condition reads a decision
    /// that is not relevant as unanswered, everything the decisions it affects in turn affect.
    pub(super) fn conditioned_by(&self) -> BTreeMap<&NodeKey, BTreeSet<NodeKey>> {
        let document = self.graph.document();
        let tree = self.graph.tree();
        let mut direct: BTreeMap<&NodeKey, BTreeSet<&NodeKey>> = BTreeMap::new();
        for key in document.nodes.as_map().keys() {
            let mut current = Some(key);
            while let Some(above) = current.filter(|above| !forced(document, above)) {
                let condition = self.node(above).relevant_when.as_ref();
                for decision in condition.into_iter().flat_map(Condition::decisions) {
                    if decision != key {
                        direct.entry(decision).or_default().insert(key);
                    }
                }
                current = tree.parent(above);
            }
        }
        // The closure, a walk per decision: at most decisions x nodes steps.
        direct
            .keys()
            .map(|decision| {
                let mut reached: BTreeSet<NodeKey> = BTreeSet::new();
                let mut stack: Vec<&NodeKey> = vec![decision];
                while let Some(from) = stack.pop() {
                    for node in direct.get(from).into_iter().flatten() {
                        if node != decision && reached.insert((*node).clone()) {
                            stack.push(node);
                        }
                    }
                }
                (*decision, reached)
            })
            .collect()
    }

    /// C13: the in-scope milestones, pinned dates, and open work's due dates, earliest first,
    /// with overdue and shortfall marked and the `final` milestone as the end anchor.
    #[must_use]
    pub fn timeline(&self) -> Timeline {
        let derived = self.derived;
        let dates = derived.dates();
        let pins = &self.graph.document().state.pins;
        let mut entries = Vec::new();
        let mut undated = Vec::new();
        let mut end = None;
        for (key, node) in self.graph.document().nodes.as_map() {
            if !derived.relevance().in_scope(key) {
                continue;
            }
            let is_final =
                matches!(&node.payload, Payload::Milestone(milestone) if milestone.is_final);
            if is_final {
                end = Some(key.clone());
            }
            let placed = if node.kind() == NodeKind::Milestone {
                let effective = dates.effective_date(key);
                if effective.is_none() {
                    undated.push(key.clone());
                }
                effective.map(|effective| (effective.date, effective.origin))
            } else if let Some(pin) = pins.get(key) {
                Some((*pin, DateOrigin::Pin))
            } else if derived.blocking().closed(key) {
                None
            } else {
                dates.due(key).map(|due| (due, DateOrigin::Due))
            };
            if let Some((date, origin)) = placed {
                entries.push(TimelineEntry {
                    node: key.clone(),
                    kind: node.kind(),
                    date,
                    origin,
                    overdue: dates.overdue(key),
                    shortfall_days: dates.shortfall_days(key),
                    is_final,
                });
            }
        }
        entries.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.node.cmp(&b.node)));
        Timeline {
            entries,
            end,
            undated,
        }
    }

    /// C18: counts by state, what remains, the overdue, shortfall, and stale nodes, upcoming
    /// milestones with their effective dates, and open decisions with their owners.
    #[must_use]
    pub fn status_summary(&self) -> StatusSummary {
        let derived = self.derived;
        let in_scope: Vec<&NodeKey> = self
            .graph
            .document()
            .nodes
            .as_map()
            .keys()
            .filter(|key| derived.relevance().in_scope(key))
            .collect();
        let mut by_state: BTreeMap<State, u32> = BTreeMap::new();
        let mut by_display_state: BTreeMap<DisplayState, u32> = BTreeMap::new();
        for key in &in_scope {
            *by_state.entry(self.state(key)).or_default() += 1;
            let shown = derived.display_state(self.graph, key);
            *by_display_state.entry(shown).or_default() += 1;
        }
        let keep = |test: &dyn Fn(&NodeKey) -> bool| -> Vec<NodeKey> {
            in_scope
                .iter()
                .filter(|key| test(key))
                .map(|key| (*key).clone())
                .collect()
        };
        let open = |key: &NodeKey| !derived.blocking().closed(key);
        let mut upcoming: Vec<UpcomingMilestone> = in_scope
            .iter()
            .filter(|key| self.node(key).kind() == NodeKind::Milestone && open(key))
            .filter_map(|key| {
                let date = derived.dates().effective_date(key)?;
                Some(UpcomingMilestone {
                    node: (*key).clone(),
                    date,
                })
            })
            .collect();
        upcoming.sort_by(|a, b| {
            a.date
                .date
                .cmp(&b.date.date)
                .then_with(|| a.node.cmp(&b.node))
        });
        let decisions = keep(&|key| {
            self.node(key).kind() == NodeKind::Decision
                && self.state(key) == State::Open
                && !derived.blocking().closed(key)
        });
        StatusSummary {
            by_state,
            by_display_state,
            remaining: count(in_scope.iter().filter(|key| open(key)).count()),
            overdue: keep(&|key| derived.dates().overdue(key)),
            shortfalls: keep(&|key| derived.dates().shortfall_days(key).is_some()),
            stale: keep(&|key| derived.is_stale(key)),
            upcoming_milestones: upcoming,
            open_decisions: derived
                .ranking()
                .sorted(&decisions)
                .into_iter()
                .map(|node| OpenDecision {
                    owners: self.owners(&node).clone(),
                    node,
                })
                .collect(),
        }
    }
}
