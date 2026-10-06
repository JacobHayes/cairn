//! Pass 7, rank (PRD Priority; ARCHITECTURE, Read path: derive, pass 7; C5, C10): each node in
//! the normalization set (relevant or undecided, open, not a group) gets `urgency` and `late`
//! from its slack, `gravity_norm` and `leverage_norm` against the largest gravity and leverage
//! in that set (0 when the largest is 0), and their blend under the rank constants; the
//! frontier and the acting frontier are ordered by rank, then smaller slack (null last), then
//! greater gravity, then key. Rank is global: leverage's owner factor is relative to each
//! node's owner. "Prioritize for me" ([`super::Derived::rank_for`]) recomputes leverage with
//! the owner factor relative to the viewer, and the ranks and order from it, leaving the
//! global ranking unchanged. Effort-adjusted ordering sorts by gravity per estimated day,
//! nodes with no or a zero estimate last.
//!
//! Rank terms are floats: rank is never stored, and ties break on exact values and keys. With
//! the default constants, which sum to 1, a rank is in [0, 1]; in general it is at most the
//! constants' sum.
//!
//! Cost at `node_count_max`: two maxima and one blend per node, O(nodes), and sorting the
//! frontiers, O(nodes log nodes); a per-viewer ranking re-sums each ranked node's unblocked
//! targets (at most one entry per node in all, pass 6) and does the same.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use cairn_schema::{NodeKey, Payload, RankConstants, Score};

use super::blocking::Blocking;
use super::dates::Dates;
use super::priority::Priority;
use crate::graph::Graph;

/// Why a node ranks where it does (C10): rank and the terms it blends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankTerms {
    /// `clamp((horizon - slack) / horizon, 0, 1)`; 0 for null slack.
    pub urgency: f64,
    /// `clamp(-slack / horizon, 0, 1)`; 0 for null slack.
    pub late: f64,
    /// Gravity over the largest in the normalization set.
    pub gravity_norm: f64,
    /// Leverage over the largest in the normalization set.
    pub leverage_norm: f64,
    /// The blend of the four under the rank constants.
    pub rank: f64,
}

/// One ranked node: its terms and the signals its ties break on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ranked {
    terms: RankTerms,
    slack_days: Option<i32>,
    gravity: Score,
    leverage: Score,
}

/// Pass 7's output, globally or for one viewer: each ranked node's terms, and the frontier
/// and acting frontier in rank order.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranking {
    ranked: BTreeMap<NodeKey, Ranked>,
    frontier: Vec<NodeKey>,
    acting_frontier: Vec<NodeKey>,
}

impl Ranking {
    /// Ranks the normalization set, each node's leverage given by `leverage` (the global value
    /// or a viewer's).
    pub(crate) fn pass(
        priority: &Priority,
        dates: &Dates,
        blocking: &Blocking,
        constants: &RankConstants,
        leverage: impl Fn(&NodeKey) -> Score,
    ) -> Self {
        let set: Vec<(&NodeKey, Score, Score)> = blocking
            .keys()
            .iter()
            .filter(|key| priority.in_normalization_set(key))
            .map(|key| (key, priority.gravity(key), leverage(key)))
            .collect();
        let gravity_max = set.iter().map(|(_, gravity, _)| *gravity).max();
        let leverage_max = set.iter().map(|(_, _, leverage)| *leverage).max();
        let ranked: BTreeMap<NodeKey, Ranked> = set
            .into_iter()
            .map(|(key, gravity, leverage)| {
                let slack_days = dates.slack_days(key);
                let (urgency, late) = urgency(slack_days, constants.horizon_days);
                let gravity_norm = normalized(gravity, gravity_max.unwrap_or_default());
                let leverage_norm = normalized(leverage, leverage_max.unwrap_or_default());
                let rank = constants.urgency.get() * urgency
                    + constants.late.get() * late
                    + constants.gravity.get() * gravity_norm
                    + constants.leverage.get() * leverage_norm;
                let terms = RankTerms {
                    urgency,
                    late,
                    gravity_norm,
                    leverage_norm,
                    rank,
                };
                assert_terms(&terms, constants);
                let ranked = Ranked {
                    terms,
                    slack_days,
                    gravity,
                    leverage,
                };
                (key.clone(), ranked)
            })
            .collect();
        let mut ranking = Ranking {
            ranked,
            frontier: blocking.frontier().to_vec(),
            acting_frontier: blocking.acting_frontier().to_vec(),
        };
        ranking.frontier = ranking.sorted(&ranking.frontier);
        ranking.acting_frontier = ranking.sorted(&ranking.acting_frontier);
        ranking
    }

    /// The keys in rank order: rank, then smaller slack (null last), then greater gravity, then
    /// key; nodes not ranked after every ranked one, in key order.
    #[must_use]
    pub fn sorted(&self, keys: &[NodeKey]) -> Vec<NodeKey> {
        let mut sorted = keys.to_vec();
        sorted.sort_by(|a, b| self.compare(a, b));
        sorted
    }

