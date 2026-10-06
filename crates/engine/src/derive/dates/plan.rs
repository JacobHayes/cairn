//! The plan layer's check (F5; Invariants: the date line): pins, rules, estimates,
//! containment, stage bounds, and dependencies, with no actuals and no today, must hold no
//! contradictory chain. Two kinds are found:
//!
//! - A positive cycle, pinned or not: Bellman-Ford with every instant of a cyclic component
//!   starting at zero, so no pin is needed to find one. After each, the cycle's constraint
//!   that asks for the most days is set aside and the component split again, so the next
//!   search finds another conflict, up to `chain_count_per_rejection_max` and one more to
//!   know there were more.
//! - With no positive cycle, two pins (or date answers) with a chain between them that needs
//!   more days than they allow: earliest bounds from the pins, and each pinned instant whose
//!   earliest bound passes its own pin. Any instant whose bounds cross does so because of
//!   such a pinned instant downstream, so checking those finds every conflict.

use cairn_schema::limits::CHAIN_COUNT_PER_REJECTION_MAX;

use super::Seeds;
use super::explain::Found;
use super::network::{Network, Slot};
use super::solve::{self, Components, Cost, Direction, Outcome, Via};

/// What the check found, with what it cost.
pub(crate) struct Findings {
    /// The contradictory chains, at most the limit.
    pub chains: Vec<Found>,
    /// More were found than the limit lists.
    pub more: bool,
    /// Operations spent.
    pub cost: Cost,
}

/// Runs the plan check over a network with the plan's seeds (pins and date answers only).
#[must_use]
pub(crate) fn check(network: &Network, seeds: &Seeds) -> Findings {
    let limit = CHAIN_COUNT_PER_REJECTION_MAX as usize;
    let mut cost = Cost::default();
    let mut enabled = vec![true; network.constraints().len()];
    let all: Vec<Slot> = (0..network.slot_count()).map(Slot::at).collect();
    let components = solve::components(network, &all, &enabled, &mut cost);
    let mut chains = cycles(network, &components, &mut enabled, limit, &mut cost);
    if chains.is_empty() {
        chains = pin_conflicts(network, &components, seeds, &enabled, &mut cost);
    }
    let more = chains.len() > limit;
    chains.truncate(limit);
    Findings { chains, more, cost }
}

/// Positive cycles, up to one past the limit.
fn cycles(
    network: &Network,
    components: &Components,
    enabled: &mut [bool],
    limit: usize,
    cost: &mut Cost,
) -> Vec<Found> {
    let mut work: Vec<Vec<Slot>> = components
        .iter()
        .filter(|(_, cyclic)| *cyclic)
        .map(|(slots, _)| slots.to_vec())
        .collect();
    let mut found = Vec::new();
    let fixed = vec![false; network.slot_count()];
    while let Some(slots) = work.pop() {
        if found.len() > limit {
            break;
        }
        let mut seed = vec![None; network.slot_count()];
        for slot in &slots {
            if let Some(value) = seed.get_mut(slot.index()) {
                *value = Some(0);
            }
        }
        let single = Components::one(slots.clone());
        let outcome = solve::solve(
            network,
            &single,
            Direction::Forward,
            &seed,
            &fixed,
            enabled,
            cost,
        );
        let Outcome::Contradiction(cycle) = outcome else {
            continue;
        };
        let weight: i32 = cycle
            .iter()
            .filter_map(|&at| network.constraint(at))
            .map(|constraint| constraint.offset_days)
            .sum();
        assert!(weight > 0, "a contradictory cycle gains days");
        let widest = cycle.iter().copied().max_by_key(|&at| {
            network
                .constraint(at)
                .map(|c| (c.offset_days, std::cmp::Reverse(at)))
        });
        if let Some(enabled) = widest.and_then(|at| enabled.get_mut(at as usize)) {
            *enabled = false;
        }
        found.push(Found {
            constraints: rotate(network, cycle),
            from: None,
            to: None,
            shortfall_days: weight,
            others: (None, None),
        });
        let split = solve::components(network, &slots, enabled, cost);
        work.extend(
            split
                .iter()
                .filter(|(_, cyclic)| *cyclic)
                .map(|(slots, _)| slots.to_vec()),
        );
    }
    found
}

/// A cycle's constraints, starting at the first shown instant, so an entry hop is folded
/// whole.
fn rotate(network: &Network, mut cycle: Vec<u32>) -> Vec<u32> {
    let start = cycle.iter().position(|&at| {
        network
            .constraint(at)
            .is_some_and(|constraint| network.shown(constraint.before))
    });
    assert!(
        start.is_some(),
        "every cycle passes a shown instant: entries form a tree"
    );
    cycle.rotate_left(start.unwrap_or(0));
    cycle
}

/// Pinned instants whose earliest bound, from the other pins, passes their own pin.
fn pin_conflicts(
    network: &Network,
    components: &Components,
    seeds: &Seeds,
    enabled: &[bool],
    cost: &mut Cost,
) -> Vec<Found> {
    let seed: Vec<Option<i32>> = (0..network.slot_count())
        .map(|at| seeds.pin(Slot::at(at)).map(|fix| fix.day))
        .collect();
    let fixed = vec![false; network.slot_count()];
    let Outcome::Settled(pass) = solve::solve(
        network,
        components,
        Direction::Forward,
        &seed,
        &fixed,
        enabled,
        cost,
    ) else {
        unreachable!("a plan with no positive cycle settles")
    };
    let mut found = Vec::new();
    for at in 0..network.slot_count() {
        let slot = Slot::at(at);
        let (Some(pin), Some(earliest)) = (seeds.pin(slot), pass.value(slot)) else {
            continue;
        };
        if earliest <= pin.day {
            continue;
        }
        let (constraints, from) = walk_back(network, &pass, Direction::Forward, slot);
        let others = pass
            .pulled(network, Direction::Forward, from)
            .map(|(day, _)| day);
        found.push(Found {
            constraints,
            from: Some(from),
            to: Some(slot),
            shortfall_days: earliest - pin.day,
            others: (others, None),
        });
    }
    found
}

/// The chain that set a slot's value, in order along the constraints, and the seeded slot it
/// starts from.
pub(crate) fn walk_back(
    network: &Network,
    pass: &solve::Pass,
    direction: Direction,
    slot: Slot,
) -> (Vec<u32>, Slot) {
    let mut chain = Vec::new();
    let mut at_slot = slot;
    while let Via::Constraint(at) = pass.via(at_slot) {
        let Some(constraint) = network.constraint(at) else {
            break;
        };
        chain.push(at);
        at_slot = match direction {
            Direction::Forward => constraint.before,
            Direction::Backward => constraint.after,
        };
        assert!(
            chain.len() <= network.slot_count(),
            "predecessors hold no cycle once settled"
        );
    }
    if direction == Direction::Forward {
        chain.reverse();
    }
    (chain, at_slot)
}
