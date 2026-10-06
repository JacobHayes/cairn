//! The API's limits (PRACTICES, Explicit limits): hard-coded and named, each the bound of
//! the layer or loop that enforces it. The request body cap lives with the schema, whose
//! parses check it too; the subscriber limit and coalescing interval live with the notifier.

use std::time::Duration;

pub use cairn_schema::limits::REQUEST_BYTES_MAX;
pub use cairn_store::limits::{SSE_COALESCING_INTERVAL, SSE_SUBSCRIBER_COUNT_MAX};

/// How long an API request may take before it is answered as timed out (not the SSE
/// stream, whose response starts at once): typical requests take tens of milliseconds, the
/// heaviest about a second, so past five something is stuck.
pub const REQUEST_DURATION_MAX: Duration = Duration::from_secs(5);

/// Requests in flight per process: tens of users with a few tabs and agents; past this a
/// client is looping. Overflow is answered 503 with `Retry-After`.
pub const REQUEST_IN_FLIGHT_COUNT_MAX: u32 = 128;

/// How long an SSE subscriber may leave a write untaken before it is disconnected: it holds
/// the latest revision per domain, not a queue, so it reconnects with current revisions
/// and misses nothing (H6).
pub const SSE_WRITE_STALL: Duration = Duration::from_secs(15);

/// How often an idle SSE stream sends a comment, so proxies keep it open, a client can tell
/// a quiet stream from a dead one, and a subscriber that went away is noticed and dropped.
/// Not a limit: well inside the write stall, so a stalled peer is caught within one.
pub const SSE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

// A heartbeat is a write: a subscriber that takes none for the stall is gone.
const _: () = assert!(SSE_HEARTBEAT_INTERVAL.as_secs() < SSE_WRITE_STALL.as_secs());
