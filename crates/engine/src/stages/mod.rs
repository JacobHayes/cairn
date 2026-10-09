//! The validation pipeline (ARCHITECTURE, Write path: apply): stages that run once, in order,
//! on the candidate the whole patch produced, each in its own module, each collecting every
//! violation it finds (A15). Nothing runs a subset of it: `apply` iterates the whole list.
//! The last stage derives the final candidate once and checks what needs derived state: the
//! relevance and `deps_done` guards and where a snooze may sit (D4, B6).

mod deployment;
mod derived;
mod entities;
mod graphs;
mod guards;
mod segments;

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{Event, GuardFailure, NodeKey, Violation};

use crate::mutate::Session;

/// What the stages read and what they find.
pub(crate) struct Check<'a, 'patch> {
    /// The apply in progress, with the candidate the patch produced.
    pub session: &'a Session<'patch>,
    /// One event per mutation so far, each with the writes it made: what names the mutations
    /// that caused a derived failure.
    pub events: &'a [Event],
    /// The patch's journey graph holds every graph invariant, so derive may run on it.
    pub journey_valid: bool,
    /// Every violation found.
    pub violations: Vec<Violation>,
    /// The guard failures each bypass in the patch accepts (D4), by node.
    pub bypassed: BTreeMap<NodeKey, BTreeSet<GuardFailure>>,
}

/// A validation stage.
pub(crate) type Stage = fn(&mut Check<'_, '_>);

/// The stages, in order: every graph the patch wrote holds the structural and state
/// invariants; a segment's graphs hold its kind's rules; entity references resolve; the deployment's aliases and emails hold; the
/// guards of the transitions the patch made pass, or a bypass in the patch accepts them,
/// first those that need no derived state, then those that do, with the snoozes it made.
pub(crate) const STAGES: [Stage; 6] = [
    graphs::check,
    segments::check,
    entities::check,
    deployment::check,
    guards::check,
    derived::check,
];

/// Runs every stage on the session's candidate; `events` are the patch's, one per mutation.
#[must_use]
pub(crate) fn run<'a, 'patch>(
    session: &'a Session<'patch>,
    events: &'a [Event],
) -> Check<'a, 'patch> {
    assert!(events.len() <= session.patch.mutations.len());
    let mut check = Check {
        session,
        events,
        journey_valid: true,
        violations: Vec::new(),
        bypassed: BTreeMap::new(),
    };
    for stage in STAGES {
        let found_before = check.violations.len();
        stage(&mut check);
        assert!(
            check.violations.len() >= found_before,
            "a stage only adds violations"
        );
    }
    // A bypass records failures only for nodes it was applied to in this patch (D4).
    assert!(
        check
            .bypassed
            .keys()
            .all(|node| session.bypassed.contains_key(node))
    );
    check
}
