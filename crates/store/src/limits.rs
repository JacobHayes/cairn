//! The store's limits (PRACTICES, Explicit limits): hard-coded, named, and used as the
//! bound wherever a page is cut or a connection is waited for.

use std::time::Duration;

/// Items in one page of history, the journey index, or search results: callers page.
pub const PAGE_ITEM_COUNT_MAX: u32 = 200;
/// Store connections per process: each in-flight request borrows one for its load or
/// commit.
pub const STORE_CONNECTION_COUNT_MAX: u32 = 16;
/// How long a request waits for a store connection: commits take milliseconds, so five
/// seconds of waiting is contention, which at this scale is a bug.
pub const STORE_CONNECTION_ACQUIRE: Duration = Duration::from_secs(5);
