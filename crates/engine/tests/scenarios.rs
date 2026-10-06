//! Every fixture scenario runs through apply (J1, J2, A17).
#![cfg(test)]

mod support;

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
