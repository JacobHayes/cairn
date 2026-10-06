//! The validation pipeline (ARCHITECTURE, Write path: apply): stages that run once, in order,
//! on the candidate the whole patch produced, each in its own module, each collecting every
//! violation it finds (A15). The list grows as the engine does: the stages that need derived
//! state (the plan check, relevance and `deps_done` guards) join it as their own modules, and
//! nothing runs a subset of it: `apply` iterates the whole list.

mod deployment;
mod entities;
mod graphs;
mod guards;

use std::collections::{BTreeMap, BTreeSet};

use cairn_schema::{GuardFailure, NodeKey, Violation};

use crate::mutate::Session;

/// What the stages read and what they find.
pub(crate) struct Check<'a, 'patch> {
    /// The apply in progress, with the candidate the patch produced.
    pub session: &'a Session<'patch>,
    /// Every violation found.
    pub violations: Vec<Violation>,
    /// The guard failures each bypass in the patch accepts (D4), by node.
    pub bypassed: BTreeMap<NodeKey, BTreeSet<GuardFailure>>,
}

/// A validation stage.
pub(crate) type Stage = fn(&mut Check<'_, '_>);

/// The stages, in order: every graph the patch wrote holds the structural and state
/// invariants; entity references resolve; the deployment's aliases and emails hold; and the
/// guards of the transitions the patch made pass, or a bypass in the patch accepts them.
pub(crate) const STAGES: [Stage; 4] = [
    graphs::check,
    entities::check,
    deployment::check,
    guards::check,
];

/// Runs every stage on the session's candidate.
#[must_use]
pub(crate) fn run<'a, 'patch>(session: &'a Session<'patch>) -> Check<'a, 'patch> {
    let mut check = Check {
        session,
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
