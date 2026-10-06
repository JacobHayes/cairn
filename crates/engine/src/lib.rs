//! The pure engine (ARCHITECTURE, Engine): the graph model, the one write path (`apply`),
//! the per-kind state machines, derive and its projections, events, and replay. No I/O: the
//! clock, the actor, and every record a patch reads arrive as arguments, and an accepted
//! patch comes back as a change set for the host to commit.
#![forbid(unsafe_code)]

pub mod derive;
mod edit;
mod entity;
pub mod format;
pub mod graph;
mod mutate;
pub mod pipeline;
pub mod project;
pub mod proposal;
pub mod records;
pub mod replay;
mod stages;
#[cfg(feature = "testing")]
pub mod testing;
pub mod transition;
pub mod upgrade;
mod validate;

pub use derive::{Derived, check_plan, consequences, derive};
pub use format::{export, from_file, import};
pub use graph::{Document, Graph, Tree};
pub use pipeline::{Applied, ApplyInputs, apply};
pub use project::{DerivedJourney, DraftContext, ProjectionError, history};
pub use proposal::{preview, resolve, resolve_partial};
pub use records::Records;
pub use replay::replay;
pub use upgrade::{DraftError, relink, save_as_route, upgrade};
