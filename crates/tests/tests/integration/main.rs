//! Every library crate's integration tests, built as one test binary so the workspace's
//! dependencies are linked once rather than once per crate (and each fresh executable is
//! scanned once on its first run). Each directory beside this one is the tests of the crate it
//! is named for, and each file in it a module; a test is named `<crate>::<file>::<test>`, and a
//! new test file is declared in its crate's `mod.rs` as `mod <file>;`. Rungs pick tests by
//! module path: `property::` (rung 3), `conformance::` and `in_process::` (rung 4), `cost::`
//! (rung 7), everything else in rung 2; scripts/ladder-tests counts each test for its crate.
//! The tests that run a binary of their own crate (`cairn`, `cairn-schema`) stay in those crates,
//! since Cargo only names a binary to the integration tests of its own package.

// A file's tests sit in a module named for that file or for the rung that runs them, so
// the path often repeats a name (`notifier::notifier::`).
#![allow(clippy::module_inception)]

mod api;
mod assistant;
mod auth;
mod engine;
mod mcp;
mod service;
mod store;
mod store_turso;
mod wasm;
