//! The crate's integration tests, built as one test binary so its dependencies are linked
//! once rather than once per file. Each file beside this one is a module and its tests are
//! named `<file>::<test>`; a new test file goes here too, added below as `mod <file>;`.
//! The steps of the validation ladder pick tests by module path: `property::`, `cost::`,
//! `conformance::` and `in_process::`, `binary::binary::` (each its own step), everything else
//! in the unit step.

// A file's tests sit in a module named for that file or for the step that runs them, so
// the path often repeats a name (`notifier::notifier::`).
#![allow(clippy::module_inception)]

#[cfg(test)]
mod support;

mod binary;
mod server;
