//! The binary's library (ARCHITECTURE, Build, run, deploy; Service layer and composition):
//! the configuration, the demo's extras, the composition root that assembles the server from it, the listener,
//! the `Host` allowlist in front of the whole router, the embedded web build, and the
//! structured logs and metrics. `main.rs` is the command line over it; tests assemble the
//! same server in process.

#![forbid(unsafe_code)]

pub mod assets;
pub mod config;
pub mod demo;
pub mod host;
pub mod limits;
pub mod listener;
pub mod root;
pub mod telemetry;
pub mod version;

/// The version this build reports (`cairn version`, MCP `serverInfo`): the release tag it was
/// built from with `CAIRN_VERSION` set, else `dev` (build.rs).
pub const VERSION: &str = env!("CAIRN_BUILD_VERSION");
