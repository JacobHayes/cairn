//! The binary's library (ARCHITECTURE, Build, run, deploy; Service layer and composition):
//! the configuration, the composition root that assembles the server from it, the listener,
//! the `Host` allowlist in front of the whole router, the embedded web build, and the
//! structured logs and metrics. `main.rs` is the command line over it; tests assemble the
//! same server in process.

#![forbid(unsafe_code)]

pub mod assets;
pub mod config;
pub mod host;
pub mod limits;
pub mod listener;
pub mod root;
pub mod telemetry;
