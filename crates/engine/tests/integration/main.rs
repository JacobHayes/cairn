//! The crate's integration tests, built as one test binary so its dependencies are linked
//! once rather than once per file. Each file beside this one is a module and its tests are
//! named `<file>::<test>`; a new test file goes here too, added below as `mod <file>;`.
//! Rungs pick tests by module path: `property::` and `cost::` (rung 3), `conformance::` and
//! `in_process::` (rung 4), `binary::binary::` (rung 6), everything else in rung 2.

// A file's tests sit in a module named for that file or for the rung that runs them, so
// the path often repeats a name (`notifier::notifier::`).
#![allow(clippy::module_inception)]

#[cfg(test)]
mod support;

mod apply;
mod blocking;
mod consequences;
mod cost_blocking;
mod cost_dates;
mod cost_priority;
mod cost_projections;
mod cost_removal;
mod cost_upgrade;
mod dates;
mod dependencies;
mod display_state;
mod document;
mod domains;
mod explanations;
mod fixture_readme;
mod format;
mod graph;
mod guards;
mod history;
mod level;
mod limits;
mod lists;
mod matrix;
mod participation;
mod plan;
mod priority;
mod property_apply;
mod property_blocking;
mod property_dates;
mod property_dependencies;
mod property_derive;
mod property_priority;
mod property_projections;
mod property_upgrade;
mod proposals;
mod rank;
mod relevance;
mod save_relink;
mod scenarios;
mod skip;
mod snapshot;
mod state_machines;
mod trace;
mod upgrade;
mod views;
