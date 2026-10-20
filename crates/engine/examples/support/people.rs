//! The people every fixture scenario starts from (fixtures/README.md): `people.yaml` applied
//! to an empty deployment, leaving it at revision 1.

use std::error::Error;
use std::path::Path;

use cairn_engine::{ApplyInputs, Records, apply};
use cairn_schema::{ScenarioStep, from_yaml};

/// The records of a deployment holding the shared people.
pub fn records(fixtures: &Path) -> Result<Records, Box<dyn Error>> {
    let step: ScenarioStep = from_yaml(&std::fs::read_to_string(fixtures.join("people.yaml"))?)?;
    let inputs = ApplyInputs {
        today: step.today,
        at: step.at,
        actor: step.actor.clone(),
        note: Some(step.note.clone()),
    };
    let applied = apply(&Records::default(), &step.patch, &inputs).map_err(|r| format!("{r:?}"))?;
    Ok(applied.records().clone())
}
