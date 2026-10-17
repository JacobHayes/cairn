//! Every fixture scenario runs through apply (J1, J2, A17), each step replayed (J3, in
//! `support::apply`), and ends in the state the fixture describes.
#![cfg(test)]

use crate::engine::support;

use cairn_schema::{Graph, ParticipationSource, ProposalStatus, State};
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

fn only_graph(records: &cairn_engine::Records) -> &Graph {
    &records.journeys.values().next().unwrap().graph
}

fn state_of(graph: &Graph, node: &str) -> State {
    graph.state.nodes[&key(node)].state
}

/// B1, B6, B10, E6: the placeholder is broken down into the children it names, completing
/// the baseline lifted its snooze, the final report is pinned, and an entity is created and
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
    assert!(children > 0, "the placeholder was broken down (B10)");
    assert!(graph.state.snoozes.is_empty(), "B6");
    assert_eq!(
        graph.state.pins[&key("n_final_report")],
        "2026-11-02".parse().unwrap()
    );
    let entities = &records.deployment.entities;
    assert!(entities.get(&"e_reviewer".parse().unwrap()).is_some(), "E6");
}

/// A2, B10, E3: a skipped screen, and interviews that each name one explicit panelist.
#[test]
fn the_hiring_loop_ends_with_explicit_panelists() {
    let records = finished("hiring-loop");
    let graph = only_graph(&records);
    assert_eq!(state_of(graph, "n_screen"), State::Skipped);
    assert_eq!(state_of(graph, "n_interviews"), State::Done);
    let interviews = graph
        .nodes
        .values()
        .filter(|node| node.parent == Some(key("n_interviews")));
    for interview in interviews {
        let source = interview.participations.as_map().values().next();
        assert!(
            matches!(source, Some(ParticipationSource::Entities(found)) if found.len() == 1),
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
    assert_eq!(freeze.state, State::Reached);
    assert_eq!(freeze.finished_on, Some("2026-11-04".parse().unwrap()));
}

/// B1, I6, E6: the proposal is applied, a role is declared on the journey, and the
/// proposal's entity creates rode in its apply.
#[test]
fn the_bake_off_applies_its_proposal() {
    let records = finished("bake-off");
    let proposal = records.proposals.values().next().unwrap();
    assert_eq!(proposal.status, ProposalStatus::Applied);
    let graph = only_graph(&records);
    assert!(graph.roles.get(&"r_judges".parse().unwrap()).is_some());
    assert_eq!(state_of(graph, "n_trial_a"), State::Active);
    assert_eq!(records.deployment.revision.get(), 1);
}
