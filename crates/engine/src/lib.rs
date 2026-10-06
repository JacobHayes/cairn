//! The pure engine (ARCHITECTURE, Engine): the graph model, the one write path (`apply`),
//! the per-kind state machines, events, and replay. No I/O: the clock, the actor, and every
//! record a patch reads arrive as arguments, and an accepted patch comes back as a change
//! set for the host to commit.
#![forbid(unsafe_code)]

pub mod derive;
mod edit;
mod entity;
pub mod file;
pub mod graph;
mod mutate;
pub mod pipeline;
pub mod records;
pub mod replay;
mod stages;
#[cfg(feature = "testing")]
pub mod testing;
pub mod transition;
mod validate;

pub use derive::{Derived, derive};
pub use file::from_file;
pub use graph::{Document, Graph, Tree};
pub use pipeline::{Applied, ApplyInputs, apply};
pub use records::Records;
pub use replay::replay;
