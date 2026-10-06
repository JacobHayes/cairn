//! Solving the network (ARCHITECTURE, Date network: strongly connected components in
//! topological order, Bellman-Ford only inside components with a cycle). One routine serves
//! both directions: earliest bounds are longest paths forward from lower seeds; latest bounds
//! are the same longest paths over the reversed network, in negated days, from upper seeds.
//! Fixed slots (actuals) keep their seed and are never relaxed through; what their neighbours
//! would have given them is read afterwards, for shortfall.
//!
//! Every relaxation and every visit of a slot or constraint counts one operation, so the cost
//! test can hold each solve to a budget at the limits (PRACTICES, Explicit limits).

use std::collections::VecDeque;

use super::network::{Constraint, Network, Slot};

/// Which way a pass reads the constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
    /// Earliest bounds: `after` is pulled up by `before + k`.
    Forward,
    /// Latest bounds, negated: `before` is pulled up by `after + k` in negated days.
    Backward,
}

impl Direction {
    /// The slot a constraint pulls, and the slot it pulls from.
    fn ends(self, constraint: &Constraint) -> (Slot, Slot) {
        match self {
            Direction::Forward => (constraint.after, constraint.before),
            Direction::Backward => (constraint.before, constraint.after),
        }
    }

    /// The constraints that pull a slot.
    fn pulling(self, network: &Network, slot: Slot) -> &[u32] {
        match self {
            Direction::Forward => network.incoming(slot),
            Direction::Backward => network.outgoing(slot),
        }
    }

    /// The constraints a slot pulls on.
    fn pushing(self, network: &Network, slot: Slot) -> &[u32] {
        match self {
            Direction::Forward => network.outgoing(slot),
            Direction::Backward => network.incoming(slot),
        }
    }
}

/// What set a slot's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Via {
    /// Nothing reached it.
    Nothing,
    /// Its own seed.
    Seed,
    /// The constraint at this position, from its other end.
    Constraint(u32),
}

/// Operations spent, for the cost test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cost {
    /// Slots and constraints visited, and relaxations tried.
    pub operations: u64,
}

impl Cost {
    fn add(&mut self, count: usize) {
        self.operations = self
            .operations
            .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    }
}

/// The strongly connected components of the enabled constraints over a set of slots, in
/// topological order (a component's constraints from outside it come from earlier ones).
#[derive(Clone, Debug, Default)]
pub(crate) struct Components {
    /// Slots, component by component.
    slots: Vec<Slot>,
    /// Where each component starts in `slots`, plus the end.
    starts: Vec<usize>,
    /// Per component: it holds a cycle.
    cyclic: Vec<bool>,
}

impl Components {
    /// One component holding a cycle.
    #[must_use]
    pub(crate) fn one(slots: Vec<Slot>) -> Self {
        let size = slots.len();
        Self {
            slots,
            starts: vec![0, size],
            cyclic: vec![true],
        }
    }

    /// The components in topological order, each with whether it holds a cycle.
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = (&[Slot], bool)> {
        self.starts
            .windows(2)
            .zip(&self.cyclic)
            .map(|(bounds, cyclic)| {
                let (from, to) = (bounds.first().copied(), bounds.get(1).copied());
                let slots = from
                    .zip(to)
                    .and_then(|(from, to)| self.slots.get(from..to))
                    .unwrap_or(&[]);
                (slots, *cyclic)
            })
    }
}

const UNSEEN: u32 = u32::MAX;

/// Tarjan's algorithm's state over one set of slots.
struct Tarjan<'a> {
    network: &'a Network,
    enabled: &'a [bool],
    member: Vec<bool>,
    order: Vec<u32>,
    low: Vec<u32>,
    on_stack: Vec<bool>,
    stack: Vec<Slot>,
    next: u32,
    found: Components,
}

