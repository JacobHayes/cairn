//! Proposal documents (I6, C14): resolving review items into the mutations a proposal
//! applies, and previewing what they would do.

mod preview;
mod resolve;

pub use preview::{Candidate, candidate, destination_after, preview, preview_of};

pub(crate) use resolve::item_node;
pub use resolve::{Resolved, resolve, resolve_partial};
