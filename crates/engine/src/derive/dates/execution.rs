//! The execution layer (F3, F6): the plan's network with actuals as fixed facts and today
//! holding unfinished work back, solved into earliest bounds (forward, from lower seeds) and
//! latest bounds (backward, from upper seeds). The two passes never feed each other, so a late
//! actual or a passed latest start never invalidates the plan; where an instant's earliest
//! bound passes its latest, or a fact passes what its neighbours allow, the gap is a
//! shortfall.

use super::Seeds;
use super::explain::Found;
use super::network::{Network, Place, Slot};
use super::plan::walk_back;
use super::solve::{self, Cost, Direction, Outcome, Pass};

/// Both passes, with what they cost.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Solved {
    /// Earliest bounds, in days.
    pub earliest: Pass,
    /// Latest bounds, in negated days.
    pub latest: Pass,
    /// Operations spent.
    pub cost: Cost,
}

/// Solves the execution layer: earliest bounds from actuals, pins, answers, and today;
/// latest bounds from actuals, pins, and answers. Actuals are held, never relaxed through.
///
/// # Panics
///
/// When the network holds a positive cycle, which the plan check rejects before any graph
/// is derived (a graph checked against another deployment's aliases is the one way here).
#[must_use]
pub(crate) fn solve(network: &Network, seeds: &Seeds) -> Solved {
    let slots: Vec<Slot> = (0..network.slot_count()).map(Slot::at).collect();
    let enabled = vec![true; network.constraints().len()];
    let mut cost = Cost::default();
    let components = solve::components(network, &slots, &enabled, &mut cost);
    let fixed: Vec<bool> = slots.iter().map(|&s| seeds.actual(s).is_some()).collect();
    let lower: Vec<Option<i32>> = slots
        .iter()
        .map(|&s| seeds.lower(s).map(|fix| fix.day))
        .collect();
    let upper: Vec<Option<i32>> = slots
        .iter()
        .map(|&s| seeds.upper(s).map(|fix| -fix.day))
        .collect();
    let mut pass = |direction, seed: &[Option<i32>]| match solve::solve(
        network,
        &components,
        direction,
        seed,
        &fixed,
        &enabled,
        &mut cost,
    ) {
        Outcome::Settled(pass) => pass,
        Outcome::Contradiction(_) => {
            panic!("derive runs on a graph whose plan holds no contradictory chain (F5)")
        }
    };
    let earliest = pass(Direction::Forward, &lower);
    let latest = pass(Direction::Backward, &upper);
    Solved {
        earliest,
        latest,
        cost,
    }
}

impl Solved {
    /// The slot's earliest bound, in days.
    #[must_use]
    pub(crate) fn earliest(&self, slot: Slot) -> Option<i32> {
        self.earliest.value(slot)
    }

    /// The slot's latest bound, in days.
    #[must_use]
    pub(crate) fn latest(&self, slot: Slot) -> Option<i32> {
        self.latest.value(slot).map(|negated| -negated)
    }

    /// The chain behind a slot's earliest bound, from the seed that holds it back.
    #[must_use]
    pub(crate) fn earliest_chain(&self, network: &Network, slot: Slot) -> Found {
        let (constraints, from) = walk_back(network, &self.earliest, Direction::Forward, slot);
        Found {
            constraints,
            from: Some(from),
            to: None,
            shortfall_days: 0,
            others: (None, None),
        }
    }

    /// The chain behind a slot's latest bound, to the seed that caps it.
    #[must_use]
    pub(crate) fn latest_chain(&self, network: &Network, slot: Slot) -> Found {
        let (constraints, to) = walk_back(network, &self.latest, Direction::Backward, slot);
        Found {
            constraints,
            from: None,
            to: Some(to),
            shortfall_days: 0,
            others: (None, None),
        }
    }

    /// F6: the slot's shortfall with its chain, from the seed that holds it back to the seed
    /// that caps it: an earliest bound past the latest, or a fact later than the chain after
    /// it allows. A fact earlier than the chain before it requires shows at that chain's
    /// upstream end, as an earliest bound past its latest or a fact too late, wherever it
    /// lies; where that end is `created_at` or a date answer, no node's instant, the fact
    /// shows it.
    #[must_use]
    pub(crate) fn shortfall(&self, network: &Network, seeds: &Seeds, slot: Slot) -> Option<Found> {
        let Some(actual) = seeds.actual(slot) else {
            let (earliest, latest) = (self.earliest(slot)?, self.latest(slot)?);
            if earliest <= latest {
                return None;
            }
            let mut found = self.earliest_chain(network, slot);
            let after = self.latest_chain(network, slot);
            found.constraints.extend(after.constraints);
            found.to = after.to;
            found.shortfall_days = earliest - latest;
            return Some(self.with_others(network, found));
        };
        let too_late = self.too_late(network, slot, actual.day);
        let too_early = too_early(network, seeds, slot, actual.day);
        let found = match (too_late, too_early) {
            (Some(late), Some(early)) if early.shortfall_days > late.shortfall_days => early,
            (Some(late), _) => late,
            (None, early) => early?,
        };
        Some(self.with_others(network, found))
    }

    /// The chain with what else holds its start back and caps its end, so a resolution
    /// never offers moving a pin that another chain or today would still hold.
    fn with_others(&self, network: &Network, mut found: Found) -> Found {
        let lower = found
            .from
            .and_then(|from| self.earliest.pulled(network, Direction::Forward, from));
        let upper = found
            .to
            .and_then(|to| self.latest.pulled(network, Direction::Backward, to));
        found.others = (
            lower.map(|(day, _)| day),
            upper.map(|(negated, _)| -negated),
        );
        found
    }

    /// A fact later than the constraints after it allow.
    fn too_late(&self, network: &Network, slot: Slot, actual: i32) -> Option<Found> {
        let (negated, at) = self.latest.pulled(network, Direction::Backward, slot)?;
        let allowed = -negated;
        if allowed >= actual {
            return None;
        }
        let after = network.constraint(at).map_or(slot, |c| c.after);
        let later = self.latest_chain(network, after);
        let mut constraints = vec![at];
        constraints.extend(later.constraints);
        Some(Found {
            constraints,
            from: Some(slot),
            to: later.to,
            shortfall_days: actual - allowed,
            others: (None, None),
        })
    }
}

/// A fact earlier than a rule from `created_at` or a date answer requires, by the date that
/// fixes the source. Such a source is no node's instant, so its own lateness would show on
/// no node: the fact it holds back shows it instead.
fn too_early(network: &Network, seeds: &Seeds, slot: Slot, actual: i32) -> Option<Found> {
    let mut worst: Option<Found> = None;
    for &at in network.incoming(slot) {
        let Some(constraint) = network.constraint(at) else {
            continue;
        };
        let fixed = match network.place(constraint.before) {
            Place::CreatedAt => seeds.actual(constraint.before),
            Place::Answer(_) => seeds.pin(constraint.before),
            Place::Node(_) => None,
        };
        let Some(earliest) = fixed.map(|fix| fix.day) else {
            continue;
        };
        let gap = earliest.saturating_add(constraint.offset_days) - actual;
        if gap > 0
            && worst
                .as_ref()
                .is_none_or(|found| gap > found.shortfall_days)
        {
            worst = Some(Found {
                constraints: vec![at],
                from: Some(constraint.before),
                to: Some(slot),
                shortfall_days: gap,
                others: (None, None),
            });
        }
    }
    worst
}