/// Tarjan's algorithm, iterative (PRACTICES, No recursion), over `slots` and the constraints
/// `enabled` keeps between them.
pub(crate) fn components(
    network: &Network,
    slots: &[Slot],
    enabled: &[bool],
    cost: &mut Cost,
) -> Components {
    let count = network.slot_count();
    let mut tarjan = Tarjan {
        network,
        enabled,
        member: vec![false; count],
        order: vec![UNSEEN; count],
        low: vec![UNSEEN; count],
        on_stack: vec![false; count],
        stack: Vec::new(),
        next: 0,
        found: Components::default(),
    };
    for slot in slots {
        set(&mut tarjan.member, slot.index(), true);
    }
    for &root in slots {
        if get(&tarjan.order, root.index()) == UNSEEN {
            tarjan.from(root, cost);
        }
    }
    let mut found = tarjan.found;
    // Tarjan finishes sinks first; reverse into topological order.
    reverse(&mut found);
    assert_eq!(
        found.slots.len(),
        slots.len(),
        "every slot is in one component"
    );
    found
}

impl Tarjan<'_> {
    /// The depth-first search from one root, with an explicit stack of (slot, next edge).
    fn from(&mut self, root: Slot, cost: &mut Cost) {
        let mut calls: Vec<(Slot, usize)> = vec![(root, 0)];
        self.visit(root);
        while let Some((slot, position)) = calls.last_mut() {
            let current = *slot;
            if let Some(&at) = self.network.outgoing(current).get(*position) {
                *position += 1;
                cost.add(1);
                let next = self
                    .network
                    .constraint(at)
                    .map(|constraint| constraint.after);
                let Some(next) = next.filter(|next| {
                    get(self.enabled, at as usize) && get(&self.member, next.index())
                }) else {
                    continue;
                };
                if get(&self.order, next.index()) == UNSEEN {
                    self.visit(next);
                    calls.push((next, 0));
                } else if get(&self.on_stack, next.index()) {
                    self.lower(current, get(&self.order, next.index()));
                }
                continue;
            }
            calls.pop();
            if let Some((parent, _)) = calls.last() {
                self.lower(*parent, get(&self.low, current.index()));
            }
            if get(&self.low, current.index()) == get(&self.order, current.index()) {
                self.close(current);
            }
        }
    }

    fn visit(&mut self, slot: Slot) {
        set(&mut self.order, slot.index(), self.next);
        set(&mut self.low, slot.index(), self.next);
        self.next += 1;
        set(&mut self.on_stack, slot.index(), true);
        self.stack.push(slot);
    }

    fn lower(&mut self, slot: Slot, to: u32) {
        let lowest = get(&self.low, slot.index()).min(to);
        set(&mut self.low, slot.index(), lowest);
    }

    /// Pops the component rooted at `root` off the stack.
    fn close(&mut self, root: Slot) {
        let from = self.found.slots.len();
        while let Some(slot) = self.stack.pop() {
            set(&mut self.on_stack, slot.index(), false);
            self.found.slots.push(slot);
            if slot == root {
                break;
            }
        }
        let size = self.found.slots.len() - from;
        let looped = self.network.outgoing(root).iter().any(|&at| {
            get(self.enabled, at as usize)
                && self.network.constraint(at).is_some_and(|c| c.after == root)
        });
        self.found.starts.push(from);
        self.found.cyclic.push(size > 1 || looped);
    }
}

fn reverse(found: &mut Components) {
    let total = found.slots.len();
    let mut slots = Vec::with_capacity(total);
    let mut starts = Vec::with_capacity(found.starts.len() + 1);
    let mut cyclic = Vec::with_capacity(found.cyclic.len());
    for at in (0..found.starts.len()).rev() {
        let from = found.starts.get(at).copied().unwrap_or(total);
        let to = found.starts.get(at + 1).copied().unwrap_or(total);
        starts.push(slots.len());
        slots.extend(found.slots.get(from..to).unwrap_or(&[]));
        cyclic.push(found.cyclic.get(at).copied().unwrap_or(false));
    }
    starts.push(slots.len());
    *found = Components {
        slots,
        starts,
        cyclic,
    };
}

/// One pass's values, in the pass's own days (negated for latest bounds), and what set each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pass {
    /// Per slot: the value, or none when nothing reaches it.
    pub value: Vec<Option<i32>>,
    /// Per slot: what set it.
    pub via: Vec<Via>,
}

