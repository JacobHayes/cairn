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
    AnswerSpec, Condition, Date, DateOrigin, DateSource, DecisionEntry, DecisionView, DisplayState,
    Instant, KeyRefs, LevelQuery, Node, NodeKey, NodeKind, OpenDecision, Payload, Relevance,
    RouteSchedule, ScheduleEntry, StatusSummary, Timeline, TimelineEntry, UndatedEntry,
    UpcomingMilestone, When,
};

use super::DerivedJourney;
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
            .level_with(&query, None, &BTreeSet::new())
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
                        AnswerSpec::Identity { fills_role }
                        | AnswerSpec::IdentityList { fills_role } => (None, fills_role.clone()),
                        AnswerSpec::Boolean
                        | AnswerSpec::SingleChoice(_)
                        | AnswerSpec::MultiChoice(_)
                        | AnswerSpec::Text => (None, None),
                    },
                    Payload::Deliverable(_)
                    | Payload::Action(_)
                    | Payload::Milestone(_)
                    | Payload::Group => (None, None),
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
        let end = derived.final_milestone(self.graph);
        for (key, node) in self.graph.document().nodes.as_map() {
            if !derived.relevance().in_scope(key) {
                continue;
            }
            let is_final = end.as_ref() == Some(key);
            let placed = if node.kind() == NodeKind::Milestone {
                let effective = dates.effective_date(key);
                if effective.is_none() {
                    undated.push(key.clone());
                }
                effective.map(|effective| (effective.date, effective.origin))
            } else if let Some(pin) = pins.get(key) {
                Some((*pin, DateOrigin::Pin))
            } else if !derived.open(key) {
                None
            } else {
                dates.due(key).map(|due| (When::Day(due), DateOrigin::Due))
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

    /// C13: a route's schedule: the journey derived as one that starts on `start` (its
    /// `created_on` and its today), with each in-scope node's earliest start and finish as
    /// days from `start`. A node no bound reaches is `undated`; one whose date rules read a
    /// date decision nothing answers yet says which it waits on (`waiting`). `final_conditional`
    /// is set when the final milestone's earliest day passes through an undecided node.
    ///
    /// Cost: a pass over the nodes and one chain walk for the final milestone,
    /// O(nodes log nodes).
    #[must_use]
    pub fn schedule(&self, start: Date) -> RouteSchedule {
        let derived = self.derived;
        let dates = derived.dates();
        let document = self.graph.document();
        let offset = |date: Date| date.since(start).map_or(0, |span| span.get_days());
        let mut entries = Vec::new();
        let mut undated = Vec::new();
        let end = derived.final_milestone(self.graph);
        for (key, node) in document.nodes.as_map() {
            if !derived.relevance().in_scope(key) {
                continue;
            }
            let waiting = self.waiting_on(node);
            let Some(earliest) = dates.earliest_start(key) else {
                undated.push(UndatedEntry {
                    node: key.clone(),
                    waiting,
                });
                continue;
            };
            let first = offset(earliest);
            let last = dates.earliest_finish(key).map_or(first, offset).max(first);
            entries.push(ScheduleEntry {
                node: key.clone(),
                kind: node.kind(),
                start_day: first,
                finish_day: last,
                conditional: derived.relevance().value(key) == Relevance::Undecided,
                waiting,
            });
        }
        entries.sort_by(|a, b| {
            a.start_day
                .cmp(&b.start_day)
                .then_with(|| a.node.cmp(&b.node))
        });
        let final_conditional = end.as_ref().is_some_and(|key| self.passes_undecided(key));
        RouteSchedule {
            entries,
            end,
            final_conditional,
            undated,
        }
    }

    /// The date decisions `node`'s date rules read that no answer settles yet, by key.
    fn waiting_on(&self, node: &Node<KeyRefs>) -> Vec<NodeKey> {
        let document = self.graph.document();
        [node.due_by.as_ref(), node.not_before.as_ref()]
            .into_iter()
            .flatten()
            .flat_map(|rule| rule.sources.as_set())
            .filter_map(|source| match source {
                DateSource::Node(decision)
                    if self.graph.node(decision).is_some_and(|found| {
                        found.kind() == NodeKind::Decision
                            && !document.state.answers.contains_key(decision)
                    }) =>
                {
                    Some(decision.clone())
                }
                DateSource::Node(_) | DateSource::CreatedAt => None,
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Whether the earliest start of `key` rests on an undecided node or a conditional constraint.
    fn passes_undecided(&self, key: &NodeKey) -> bool {
        let derived = self.derived;
        derived
            .dates()
            .node(self.graph, key)
            .earliest_start
            .is_some_and(|bound| {
                bound.chain.constraints.iter().any(|constraint| {
                    constraint.conditional
                        || [&constraint.before, &constraint.after]
                            .into_iter()
                            .any(|instant| {
                                matches!(instant, Instant::Node { node, .. }
                                    if derived.relevance().value(node) == Relevance::Undecided)
                            })
                })
            })
    }

    /// C18: in-scope nodes with work left on them.
    fn remaining(&self) -> u32 {
        let derived = self.derived;
        let count = self
            .graph
            .document()
            .nodes
            .as_map()
            .keys()
            .filter(|key| derived.open(key))
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// B11: the journey has a node and none in scope is left open, or its final milestone
    /// is done (stored or done by its date, F1). Completing is the person's call.
    #[must_use]
    pub fn completion_suggested(&self) -> bool {
        if self.graph.document().nodes.is_empty() {
            return false;
        }
        self.remaining() == 0
            || self.derived.final_milestone(self.graph).is_some_and(|key| {
                self.derived.display_state(self.graph, &key) == DisplayState::Done
            })
    }

    /// C18: counts by state, what remains, the overdue, shortfall, and stale nodes, upcoming
    /// milestones with their effective dates, and open decisions with their owners.
    #[must_use]
    pub fn status_summary(&self) -> StatusSummary {
        let derived = self.derived;
        let in_scope = self.in_scope();
        let (by_state, by_display_state) = self.state_counts(&in_scope);
        let keep = |test: &dyn Fn(&NodeKey) -> bool| -> Vec<NodeKey> {
            in_scope
                .iter()
                .filter(|key| test(key))
                .map(|key| (*key).clone())
                .collect()
        };
        let open = |key: &NodeKey| derived.open(key);
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
        let decisions = keep(&|key| self.open_decision(key));
        StatusSummary {
            by_state,
            by_display_state,
            remaining: self.remaining(),
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
            completion_suggested: self.completion_suggested(),
        }
    }
}
