//! Every fixture scenario runs through apply (J1, J2, A17), each step replayed (J3, in
//! `support::apply`), and ends in the state the fixture describes.
#![cfg(test)]

use crate::engine::support;

use std::collections::BTreeSet;

use cairn_schema::{Graph, Mutation, ParticipationSource, ProposalStatus, State};
use support::{finished, key};

#[test]
fn every_fixture_scenario_applies() {
    for name in support::fixture_names() {
        let (_, applied) = support::run(&name);
        let steps = support::scenario(&name);
        for (step, result) in steps.steps.as_slice().iter().zip(&applied) {
            assert_eq!(result.events().len(), step.patch.mutations.len(), "{name}");
        }
    }
}

/// What a fixture's scenario submits, read from its source, so an expected count follows the
/// fixture rather than being written down a second time: each step's mutations, with those of
/// a proposal it drafts.
fn submitted(name: &str) -> Vec<Vec<Mutation>> {
    let scenario = support::scenario(name);
    let mut steps = Vec::new();
    for step in scenario.steps.as_slice() {
        let mut mutations = Vec::new();
        for mutation in step.patch.mutations.as_slice() {
            if let Mutation::CreateProposal { proposal } = mutation {
                mutations.extend(proposal.mutations.as_slice().iter().cloned());
            }
            mutations.push(mutation.clone());
        }
        steps.push(mutations);
    }
    steps
}

/// The nodes a fixture's scenario adds under `parent`.
fn added_under(name: &str, parent: &str) -> usize {
    let parent = Some(key(parent));
    submitted(name)
        .iter()
        .flatten()
        .filter(|mutation| matches!(mutation, Mutation::AddNode { node } if node.parent == parent))
        .count()
}

/// The nodes a fixture's scenario adds anywhere.
fn added(name: &str) -> usize {
    let adds = |mutation: &&Mutation| matches!(mutation, Mutation::AddNode { .. });
    submitted(name).iter().flatten().filter(adds).count()
}

/// The decisions a fixture's scenario answers.
fn answered(name: &str) -> BTreeSet<cairn_schema::NodeKey> {
    let decision = |mutation: &Mutation| match mutation {
        Mutation::Answer { decision, .. } => Some(decision.clone()),
        _ => None,
    };
    submitted(name)
        .iter()
        .flatten()
        .filter_map(decision)
        .collect()
}

/// E6: the deployment's revision after a fixture's scenario: one for the shared people, and one
/// for each step that creates identities, directly or in the proposal it drafts (whose creates
/// ride in its apply).
fn identity_steps(name: &str) -> u32 {
    let creates = |step: &Vec<Mutation>| {
        step.iter()
            .any(|mutation| matches!(mutation, Mutation::CreateIdentity { .. }))
    };
    let steps = submitted(name).iter().filter(|step| creates(step)).count();
    1 + u32::try_from(steps).unwrap()
}

fn only_graph(records: &cairn_engine::Records) -> &Graph {
    &records.journeys.values().next().unwrap().graph
}

fn state_of(graph: &Graph, node: &str) -> State {
    graph.state.nodes[&key(node)].state
}

/// B1, B6, B10, E6: the placeholder is broken down into the children it names, completing
/// the baseline lifted its snooze, the final report is pinned, and an identity is created and
/// used in one patch.
#[test]
fn the_vendor_evaluation_ends_broken_down_and_pinned() {
    let records = finished("vendor-evaluation");
    let graph = only_graph(&records);
    assert_eq!(state_of(graph, "n_workload"), State::Done);
    let children = graph
        .nodes
        .values()
        .filter(|node| node.parent == Some(key("n_workload")))
        .count();
    let breakdown = added_under("vendor-evaluation", "n_workload");
    assert!(breakdown > 0, "the scenario breaks the placeholder down");
    assert_eq!(children, breakdown, "the placeholder was broken down (B10)");
    assert!(graph.state.snoozes.is_empty(), "B6");
    assert_eq!(
        graph.state.pins[&key("n_final_report")],
        "2026-11-02".parse().unwrap()
    );
    let answers: BTreeSet<_> = graph.state.answers.keys().cloned().collect();
    assert_eq!(
        answers,
        answered("vendor-evaluation"),
        "every decision answered, once (B2)"
    );
    let deployment = &records.deployment;
    assert!(
        deployment
            .identities
            .get(&"who_j5v2nx6cwr4tdkq7".parse().unwrap())
            .is_some(),
        "E6"
    );
    assert_eq!(
        deployment.revision.get(),
        identity_steps("vendor-evaluation"),
        "each patch that created identities moved the deployment once (E6)"
    );
}

/// A2, B10, E3: a skipped screen, and interviews that each name one explicit panelist.
#[test]
fn the_hiring_loop_ends_with_explicit_panelists() {
    let records = finished("hiring-loop");
    let graph = only_graph(&records);
    assert_eq!(state_of(graph, "n_screen"), State::Skipped);
    assert_eq!(state_of(graph, "n_interviews"), State::Done);
    let interviews: Vec<_> = graph
        .nodes
        .values()
        .filter(|node| node.parent == Some(key("n_interviews")))
        .collect();
    let broken_down = added_under("hiring-loop", "n_interviews");
    assert!(broken_down > 0, "the scenario breaks the interviews down");
    assert_eq!(interviews.len(), broken_down);
    for interview in interviews {
        let source = interview.participations.as_map().values().next();
        assert!(
            matches!(source, Some(ParticipationSource::Identities(found)) if found.len() == 1),
            "one explicit panelist each (B10)"
        );
    }
    let fills = &graph.state.role_fills;
    assert!(fills.contains_key(&"r_hiring_manager".parse().unwrap()));
}

/// F5, F6: the launch pin shifted a week, and a late actual is recorded, never rejected.
#[test]
fn the_product_launch_records_a_late_actual() {
    let records = finished("product-launch");
    let state = &only_graph(&records).state;
    assert_eq!(state.pins[&key("n_launch")], "2026-11-23".parse().unwrap());
    let freeze = &state.nodes[&key("n_code_freeze")];
    assert_eq!(freeze.state, State::Done);
    assert_eq!(freeze.finished_on, Some("2026-11-04".parse().unwrap()));
}

/// B1, I6, E6: the proposal is applied, a role is declared on the journey, and the
/// proposal's identity creates rode in its apply.
#[test]
fn the_bake_off_applies_its_proposal() {
    let records = finished("bake-off");
    let proposal = records.proposals.values().next().unwrap();
    assert_eq!(proposal.status, ProposalStatus::Applied);
    let graph = only_graph(&records);
    assert_eq!(
        graph.nodes.len(),
        added("bake-off"),
        "every node the proposal drafted"
    );
    assert!(graph.roles.get(&"r_judges".parse().unwrap()).is_some());
    assert_eq!(state_of(graph, "n_trial_a"), State::Active);
    assert_eq!(
        records.deployment.revision.get(),
        identity_steps("bake-off")
    );
}