impl Pass {
    /// The slot's value.
    #[must_use]
    pub(crate) fn value(&self, slot: Slot) -> Option<i32> {
        self.value.get(slot.index()).copied().flatten()
    }

    /// What set the slot's value.
    #[must_use]
    pub(crate) fn via(&self, slot: Slot) -> Via {
        self.via.get(slot.index()).copied().unwrap_or(Via::Nothing)
    }

    /// The best value the slot's neighbours give it, ignoring its own seed, with the
    /// constraint that gives it: what a fixed slot is held to by the constraints it is never
    /// relaxed through.
    #[must_use]
    pub(crate) fn pulled(
        &self,
        network: &Network,
        direction: Direction,
        slot: Slot,
    ) -> Option<(i32, u32)> {
        let mut best: Option<(i32, u32)> = None;
        for &at in direction.pulling(network, slot) {
            let Some(constraint) = network.constraint(at) else {
                continue;
            };
            let (_, from) = direction.ends(constraint);
            let Some(value) = self.value(from) else {
                continue;
            };
            let candidate = value.saturating_add(constraint.offset_days);
            if best.is_none_or(|(found, _)| candidate > found) {
                best = Some((candidate, at));
            }
        }
        best
    }
}

/// What a solve found: a pass, or a positive cycle (a contradictory chain) through these
/// constraint positions, in order along the cycle.
pub(crate) enum Outcome {
    /// Every component settled.
    Settled(Pass),
    /// A component holds a positive cycle.
    Contradiction(Vec<u32>),
}

/// Longest paths over the enabled constraints from `seed`, with `fixed` slots held at their
/// seed. Components in topological order (reverse for a backward pass); Bellman-Ford with an
/// active-slot queue inside a component with a cycle, stopping at a positive cycle.
pub(crate) fn solve(
    network: &Network,
    components: &Components,
    direction: Direction,
    seed: &[Option<i32>],
    fixed: &[bool],
    enabled: &[bool],
    cost: &mut Cost,
) -> Outcome {
    let count = network.slot_count();
    assert_eq!(seed.len(), count);
    assert_eq!(fixed.len(), count);
    let mut pass = Pass {
        value: seed.to_vec(),
        via: seed
            .iter()
            .map(|value| {
                if value.is_some() {
                    Via::Seed
                } else {
                    Via::Nothing
                }
            })
            .collect(),
    };
    let ordered: Vec<(&[Slot], bool)> = match direction {
        Direction::Forward => components.iter().collect(),
        Direction::Backward => components.iter().rev().collect(),
    };
    let mut member = vec![false; count];
    for (slots, cyclic) in ordered {
        for &slot in slots {
            if !get(fixed, slot.index()) {
                pull(network, direction, enabled, slot, &mut pass, cost);
            }
        }
        if !cyclic {
            continue;
        }
        for slot in slots {
            set(&mut member, slot.index(), true);
        }
        let settled = relax_cycle(
            network, direction, slots, &member, fixed, enabled, &mut pass, cost,
        );
        for slot in slots {
            set(&mut member, slot.index(), false);
        }
        if let Err(cycle) = settled {
            return Outcome::Contradiction(cycle);
        }
    }
    Outcome::Settled(pass)
}

/// Sets a slot from every constraint that pulls it, where that raises it.
fn pull(
    network: &Network,
    direction: Direction,
    enabled: &[bool],
    slot: Slot,
    pass: &mut Pass,
    cost: &mut Cost,
) {
    for &at in direction.pulling(network, slot) {
        cost.add(1);
        if !get(enabled, at as usize) {
            continue;
        }
        let Some(constraint) = network.constraint(at) else {
            continue;
        };
        let (_, from) = direction.ends(constraint);
        let Some(value) = pass.value(from) else {
            continue;
        };
        let candidate = value.saturating_add(constraint.offset_days);
        if pass.value(slot).is_none_or(|current| candidate > current) {
            set(&mut pass.value, slot.index(), Some(candidate));
            set(&mut pass.via, slot.index(), Via::Constraint(at));
        }
    }
}

