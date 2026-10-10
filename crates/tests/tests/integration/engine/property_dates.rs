//! Property tests for the date network (F2, F3, F5, F6; ARCHITECTURE, Date network;
//! Read path: the reference test). Over generated deep journeys with estimates, rules from
//! milestones anywhere and from `created_at`, stage closes, pins, and recorded dates:
//!
//! - bounds over the entry chain equal those over the plain expansion, solved by a naive
//!   Bellman-Ford that copies every inherited requirement onto each descendant;
//! - no accepted graph has a positive cycle in its plan layer;
//! - removing a constraint never tightens a bound;
//! - every chain ends at a pin, an actual, or today.

#[cfg(test)]
mod property {
    use std::collections::{BTreeMap, BTreeSet};

    use cairn_engine::testing::derive_inputs;
    use cairn_engine::testing::generated::arb_journey;
    use cairn_engine::{Derived, Graph, derive};
    use cairn_schema::{
        Date, DateSource, Deployment, Direction, Instant, InstantPoint, NodeKey, NodeKind, Payload,
        State,
    };
    use patina_dst_proptest::prelude::*;

    fn derived(graph: &Graph) -> Derived {
        derive(graph, None, &derive_inputs(Deployment::default()))
    }

    fn today() -> i64 {
        day(derive_inputs(Deployment::default()).today)
    }

    fn day(date: Date) -> i64 {
        i64::from(date.since(Date::constant(1970, 1, 1)).unwrap().get_days())
    }

    /// An instant: a node's start or finish (a decision's or milestone's one instant is its
    /// finish), or `created_at` (no node).
    type Slot = (Option<NodeKey>, bool);

    fn start(graph: &Graph, key: &NodeKey) -> Slot {
        (Some(key.clone()), graph.node(key).unwrap().kind().is_leaf())
    }

    fn finish(key: &NodeKey) -> Slot {
        (Some(key.clone()), true)
    }

    /// The plain expansion of 2.2's reference test, in the pruned set: a node depends on its
    /// in-scope children and on every requirement, opening, and (unless force-included)
    /// condition decision of itself and its ancestors that is in scope.
    fn plain(graph: &Graph, derived: &Derived, key: &NodeKey) -> BTreeSet<NodeKey> {
        let in_scope = |other: &NodeKey| derived.relevance().in_scope(other);
        let tree = graph.tree();
        let mut found: BTreeSet<NodeKey> = tree.children(key).iter().cloned().collect();
        let mut conditions_apply = true;
        let mut current = Some(key.clone());
        while let Some(at) = current {
            let overrides = graph.document().state.overrides.get(&at);
            conditions_apply &= overrides.is_none_or(|o| o.force_include.is_none());
            let node = graph.node(&at).unwrap();
            found.extend(node.requires.iter().cloned());
            if let Payload::Group(group) = &node.payload {
                found.extend(group.opens_at.clone());
            }
            if conditions_apply {
                let reads = node.relevant_when.iter().flat_map(|c| c.decisions());
                found.extend(reads.cloned());
            }
            current = tree.parent(&at).cloned();
        }
        found.retain(|other| in_scope(other));
        found
    }

    fn skipped(graph: &Graph, derived: &Derived, key: &NodeKey) -> bool {
        let state = graph.document().state.nodes.get(key).map(|s| s.state);
        state == Some(State::Skipped) || derived.skips().skipped_by(key).is_some()
    }

    fn estimate(graph: &Graph, key: &NodeKey) -> i64 {
        let days = match &graph.node(key).unwrap().payload {
            Payload::Deliverable(deliverable) => deliverable.estimate,
            Payload::Action(action) => action.estimate,
            _ => None,
        };
        days.map_or(0, |days| i64::from(days.get()))
    }

    type Constraint = (Slot, Slot, i64);

