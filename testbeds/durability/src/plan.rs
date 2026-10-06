//! The workload: a sequence of domain patches drawn from one seed, each valid against the
//! state the ones before it leave (A17). One client submits them in order and moves on only
//! once a patch is acknowledged, so after any crash the committed patches are a prefix of
//! the plan, give or take the one in flight.
//!
//! The plan is a pure function of its seed and length: a restarted incarnation rebuilds it
//! from the seed the ledger recorded and knows every patch id, its content, and how many
//! events it emits (J2: one per mutation).

use std::collections::BTreeSet;
use std::fmt::Write as _;

use cairn_schema::{JourneyId, Patch, PatchId, Timestamp, from_yaml};

/// The journeys a plan writes to.
const JOURNEYS: [&str; 3] = ["j_alpha", "j_beta", "j_gamma"];

/// When the first patch is submitted; each later one a minute after the one before.
const FIRST_AT: Timestamp = Timestamp::constant(1_791_288_000, 0);

/// One planned patch.
#[derive(Clone, Debug)]
pub struct Step {
    /// The patch, with its id, target, base revision, and mutations.
    pub patch: Patch,
    /// When it is submitted (the call's clock).
    pub at: Timestamp,
}

impl Step {
    /// The events its commit emits (J2).
    pub fn event_count(&self) -> usize {
        self.patch.mutations.len()
    }
}

/// The whole workload.
#[derive(Clone, Debug)]
pub struct Plan {
    pub seed: u64,
    pub steps: Vec<Step>,
}

impl Plan {
    /// `length` patches drawn from `seed`.
    pub fn generate(seed: u64, length: usize) -> Plan {
        let mut draw = Draw(seed);
        let mut model = Model::default();
        let mut steps = Vec::with_capacity(length);
        for index in 0..length {
            let minutes = i64::try_from(index).unwrap_or(i64::MAX);
            let at = FIRST_AT
                .saturating_add(jiff::SignedDuration::from_mins(minutes))
                .unwrap_or(FIRST_AT);
            steps.push(Step {
                patch: model.next(index, &mut draw),
                at,
            });
        }
        Plan { seed, steps }
    }

    /// The index of a planned patch id, if it is one.
    pub fn index_of(&self, id: &PatchId) -> Option<usize> {
        let index = id.as_str().strip_prefix("p_step_")?.parse::<usize>().ok()?;
        (index < self.steps.len()).then_some(index)
    }

    /// Every journey the plan writes to.
    pub fn journeys() -> impl Iterator<Item = JourneyId> {
        JOURNEYS.iter().map(|id| parse_id(id))
    }
}

/// What the plan has done so far, so each patch is valid where it lands.
#[derive(Default)]
struct Model {
    journeys: [JourneyModel; JOURNEYS.len()],
    deployment_revision: u32,
    next_key: u32,
}

#[derive(Default)]
struct JourneyModel {
    revision: u32,
    /// Action nodes by key, with their state.
    nodes: Vec<NodeModel>,
}

struct NodeModel {
    key: String,
    /// The nodes it requires: only older ones, so the graph stays acyclic.
    requires: BTreeSet<String>,
    state: Progress,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Progress {
    NotStarted,
    Started,
    Complete,
}

impl Model {
    fn key(&mut self) -> u32 {
        self.next_key += 1;
        self.next_key
    }

    /// The next patch: a deployment patch creating an entity one time in eight, otherwise a
    /// patch to a drawn journey (its creation first), now and then with an entity create
    /// riding along (which moves the deployment's revision too).
    fn next(&mut self, index: usize, draw: &mut Draw) -> Patch {
        let id = format!("p_step_{index}");
        if draw.below(8) == 0 {
            let key = self.key();
            let base = self.deployment_revision;
            self.deployment_revision += 1;
            let mutations =
                format!("- op: create_entity\n  entity: {{key: e_k{key}, name: Entity {key}}}\n");
            return patch(&id, "deployment", base, &mutations);
        }
        let which = draw.below(JOURNEYS.len());
        let mut mutations = String::new();
        let journey_base = self.journeys[which].revision;
        if journey_base == 0 {
            let _ = writeln!(mutations, "- op: create_journey\n  name: Journey {which}");
            for _ in 0..draw.below(3) {
                mutations += &self.add_node(which);
            }
        } else {
            for _ in 0..=draw.below(3) {
                mutations += &self.mutation(which, draw);
            }
        }
        if draw.below(6) == 0 {
            let key = self.key();
            let _ = writeln!(
                mutations,
                "- op: create_entity\n  entity: {{key: e_k{key}, name: Entity {key}}}"
            );
            self.deployment_revision += 1;
        }
        self.journeys[which].revision += 1;
        let target = format!("{{journey: {}}}", JOURNEYS[which]);
        patch(&id, &target, journey_base, &mutations)
    }

