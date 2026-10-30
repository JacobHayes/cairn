//! The store's limits (PRACTICES, Explicit limits): hard-coded, named, and used as the
//! bound wherever a page is cut, a connection is waited for, or a subscriber is admitted or
//! handed its ticks.

use std::time::Duration;

/// Items in one page of history, the journey index, or search results: callers page.
pub use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
/// Store reader connections per process, besides the one writer: each in-flight read
/// borrows one; every write takes the writer in turn.
pub const STORE_CONNECTION_COUNT_MAX: u32 = 16;
/// How long a request waits for a store connection: commits take milliseconds, so five
/// seconds of waiting is contention, which at this scale is a bug.
pub const STORE_CONNECTION_ACQUIRE: Duration = Duration::from_secs(5);
/// Subscribers to the notifier per process (PRACTICES, Explicit limits): tens of
/// users with a few tabs each.
pub const SSE_SUBSCRIBER_COUNT_MAX: u32 = 256;
/// How often a subscriber is handed its ticks (PRACTICES, Explicit limits): it feels
/// live, at most four ticks a second per domain.
pub const SSE_COALESCING_INTERVAL: Duration = Duration::from_millis(250);
