//! Chains as people read them (F3, F5, F7): constraint positions resolved to their sources,
//! with a container's internal entries collapsed into the one dependency they carry (an
//! ancestor's requirement, condition, or stage opening), the dates fixed on the chain, and
//! for a short chain the resolution moves as ordinary mutations.

use cairn_schema::limits::OFFSET_DAYS_MAX;
use cairn_schema::{
    AnswerValue, Chain, Constraint as ShownConstraint, ConstraintSource, DateRule, Days,
    DependencyVia, Direction as RuleDirection, Edge as ExplicitEdge, FixedBy, FixedDate,
    Instant as ShownInstant, InstantPoint, Mutation, NodeFieldValue, NodeKey, ShortChain,
    SignedDays,
};

use super::network::{Constraint, Network, Origin, Place, Rule, Slot, estimate_days};
use super::{Fix, Seeds, checked_date, date};
use crate::derive::dependencies::{EdgeSource, NodeIndex, Point};
use crate::graph::Document;

/// What moves a chain's end: a direct pin, or a date decision's answer (its own, or the one
/// that pins a milestone, E3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mover {
    Pin(NodeIndex),
    Answer(NodeIndex),
}

/// What a chain is read against.
pub(crate) struct Explain<'a> {
    pub document: &'a Document,
    pub keys: &'a [NodeKey],
    pub network: &'a Network,
    pub seeds: &'a Seeds,
}

/// A chain found by a solve: constraint positions in order, with the slot whose seed starts
/// it (a lower bound) and the slot whose seed ends it (an upper bound), when it has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Found {
    /// Constraint positions along the chain.
    pub constraints: Vec<u32>,
    /// The slot whose lower seed the chain starts from.
    pub from: Option<Slot>,
    /// The slot whose upper seed the chain ends at.
    pub to: Option<Slot>,
    /// Days short, when the chain is a shortfall or a contradiction.
    pub shortfall_days: i32,
    /// What else holds the start slot back (its earliest bound without its own seed) and
    /// caps the end slot (its latest without its own), in days: a pin moved past these no
    /// longer gives the chain its days.
    pub others: (Option<i32>, Option<i32>),
}

