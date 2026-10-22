//! The pure engine (ARCHITECTURE, Engine): the graph model, the one write path (`apply`),
//! the per-kind state machines, derive and its projections, events, and replay. No I/O: the
//! clock, the actor, and every record a patch reads arrive as arguments, and an accepted
//! patch comes back as a change set for the host to commit.
#![forbid(unsafe_code)]

mod cascade;
pub mod derive;
mod edit;
pub mod format;
pub mod graph;
mod identity;
mod insertion;
mod mutate;
mod notices;
pub mod pipeline;
pub mod project;
pub mod proposal;
pub mod records;
pub mod replay;
pub mod rules;
mod stages;
#[cfg(feature = "testing")]
pub mod testing;
pub mod transition;
pub mod upgrade;
mod validate;

pub use cascade::plan_removal;
pub use derive::{Derived, check_plan, consequences, derive};
pub use format::{export, from_file, import};
pub use graph::{Document, Graph, Tree};
pub use notices::notices;
pub use pipeline::{Applied, AppliedAt, ApplyInputs, apply, apply_over, touched};
pub use project::{DerivedJourney, DraftContext, ProjectionError, history};
pub use proposal::{
    Candidate, candidate, destination_after, preview, preview_of, resolve, resolve_partial,
};
pub use records::Records;
pub use replay::replay;
pub use upgrade::{DraftError, relink, save_as_route, upgrade};