    /// Every constraint, written plainly: no entries, each inherited requirement copied.
    /// `execution` gives done work zero duration, as its actual dates replace its estimate.
    fn plain_constraints(graph: &Graph, derived: &Derived, execution: bool) -> Vec<Constraint> {
        let in_scope = |key: &NodeKey| derived.relevance().in_scope(key);
        let mut found = Vec::new();
        for (key, node) in graph.document().nodes.as_map() {
            if !in_scope(key) {
                continue;
            }
            let done = graph
                .document()
                .state
                .nodes
                .get(key)
                .is_some_and(|s| s.state == State::Done);
            let zero = skipped(graph, derived, key) || (execution && done);
            if !node.kind().is_leaf() {
                let work = if zero { 0 } else { estimate(graph, key) };
                found.push((start(graph, key), finish(key), work));
            }
            for required in plain(graph, derived, key) {
                found.push((finish(&required), start(graph, key), 0));
            }
            let rules = [
                (true, node.due_by.as_ref()),
                (false, node.not_before.as_ref()),
            ];
            for (due_by, rule) in rules {
                let Some(rule) = rule else { continue };
                let offset = i64::from(rule.offset.get());
                let signed = if rule.direction == Direction::After {
                    offset
                } else {
                    -offset
                };
                for source in rule.sources.as_set() {
                    let slot = match source {
                        DateSource::CreatedAt => (None, false),
                        DateSource::Node(other) if in_scope(other) => finish(other),
                        DateSource::Node(_) => continue,
                    };
                    if due_by {
                        found.push((finish(key), slot, -signed));
                    } else {
                        found.push((slot, start(graph, key), signed));
                    }
                }
            }
            if let Payload::Group(group) = &node.payload
                && let Some(closes_at) = group.closes_at.as_ref().filter(|_| group.closes)
                && in_scope(closes_at)
            {
                found.push((finish(key), finish(closes_at), 0));
            }
        }
        found.retain(|(before, after, offset)| before != after || *offset > 0);
        found
    }

    /// The execution layer's seeds: (lower, upper, fixed) per slot, in days.
    fn seeds(graph: &Graph, derived: &Derived) -> BTreeMap<Slot, (Option<i64>, Option<i64>, bool)> {
        let mut seeds: BTreeMap<Slot, (Option<i64>, Option<i64>, bool)> = BTreeMap::new();
        let document = graph.document();
        for (key, date) in &document.state.pins {
            if derived.relevance().in_scope(key) {
                seeds.insert(finish(key), (Some(day(*date)), Some(day(*date)), false));
            }
        }
        for (key, stored) in &document.state.nodes {
            if !derived.relevance().in_scope(key) || skipped(graph, derived, key) {
                continue;
            }
            let mut floor = |slot: Slot| {
                let entry = seeds.entry(slot).or_insert((None, None, false));
                entry.0 = Some(entry.0.map_or(today(), |pin| pin.max(today())));
            };
            match stored.state {
                State::Todo | State::Open => floor(start(graph, key)),
                State::Active => floor(finish(key)),
                _ => {}
            }
            let actuals = [
                (start(graph, key), stored.started_on),
                (
                    finish(key),
                    stored.finished_on.filter(|_| stored.state.is_terminal()),
                ),
            ];
            for (slot, recorded) in actuals {
                let recorded = recorded.filter(|_| {
                    matches!(
                        stored.state,
                        State::Active | State::Done | State::Decided | State::Reached
                    )
                });
                if let Some(recorded) = recorded {
                    seeds.insert(slot, (Some(day(recorded)), Some(day(recorded)), true));
                }
            }
        }
        seeds
    }

    /// Naive Bellman-Ford: longest paths forward (or backward, in negated days), fixed slots
    /// held. Panics past as many rounds as there are slots, which only a positive cycle needs.
    fn longest(
        constraints: &[Constraint],
        seeds: &BTreeMap<Slot, (Option<i64>, Option<i64>, bool)>,
        forward: bool,
    ) -> BTreeMap<Slot, i64> {
        let mut value: BTreeMap<Slot, i64> = BTreeMap::new();
        for (slot, (lower, upper, _)) in seeds {
            let seed = if forward { *lower } else { upper.map(|u| -u) };
            if let Some(seed) = seed {
                value.insert(slot.clone(), seed);
            }
        }
        let rounds = 2 * constraints.len() + 2;
        for _ in 0..rounds {
            let mut changed = false;
            for (before, after, offset) in constraints {
                let (from, to) = if forward {
                    (before, after)
                } else {
                    (after, before)
                };
                if seeds.get(to).is_some_and(|seed| seed.2) {
                    continue;
                }
                let Some(&found) = value.get(from) else {
                    continue;
                };
                if value
                    .get(to)
                    .is_none_or(|&current| found + offset > current)
                {
                    value.insert(to.clone(), found + offset);
                    changed = true;
                }
            }
            if !changed {
                return value;
            }
        }
        panic!("the plain network does not settle: a positive cycle")
    }