    fn add_node(&mut self, which: usize) -> String {
        let key = self.key();
        self.journeys[which].nodes.push(NodeModel {
            key: format!("n_k{key}"),
            requires: BTreeSet::new(),
            state: Progress::NotStarted,
        });
        format!(
            "- op: add_node\n  node: {{key: n_k{key}, id: k{key}, kind: action, title: Node {key}}}\n"
        )
    }

    /// One mutation on an existing journey: a node added, renamed, started, or completed, an
    /// edge added from a node not yet started to an older one, or the journey renamed. A
    /// node with requirements is never started, so no transition meets a gate.
    fn mutation(&mut self, which: usize, draw: &mut Draw) -> String {
        let journey = &mut self.journeys[which];
        let count = journey.nodes.len();
        match draw.below(6) {
            1 if count > 0 => {
                let node = &journey.nodes[draw.below(count)];
                let title = draw.below(1000);
                format!(
                    "- op: set_node_field\n  node: {}\n  value: {{title: Renamed {title}}}\n",
                    node.key
                )
            }
            2 if count > 0 => {
                let node = &mut journey.nodes[draw.below(count)];
                match node.state {
                    Progress::NotStarted if node.requires.is_empty() => {
                        node.state = Progress::Started;
                        format!(
                            "- op: transition\n  node: {}\n  transition: start\n",
                            node.key
                        )
                    }
                    Progress::Started => {
                        node.state = Progress::Complete;
                        format!(
                            "- op: transition\n  node: {}\n  transition: complete\n",
                            node.key
                        )
                    }
                    _ => self.add_node(which),
                }
            }
            3 if count > 1 => {
                let later = 1 + draw.below(count - 1);
                let earlier = draw.below(later);
                let required = journey.nodes[earlier].key.clone();
                let node = &mut journey.nodes[later];
                if node.state == Progress::NotStarted && node.requires.insert(required.clone()) {
                    format!(
                        "- op: add_edge\n  edge: {{node: {}, requires: {required}}}\n",
                        node.key
                    )
                } else {
                    self.add_node(which)
                }
            }
            4 => format!(
                "- op: edit_journey\n  name: Journey {which} take {}\n",
                draw.below(1000)
            ),
            _ => self.add_node(which),
        }
    }
}

fn patch(id: &str, target: &str, base: u32, mutations: &str) -> Patch {
    let yaml =
        format!("id: {id}\ntarget: {target}\nbase_revision: {base}\nmutations:\n{mutations}");
    match from_yaml(&yaml) {
        Ok(patch) => patch,
        Err(error) => unreachable!("the plan writes only valid patches: {error:?}\n{yaml}"),
    }
}

fn parse_id<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    match text.parse() {
        Ok(id) => id,
        Err(error) => unreachable!("{text} is an id: {error:?}"),
    }
}

/// `SplitMix64`: the plan's own generator, so a plan depends on nothing but its seed.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..bound` (`bound` > 0).
    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).unwrap_or(u64::MAX).max(1);
        usize::try_from(self.next() % bound).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_engine::{ApplyInputs, Records, apply};
    use cairn_schema::{Actor, Date};

    /// Every patch of many plans is accepted by the engine where it lands, so a rejection
    /// in a run is a finding, never the plan's own mistake.
    #[test]
    fn every_planned_patch_applies_in_order() {
        for seed in 0..64 {
            let plan = Plan::generate(seed, 40);
            let mut records = Records::default();
            for step in &plan.steps {
                let inputs = ApplyInputs {
                    today: Date::constant(2026, 10, 6),
                    at: step.at,
                    actor: Actor {
                        user: parse_id("u_client"),
                        agent: None,
                    },
                    note: None,
                };
                match apply(&records, &step.patch, &inputs) {
                    Ok(applied) => records = applied.records().clone(),
                    Err(rejection) => panic!("seed {seed} {}: {rejection:#?}", step.patch.id),
                }
            }
        }
    }

    #[test]
    fn a_plan_is_a_function_of_its_seed() {
        let ids = |seed| -> Vec<_> {
            Plan::generate(seed, 30)
                .steps
                .into_iter()
                .map(|step| (step.patch.id.clone(), step.patch.content_hash()))
                .collect()
        };
        assert_eq!(ids(7), ids(7));
        assert_ne!(ids(7), ids(8));
    }

    #[test]
    fn planned_ids_map_back_to_their_index() {
        let plan = Plan::generate(1, 5);
        assert_eq!(plan.index_of(&plan.steps[3].patch.id), Some(3));
        assert_eq!(plan.index_of(&parse_id("p_step_5")), None);
        assert_eq!(plan.index_of(&parse_id("p_other")), None);
    }
}
