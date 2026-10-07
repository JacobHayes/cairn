//! The crate's integration tests, built as one test binary so its dependencies are linked
//! once rather than once per file. Each file beside this one is a module and its tests are
//! named `<file>::<test>`; a new test file goes here too, added below as `mod <file>;`.
//! Rungs pick tests by module path: `property::` and `cost::` (rung 3), `conformance::` and
//! `in_process::` (rung 4), `binary::binary::` (rung 6), everything else in rung 2.

// A file's tests sit in a module named for that file or for the rung that runs them, so
// the path often repeats a name (`notifier::notifier::`).
#![allow(clippy::module_inception)]

mod conformance;
mod notifier;