    fn engine_bounds(derived: &Derived, key: &NodeKey) -> [Option<i64>; 3] {
        let dates = derived.dates();
        [
            dates.earliest_start(key),
            dates.latest_start(key),
            dates.due(key),
        ]
        .map(|d| d.map(day))
    }

    fn plain_bounds(
        graph: &Graph,
        earliest: &BTreeMap<Slot, i64>,
        latest: &BTreeMap<Slot, i64>,
        key: &NodeKey,
    ) -> [Option<i64>; 3] {
        let start = start(graph, key);
        [
            earliest.get(&start).copied(),
            latest.get(&start).map(|l| -l),
            latest.get(&finish(key)).map(|l| -l),
        ]
    }

    /// The plain shortfall at a node: at each of its instants, an earliest bound past the
    /// latest, or a fact later than the constraints after it allow; the larger.
    fn plain_shortfall(
        graph: &Graph,
        constraints: &[Constraint],
        seeds: &BTreeMap<Slot, (Option<i64>, Option<i64>, bool)>,
        (earliest, latest): (&BTreeMap<Slot, i64>, &BTreeMap<Slot, i64>),
        key: &NodeKey,
    ) -> Option<u32> {
        let mut worst: Option<i64> = None;
        for slot in [start(graph, key), finish(key)] {
            let gap = match seeds.get(&slot) {
                Some((Some(actual), _, true)) => constraints
                    .iter()
                    .filter(|(before, _, _)| *before == slot)
                    .filter_map(|(_, after, offset)| latest.get(after).map(|l| -l - offset))
                    .min()
                    .map(|allowed| actual - allowed),
                _ => earliest
                    .get(&slot)
                    .zip(latest.get(&slot))
                    .map(|(e, l)| e + l),
            };
            if let Some(gap) = gap.filter(|gap| *gap > 0) {
                worst = Some(worst.map_or(gap, |w| w.max(gap)));
            }
        }
        worst.map(|days| u32::try_from(days).unwrap())
    }

    /// One constraint source a removal drops.
    #[derive(Clone, Debug)]
    enum Removal {
        Requires(NodeKey),
        DueBy,
        NotBefore,
        Estimate,
    }

    /// Every rule, estimate, and explicit requirement in the graph.
    fn removable(graph: &Graph) -> Vec<(NodeKey, Removal)> {
        let mut found = Vec::new();
        for (key, node) in graph.document().nodes.as_map() {
            let at = |removal| (key.clone(), removal);
            found.extend(
                node.requires
                    .iter()
                    .map(|r| at(Removal::Requires(r.clone()))),
            );
            if node.due_by.is_some() {
                found.push(at(Removal::DueBy));
            }
            if node.not_before.is_some() {
                found.push(at(Removal::NotBefore));
            }
            if estimate(graph, key) > 0 {
                found.push(at(Removal::Estimate));
            }
        }
        found
    }

    fn without(graph: &Graph, (key, removal): &(NodeKey, Removal)) -> Graph {
        let mut document = graph.document().clone();
        let mut node = document.nodes.get(key).unwrap().clone();
        match removal {
            Removal::Requires(dropped) => {
                let kept: BTreeSet<NodeKey> = node
                    .requires
                    .iter()
                    .filter(|r| *r != dropped)
                    .cloned()
                    .collect();
                node.requires = cairn_schema::BoundedSet::new(kept).unwrap();
            }
            Removal::DueBy => node.due_by = None,
            Removal::NotBefore => node.not_before = None,
            Removal::Estimate => match &mut node.payload {
                Payload::Deliverable(deliverable) => deliverable.estimate = None,
                Payload::Action(action) => action.estimate = None,
                _ => unreachable!("only work has an estimate"),
            },
        }
        document.nodes.put(node).unwrap();
        Graph::new(document, &Deployment::default()).unwrap()
    }

