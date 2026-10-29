//! Pass 7, rank (PRD Priority; ARCHITECTURE, Read path: derive, pass 7; C5, C10): each node in
//! the normalization set (relevant or undecided, open, not a group) gets `urgency` and `late`
//! from its slack, `gravity_norm` and `unlocks_norm` against the largest gravity and unlocks
//! in that set (0 when the largest is 0), and their blend under the fixed rank constants; the
//! frontier and the acting frontier are ordered by rank, then smaller slack (null last), then
//! greater gravity, then key. Rank is global: the owner factor of unlocks is relative to each
//! node's owner.
//!
//! Rank terms are floats: rank is never stored, and ties break on exact values and keys. The
//! constants sum to 1, so a rank is in [0, 1].
//!
//! Cost at `node_count_max`: two maxima and one blend per node, O(nodes), and sorting the
//! frontiers, O(nodes log nodes).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use cairn_schema::{NodeKey, Score};

use super::blocking::Blocking;
use super::dates::Dates;
use super::priority::Priority;

/// The rank constants (Priority), fixed: the blend's weights, which sum to 1, and the days of
/// slack over which urgency rises from 0 to 1.
pub struct RankConstants {
    /// Urgency's weight.
    pub urgency: f64,
    /// Lateness's weight.
    pub late: f64,
    /// Normalized gravity's weight.
    pub gravity: f64,
    /// Normalized unlocks' weight.
    pub unlocks: f64,
    /// Days of slack over which urgency rises from 0 to 1.
    pub horizon_days: u32,
}

/// The rank constants.
pub const RANK: RankConstants = RankConstants {
    urgency: 0.40,
    late: 0.15,
    gravity: 0.25,
    unlocks: 0.20,
    horizon_days: 14,
};

/// Why a node ranks where it does (C10): rank and the terms it blends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankTerms {
    /// `clamp((horizon - slack) / horizon, 0, 1)`; 0 for null slack.
    pub urgency: f64,
    /// `clamp(-slack / horizon, 0, 1)`; 0 for null slack.
    pub late: f64,
    /// Gravity over the largest in the normalization set.
    pub gravity_norm: f64,
    /// Unlocks over the largest in the normalization set.
    pub unlocks_norm: f64,
    /// What each term adds to the rank: the term times its constant.
    pub adds: RankAdds,
    /// The blend of the four under the rank constants: the sum of what they add.
    pub rank: f64,
}

/// What each of the four terms adds to a rank (C10).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankAdds {
    /// Urgency times its constant.
    pub urgency: f64,
    /// Running late times its constant.
    pub late: f64,
    /// Gravity's share times its constant.
    pub gravity: f64,
    /// Unlocks' share times its constant.
    pub unlocks: f64,
}

/// One ranked node: its terms and the signals its ties break on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ranked {
    terms: RankTerms,
    slack_days: Option<i32>,
    gravity: Score,
}

/// Pass 7's output: each ranked node's terms, and the frontier and acting frontier in rank
/// order.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranking {
    ranked: BTreeMap<NodeKey, Ranked>,
    frontier: Vec<NodeKey>,
    acting_frontier: Vec<NodeKey>,
}

impl Ranking {
    /// Ranks the normalization set.
    pub(crate) fn pass(priority: &Priority, dates: &Dates, blocking: &Blocking) -> Self {
        let set: Vec<(&NodeKey, Score, Score)> = blocking
            .keys()
            .iter()
            .filter(|key| priority.in_normalization_set(key))
            .map(|key| (key, priority.gravity(key), priority.unlocks(key)))
            .collect();
        let gravity_max = set.iter().map(|(_, gravity, _)| *gravity).max();
        let unlocks_max = set.iter().map(|(_, _, unlocks)| *unlocks).max();
        let ranked: BTreeMap<NodeKey, Ranked> = set
            .into_iter()
            .map(|(key, gravity, unlocks)| {
                let slack_days = dates.slack_days(key);
                let (urgency, late) = urgency(slack_days);
                let gravity_norm = normalized(gravity, gravity_max.unwrap_or_default());
                let unlocks_norm = normalized(unlocks, unlocks_max.unwrap_or_default());
                let adds = RankAdds {
                    urgency: RANK.urgency * urgency,
                    late: RANK.late * late,
                    gravity: RANK.gravity * gravity_norm,
                    unlocks: RANK.unlocks * unlocks_norm,
                };
                let terms = RankTerms {
                    urgency,
                    late,
                    gravity_norm,
                    unlocks_norm,
                    adds,
                    rank: adds.urgency + adds.late + adds.gravity + adds.unlocks,
                };
                assert_terms(&terms);
                let ranked = Ranked {
                    terms,
                    slack_days,
                    gravity,
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

/// Priority, Rank: `urgency` and `late` from slack over the horizon; both 0 for null slack.
fn urgency(slack_days: Option<i32>) -> (f64, f64) {
    let Some(slack) = slack_days else {
        return (0.0, 0.0);
    };
    let horizon = f64::from(RANK.horizon_days);
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
fn assert_terms(terms: &RankTerms) {
    for term in [
        terms.urgency,
        terms.late,
        terms.gravity_norm,
        terms.unlocks_norm,
    ] {
        assert!((0.0..=1.0).contains(&term), "a rank term is in [0, 1]");
    }
    let sum = RANK.urgency + RANK.late + RANK.gravity + RANK.unlocks;
    assert!(
        terms.rank.is_finite() && terms.rank >= 0.0 && terms.rank <= sum * (1.0 + 1e-12),
        "a rank is within the constants' sum"
    );
}