    /// Priority's order between two nodes: rank descending, then smaller slack with null last,
    /// then greater gravity, then key; a node not ranked after a ranked one.
    #[must_use]
    pub fn compare(&self, a: &NodeKey, b: &NodeKey) -> Ordering {
        let order = match (self.ranked.get(a), self.ranked.get(b)) {
            (Some(first), Some(second)) => second
                .terms
                .rank
                .total_cmp(&first.terms.rank)
                .then_with(|| by_slack(first.slack_days, second.slack_days))
                .then_with(|| second.gravity.cmp(&first.gravity)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        order.then_with(|| a.cmp(b))
    }

    /// The node's rank and its terms, for nodes in the normalization set.
    #[must_use]
    pub fn terms(&self, key: &NodeKey) -> Option<RankTerms> {
        self.ranked.get(key).map(|ranked| ranked.terms)
    }

    /// The node's rank, for nodes in the normalization set.
    #[must_use]
    pub fn rank(&self, key: &NodeKey) -> Option<f64> {
        self.terms(key).map(|terms| terms.rank)
    }

    /// The leverage the rank read: the global value, or the viewer's in a per-viewer ranking.
    #[must_use]
    pub fn leverage(&self, key: &NodeKey) -> Option<Score> {
        self.ranked.get(key).map(|ranked| ranked.leverage)
    }

    /// PRD Frontier, in rank order.
    #[must_use]
    pub fn frontier(&self) -> &[NodeKey] {
        &self.frontier
    }

    /// PRD Acting frontier, in rank order (C10's next list).
    #[must_use]
    pub fn acting_frontier(&self) -> &[NodeKey] {
        &self.acting_frontier
    }

    /// Priority, Effort-adjusted: the keys by gravity per estimated day, greatest first; nodes
    /// with no or a zero estimate last; ties in rank order. `graph` is the one derived.
    #[must_use]
    pub fn by_effort(&self, graph: &Graph, priority: &Priority, keys: &[NodeKey]) -> Vec<NodeKey> {
        let estimate = |key: &NodeKey| {
            let days = graph.node(key).and_then(|node| match &node.payload {
                Payload::Deliverable(deliverable) => deliverable.estimate,
                Payload::Action(action) => action.estimate,
                Payload::Decision(_) | Payload::Milestone(_) | Payload::Group(_) => None,
            });
            days.map(|days| u128::from(days.get()))
                .filter(|days| *days > 0)
        };
        let mut sorted = keys.to_vec();
        sorted.sort_by(|a, b| {
            let per_day = match (estimate(a), estimate(b)) {
                // a / ea against b / eb, exactly: a x eb against b x ea.
                (Some(first), Some(second)) => {
                    let a_gravity = u128::from(priority.gravity(a).millionths());
                    let b_gravity = u128::from(priority.gravity(b).millionths());
                    (b_gravity * first).cmp(&(a_gravity * second))
                }
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            per_day.then_with(|| self.compare(a, b))
        });
        sorted
    }
}

/// Smaller slack first, null last.
fn by_slack(first: Option<i32>, second: Option<i32>) -> Ordering {
    match (first, second) {
        (Some(first), Some(second)) => first.cmp(&second),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Priority, Rank: `urgency` and `late` from slack over the horizon; both 0 for null slack. A
/// zero horizon is the limit of a shrinking one: urgency 1 from slack 0, late 1 below it.
fn urgency(slack_days: Option<i32>, horizon_days: u32) -> (f64, f64) {
    let Some(slack) = slack_days else {
        return (0.0, 0.0);
    };
    if horizon_days == 0 {
        let urgency = if slack <= 0 { 1.0 } else { 0.0 };
        let late = if slack < 0 { 1.0 } else { 0.0 };
        return (urgency, late);
    }
    let horizon = f64::from(horizon_days);
    let slack = f64::from(slack);
    (
        ((horizon - slack) / horizon).clamp(0.0, 1.0),
        (-slack / horizon).clamp(0.0, 1.0),
    )
}

/// `value / max`, or 0 when the max is 0.
fn normalized(value: Score, max: Score) -> f64 {
    assert!(value <= max, "the max is over the normalization set");
    if max == Score::default() {
        return 0.0;
    }
    value.value() / max.value()
}

/// Each term is in [0, 1], and the rank within the constants' sum.
fn assert_terms(terms: &RankTerms, constants: &RankConstants) {
    for term in [
        terms.urgency,
        terms.late,
        terms.gravity_norm,
        terms.leverage_norm,
    ] {
        assert!((0.0..=1.0).contains(&term), "a rank term is in [0, 1]");
    }
    let sum = constants.urgency.get()
        + constants.late.get()
        + constants.gravity.get()
        + constants.leverage.get();
    assert!(
        terms.rank.is_finite() && terms.rank >= 0.0 && terms.rank <= sum * (1.0 + 1e-12),
        "a rank is within the constants' sum"
    );
}