/// Bellman-Ford inside one component: relaxes from slots that changed, in rounds, until
/// nothing changes. Past as many rounds as the component has slots, something still changing
/// lies downstream of a positive cycle; each further round looks for that cycle among the
/// predecessors, where it shows once values have climbed past every simple path.
#[allow(clippy::too_many_arguments)]
fn relax_cycle(
    network: &Network,
    direction: Direction,
    slots: &[Slot],
    member: &[bool],
    fixed: &[bool],
    enabled: &[bool],
    pass: &mut Pass,
    cost: &mut Cost,
) -> Result<(), Vec<u32>> {
    let mut active: VecDeque<Slot> = slots.iter().copied().collect();
    let mut queued = vec![false; network.slot_count()];
    for slot in slots {
        set(&mut queued, slot.index(), true);
    }
    let mut rounds = 0_usize;
    let mut round_left = active.len();
    while let Some(slot) = active.pop_front() {
        set(&mut queued, slot.index(), false);
        cost.add(1);
        for &at in direction.pushing(network, slot) {
            cost.add(1);
            let Some(constraint) = network.constraint(at) else {
                continue;
            };
            let (to, _) = direction.ends(constraint);
            if !get(enabled, at as usize) || !get(member, to.index()) || get(fixed, to.index()) {
                continue;
            }
            let Some(value) = pass.value(slot) else {
                continue;
            };
            let candidate = value.saturating_add(constraint.offset_days);
            if pass.value(to).is_none_or(|current| candidate > current) {
                set(&mut pass.value, to.index(), Some(candidate));
                set(&mut pass.via, to.index(), Via::Constraint(at));
                if !get(&queued, to.index()) {
                    set(&mut queued, to.index(), true);
                    active.push_back(to);
                }
            }
        }
        round_left = round_left.saturating_sub(1);
        if round_left > 0 {
            continue;
        }
        rounds += 1;
        round_left = active.len();
        if rounds > slots.len() && !active.is_empty() {
            cost.add(slots.len());
            if let Some(cycle) = predecessor_cycle(network, direction, slots, member, pass) {
                return Err(cycle);
            }
            assert!(
                rounds <= 2 * slots.len() + 2,
                "a positive cycle shows among the predecessors within twice the rounds"
            );
        }
    }
    Ok(())
}

/// A cycle among the slots' predecessors (the constraints that set their values), in order
/// along the chain. Such a cycle gains days every time around: a contradictory chain.
fn predecessor_cycle(
    network: &Network,
    direction: Direction,
    slots: &[Slot],
    member: &[bool],
    pass: &Pass,
) -> Option<Vec<u32>> {
    const UNSEEN: usize = usize::MAX;
    let back = |slot: Slot| match pass.via(slot) {
        Via::Constraint(at) => network
            .constraint(at)
            .map(|constraint| (direction.ends(constraint).1, at))
            .filter(|(previous, _)| get(member, previous.index())),
        Via::Seed | Via::Nothing => None,
    };
    let mut walk_of = vec![UNSEEN; network.slot_count()];
    for (walk, &first) in slots.iter().enumerate() {
        let mut slot = first;
        while get(&walk_of, slot.index()) == UNSEEN {
            set(&mut walk_of, slot.index(), walk);
            let Some((previous, _)) = back(slot) else {
                break;
            };
            slot = previous;
        }
        if get(&walk_of, slot.index()) != walk || back(slot).is_none() {
            continue;
        }
        // `slot` is on a cycle of this walk: go once around it.
        let mut cycle = Vec::new();
        let mut at_slot = slot;
        loop {
            let (previous, at) = back(at_slot)?;
            cycle.push(at);
            at_slot = previous;
            if at_slot == slot || cycle.len() > slots.len() {
                break;
            }
        }
        // Walked against the chain's direction in a forward pass.
        if direction == Direction::Forward {
            cycle.reverse();
        }
        return Some(cycle);
    }
    None
}

fn get<T: Copy + Default>(values: &[T], at: usize) -> T {
    values.get(at).copied().unwrap_or_default()
}

fn set<T>(values: &mut [T], at: usize, value: T) {
    if let Some(slot) = values.get_mut(at) {
        *slot = value;
    }
}