    /// A chain's ends are fixed: its first fixed date at its first instant, its last at its
    /// last, and nothing else ends a chain.
    fn ends_fixed(chain: &cairn_schema::Chain, at: &Instant, earliest: bool) -> bool {
        let first = chain.constraints.first().map_or(at, |c| &c.before);
        let last = chain.constraints.last().map_or(at, |c| &c.after);
        let end = if earliest {
            chain.fixed.first()
        } else {
            chain.fixed.last()
        };
        let instant = if earliest { first } else { last };
        end.is_some_and(|fixed| fixed.instant == *instant)
    }

    proptest! {
        #[test]
        fn bounds_over_the_entry_chain_match_the_plain_expansion(graph in arb_journey(1..=60)) {
            let derived = derived(&graph);
            let constraints = plain_constraints(&graph, &derived, true);
            let seeds = seeds(&graph, &derived);
            let earliest = longest(&constraints, &seeds, true);
            let latest = longest(&constraints, &seeds, false);
            for key in graph.document().nodes.as_map().keys() {
                prop_assert_eq!(
                    engine_bounds(&derived, key),
                    plain_bounds(&graph, &earliest, &latest, key),
                    "{} [earliest start, latest start, due]", key
                );
                prop_assert_eq!(
                    derived.dates().shortfall_days(key),
                    plain_shortfall(&graph, &constraints, &seeds, (&earliest, &latest), key),
                    "{} shortfall", key
                );
            }
        }

        #[test]
        fn no_accepted_plan_has_a_positive_cycle(graph in arb_journey(1..=80)) {
            let derived = derived(&graph);
            let constraints = plain_constraints(&graph, &derived, false);
            let mut zero = BTreeMap::new();
            for (before, after, _) in &constraints {
                zero.insert(before.clone(), (Some(0), None, false));
                zero.insert(after.clone(), (Some(0), None, false));
            }
            longest(&constraints, &zero, true);
            prop_assert!(cairn_engine::check_plan(&graph, &Deployment::default()).is_consistent());
        }

        #[test]
        fn removing_a_constraint_never_tightens_a_bound(graph in arb_journey(1..=60), pick in any::<usize>()) {
            let candidates = removable(&graph);
            prop_assume!(!candidates.is_empty());
            let looser = without(&graph, &candidates[pick % candidates.len()]);
            let (before, after) = (derived(&graph), derived(&looser));
            for key in graph.document().nodes.as_map().keys() {
                let [early_was, late_was, due_was] = engine_bounds(&before, key);
                let [early, late, due] = engine_bounds(&after, key);
                prop_assert!(early_was.is_some() || early.is_none(), "{} earliest appeared", key);
                prop_assert!(early <= early_was || early_was.is_none(), "{} earliest rose", key);
                for (was, now) in [(late_was, late), (due_was, due)] {
                    prop_assert!(now.is_none() || was.is_some_and(|was| now >= Some(was)), "{} latest fell", key);
                }
                let short = |d: &Derived| d.dates().shortfall_days(key).unwrap_or(0);
                prop_assert!(short(&after) <= short(&before), "{} shortfall grew", key);
            }
        }

        #[test]
        fn every_chain_ends_at_a_pin_an_actual_or_today(graph in arb_journey(1..=60)) {
            let derived = derived(&graph);
            for key in graph.document().nodes.as_map().keys() {
                let dates = derived.dates().node(&graph, key);
                let kind = graph.node(key).unwrap().kind();
                let point = |finish: bool| Instant::Node {
                    node: key.clone(),
                    point: if finish || kind == NodeKind::Milestone || kind == NodeKind::Decision { InstantPoint::Finish } else { InstantPoint::Start },
                };
                if let Some(bound) = &dates.earliest_start {
                    prop_assert!(ends_fixed(&bound.chain, &point(false), true), "{} earliest: {:#?}", key, bound);
                }
                for (bound, finish) in [(&dates.latest_start, false), (&dates.due, true)] {
                    if let Some(bound) = bound {
                        prop_assert!(ends_fixed(&bound.chain, &point(finish), false), "{} latest: {:#?}", key, bound);
                    }
                }
                if let Some(short) = &dates.shortfall {
                    prop_assert!(short.shortfall_days > 0);
                    let chain = &short.chain;
                    prop_assert!(!chain.fixed.is_empty(), "{} {:#?}", key, short);
                }
            }
        }
    }
}
