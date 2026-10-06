//! Journey scenarios (fixtures/): a journey built step by step from patches at a fixed
//! clock, the shared input for engine scenario tests, the in-browser host, and server
//! integration tests (ARCHITECTURE, Testing the engine).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::collections::{BoundedVec, ByDocumentSize};
use crate::event::Actor;
use crate::id::JourneyId;
use crate::patch::Patch;
use crate::text::Markdown;
use jiff::Timestamp;
use jiff::civil::Date;

/// One step: a patch, who submits it, and the clock it is applied at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioStep {
    /// What the step does, for people reading the fixture.
    pub note: Markdown,
    /// Today, in the deployment's time zone, when the step applies (the derive input).
    pub today: Date,
    /// When it commits.
    pub at: Timestamp,
    /// Who submits it.
    pub actor: Actor,
    /// The patch.
    pub patch: Patch,
}

/// A journey scenario: its steps in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// The journey the steps build.
    pub journey: JourneyId,
    /// What the scenario walks through.
    pub description: Markdown,
    /// The steps.
    pub steps: BoundedVec<ScenarioStep, ByDocumentSize>,
}
