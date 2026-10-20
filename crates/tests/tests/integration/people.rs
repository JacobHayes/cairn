//! The people every fixture scenario starts from (fixtures/README.md): one deployment
#![cfg(test)]

//! patch, applied before a scenario's first step.

use std::path::Path;

use cairn_schema::{ScenarioStep, from_yaml};

/// The step that creates the shared people, at deployment revision 0; the deployment is at
/// revision 1 afterwards.
pub fn people_step() -> ScenarioStep {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/people.yaml");
    from_yaml(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// The people's keys, for tests that name one: Ana Ruiz, Ben Okafor, Chen Wei, Dara Singh,
/// Eli Park, Fatima Noor, Gus Lindqvist, Hana Kimura, and Farid Haddad (created by the
/// vendor evaluation's own patch).
#[allow(dead_code)]
pub mod keys {
    pub const ANA: &str = "who_k3d7qm2xw5tn6pfa";
    pub const BEN: &str = "who_b6r2vz4jhs7cnqe3";
    pub const CHEN: &str = "who_c5y3tw7ldp2aoxgm";
    pub const DARA: &str = "who_d4m6uk2eqf5rbhzs";
    pub const ELI: &str = "who_e7h2jv3nxc4yaw6t";
    pub const FATIMA: &str = "who_f2q5nr7xbd3kwzh4";
    pub const GUS: &str = "who_g6t4ycm2pe7vsjan";
    pub const HANA: &str = "who_h3w7bs5xkq2dfnr6";
    pub const FARID: &str = "who_j5v2nx6cwr4tdkq7";
}

/// The people as a write: submit it once to a fresh service before a scenario's first step.
/// A second submission is answered from its receipt.
pub fn people_write() -> (cairn_service::Call, cairn_service::DomainPatch) {
    let step = people_step();
    let call = cairn_service::Call {
        actor: step.actor.clone(),
        now: step.at,
    };
    (
        call,
        cairn_service::DomainPatch::new(step.patch, Some(step.note)).unwrap(),
    )
}