impl Explain<'_> {
    fn key(&self, node: NodeIndex) -> NodeKey {
        let key = self.keys.get(node.get());
        assert!(key.is_some(), "every slot's node is in the graph");
        key.cloned()
            .unwrap_or_else(|| unreachable!("asserted above"))
    }

    /// The slot as people see it. An entry never reaches here.
    pub(crate) fn instant(&self, slot: Slot) -> ShownInstant {
        match self.network.place(slot) {
            Place::Node(instant) => {
                let point = match instant.point {
                    Point::Start => InstantPoint::Start,
                    Point::Finish => InstantPoint::Finish,
                    Point::Entry | Point::ConditionEntry => {
                        unreachable!("entries are collapsed out of chains")
                    }
                };
                ShownInstant::Node {
                    node: self.key(instant.node),
                    point,
                }
            }
            Place::CreatedAt => ShownInstant::CreatedAt,
            Place::Answer(decision) => ShownInstant::Answer {
                decision: self.key(decision),
            },
        }
    }

    fn constraint(&self, at: u32) -> &Constraint {
        let found = self.network.constraint(at);
        assert!(found.is_some(), "a chain names constraints of its network");
        found.unwrap_or_else(|| unreachable!("asserted above"))
    }

    /// The constraints along a chain as shown: each entry hop folded into the dependency it
    /// carries, from the requirement's finish to the node that waits for it.
    pub(crate) fn shown(&self, constraints: &[u32]) -> Vec<ShownConstraint> {
        let mut shown = Vec::new();
        let mut folding: Option<(&Constraint, i32, bool)> = None;
        for &at in constraints {
            let constraint = self.constraint(at);
            let (first, offset, conditional) = match folding.take() {
                Some((first, offset, conditional)) => (
                    first,
                    offset.saturating_add(constraint.offset_days),
                    conditional || constraint.conditional,
                ),
                None => (constraint, constraint.offset_days, constraint.conditional),
            };
            if !self.network.shown(constraint.after) {
                folding = Some((first, offset, conditional));
                continue;
            }
            shown.push(ShownConstraint {
                before: self.instant(first.before),
                after: self.instant(constraint.after),
                offset_days: offset,
                source: self.source(first, constraint.after),
                conditional,
            });
        }
        assert!(folding.is_none(), "a chain ends at a shown instant");
        shown
    }

    /// Where a constraint comes from; for a folded entry hop, `first` is the hop's first
    /// constraint and `to` the instant it ends at.
    fn source(&self, first: &Constraint, to: Slot) -> ConstraintSource {
        let waiting = match self.network.place(to) {
            Place::Node(instant) => Some(instant.node),
            Place::CreatedAt | Place::Answer(_) => None,
        };
        match first.origin {
            Origin::Rule {
                node,
                rule: Rule::DueBy,
            } => ConstraintSource::DueBy {
                node: self.key(node),
            },
            Origin::Rule {
                node,
                rule: Rule::NotBefore,
            } => ConstraintSource::NotBefore {
                node: self.key(node),
            },
            Origin::StageClose { group } => ConstraintSource::StageClose {
                group: self.key(group),
            },
            Origin::Dependency(edge) => {
                let owner = self.key(edge.dependent.node);
                let required = self.key(edge.requirement.node);
                let node = waiting.map_or_else(|| owner.clone(), |at| self.key(at));
                let via = match edge.source {
                    EdgeSource::Work => return ConstraintSource::Estimate { node: owner },
                    EdgeSource::Containment => {
                        return ConstraintSource::Containment {
                            parent: owner,
                            child: required,
                        };
                    }
                    EdgeSource::Explicit if owner == node => DependencyVia::Explicit,
                    EdgeSource::Explicit | EdgeSource::Entry | EdgeSource::Chain => {
                        DependencyVia::Inherited { ancestor: owner }
                    }
                    EdgeSource::Condition => DependencyVia::Condition {
                        condition_on: owner,
                    },
                    EdgeSource::StageOpening => DependencyVia::StageOpening { group: owner },
                };
                ConstraintSource::Dependency {
                    node,
                    requires: required,
                    via,
                }
            }
        }
    }

    /// A fixed date as shown.
    fn fixed(&self, slot: Slot, fix: Fix) -> FixedDate {
        FixedDate {
            instant: self.instant(slot),
            date: date(fix.day),
            fixed_by: fix.by,
        }
    }

    /// The chain as shown, with the seeds that start and end it and any pins on the way.
    pub(crate) fn chain(&self, found: &Found) -> Chain {
        let mut fixed = Vec::new();
        if let Some(from) = found.from
            && let Some(fix) = self.seeds.lower(from)
        {
            fixed.push(self.fixed(from, fix));
        }
        for &at in &found.constraints {
            let after = self.constraint(at).after;
            let last = found.constraints.last() == Some(&at);
            let ends = last && found.to == Some(after);
            if let Some(pin) = self
                .seeds
                .pin(after)
                .filter(|_| !ends && self.network.shown(after))
            {
                let shown = self.fixed(after, pin);
                if !fixed.contains(&shown) {
                    fixed.push(shown);
                }
            }
        }
        if let Some(to) = found.to
            && let Some(fix) = self.seeds.upper(to)
        {
            let shown = self.fixed(to, fix);
            if !fixed.contains(&shown) {
                fixed.push(shown);
            }
        }
        Chain {
            constraints: self.shown(&found.constraints),
            fixed,
        }
    }

    /// A chain that is short of days, with its resolution moves (F5, F6).
    pub(crate) fn short(&self, found: &Found) -> ShortChain {
        assert!(found.shortfall_days > 0, "a short chain lacks days");
        ShortChain {
            chain: self.chain(found),
            shortfall_days: found.shortfall_days.unsigned_abs(),
            resolutions: self.resolutions(found),
        }
    }

    /// F5: each move that gives the chain the days it lacks, alone. A pin or answer that
    /// starts the chain moves earlier; one that ends it moves later; a rule's offset or an
    /// estimate shrinks; an explicit requirement or a stage close is dropped. Actuals and
    /// today never move.
    fn resolutions(&self, found: &Found) -> Vec<Mutation> {
        let days = found.shortfall_days;
        let start = found
            .from
            .and_then(|slot| self.mover(slot, -days, found.others.0));
        let end = found
            .to
            .and_then(|slot| self.mover(slot, days, found.others.1));
        let mut moves = Vec::new();
        // One answer fixing both ends moves both: the gap stays.
        let same = start.is_some() && start.map(|(by, _)| by) == end.map(|(by, _)| by);
        for (mover, shift) in [(start, -days), (end, days)] {
            if let Some((by, fix)) = mover.filter(|_| !same) {
                self.seed_moves(by, fix, shift, &mut moves);
            }
        }
        for &at in &found.constraints {
            for found_move in self.constraint_moves(at, days) {
                if !moves.contains(&found_move) {
                    moves.push(found_move);
                }
            }
        }
        moves
    }

    /// What can move the seed that holds a chain's end, when it is a pin or an answer that
    /// moving by `days` (earlier at the start, later at the end) gives the chain its days:
    /// not today or an actual, and not a pin that today or another chain (`other`, what else
    /// holds the start back or caps the end) would still hold once moved, nor one moved past
    /// the dates there are.
    fn mover(&self, slot: Slot, days: i32, other: Option<i32>) -> Option<(Mover, Fix)> {
        let start = days < 0;
        let fix = if start {
            self.seeds.lower(slot)?
        } else {
            self.seeds.upper(slot)?
        };
        if !matches!(fix.by, FixedBy::Pin | FixedBy::Answer) {
            return None;
        }
        let target = fix.day.checked_add(days)?;
        checked_date(target)?;
        let floor = self.seeds.floor(slot).map(|today| today.day);
        let held = if start {
            floor.max(other).is_some_and(|held| held > target)
        } else {
            other.is_some_and(|held| held < target)
        };
        if held {
            return None;
        }
        let mover = match self.network.place(slot) {
            Place::Answer(decision) => Mover::Answer(decision),
            Place::Node(instant) => self
                .seeds
                .feeding(instant.node)
                .map_or(Mover::Pin(instant.node), Mover::Answer),
            Place::CreatedAt => return None,
        };
        Some((mover, fix))
    }

    /// Moving a pin or answer by `days`: shift or clear a direct pin; answer a date decision
    /// (or the one feeding a pin) with the moved date.
    fn seed_moves(&self, by: Mover, fix: Fix, days: i32, moves: &mut Vec<Mutation>) {
        let shifted = date(fix.day.saturating_add(days));
        match by {
            Mover::Answer(decision) => moves.push(Mutation::Answer {
                decision: self.key(decision),
                value: AnswerValue::Date(shifted),
            }),
            Mover::Pin(node) => {
                let node = self.key(node);
                if let Ok(offset_days) = SignedDays::try_from(days) {
                    moves.push(Mutation::ShiftPin {
                        node: node.clone(),
                        offset_days,
                    });
                }
                moves.push(Mutation::ClearPin { node });
            }
        }
    }

    /// The moves one constraint offers.
    fn constraint_moves(&self, at: u32, days: i32) -> Vec<Mutation> {
        let constraint = self.constraint(at);
        match constraint.origin {
            Origin::Rule { node, rule } => self.rule_move(node, rule, days).into_iter().collect(),
            Origin::StageClose { group } => vec![Mutation::SetNodeField {
                node: self.key(group),
                value: NodeFieldValue::Closes(false),
            }],
            Origin::Dependency(edge) => match edge.source {
                EdgeSource::Work => {
                    let node = self.key(edge.dependent.node);
                    let estimate = self.document.nodes.get(&node).map_or(0, estimate_days);
                    let left = estimate - days;
                    let revised = u32::try_from(left)
                        .ok()
                        .and_then(|left| Days::try_from(left).ok());
                    match revised {
                        Some(revised) if constraint.offset_days >= days => {
                            vec![Mutation::SetNodeField {
                                node,
                                value: NodeFieldValue::Estimate(Some(revised)),
                            }]
                        }
                        Some(_) | None => Vec::new(),
                    }
                }
                EdgeSource::Explicit => vec![Mutation::RemoveEdge {
                    edge: ExplicitEdge {
                        node: self.key(edge.dependent.node),
                        requires: self.key(edge.requirement.node),
                    },
                }],
                EdgeSource::Entry
                | EdgeSource::Chain
                | EdgeSource::Containment
                | EdgeSource::Condition
                | EdgeSource::StageOpening => Vec::new(),
            },
        }
    }

    /// A rule loosened by `days`: a `due_by` moves its cap later, a `not_before` its floor
    /// earlier, its direction flipping where the offset passes zero.
    fn rule_move(&self, node: NodeIndex, rule: Rule, days: i32) -> Option<Mutation> {
        let key = self.key(node);
        let found = self.document.nodes.get(&key)?;
        let current = match rule {
            Rule::DueBy => found.due_by.as_ref()?,
            Rule::NotBefore => found.not_before.as_ref()?,
        };
        let signed = super::network::signed_offset_days(current);
        let loosened = match rule {
            Rule::DueBy => signed.checked_add(days)?,
            Rule::NotBefore => signed.checked_sub(days)?,
        };
        if loosened.unsigned_abs() > OFFSET_DAYS_MAX {
            return None;
        }
        let direction = if loosened >= 0 {
            RuleDirection::After
        } else {
            RuleDirection::Before
        };
        let revised = DateRule {
            direction,
            sources: current.sources.clone(),
            offset: Days::try_from(loosened.unsigned_abs()).ok()?,
        };
        let value = match rule {
            Rule::DueBy => NodeFieldValue::DueBy(Some(revised)),
            Rule::NotBefore => NodeFieldValue::NotBefore(Some(revised)),
        };
        Some(Mutation::SetNodeField { node: key, value })
    }
}
