//! The binary's limits (PRACTICES, Explicit limits). It names none of its own: each bound
//! it enforces is one the table already names, borrowed where the table does not name the
//! value (DECISIONS.md, 1.2: values the limits table does not name take the nearest named
//! limit).

use std::time::Duration;

pub use cairn_api::limits::{REQUEST_DURATION_MAX, SSE_HEARTBEAT_INTERVAL, SSE_WRITE_STALL};
pub use cairn_store::limits::SSE_COALESCING_INTERVAL;

/// How long data sent on a connection may go unacknowledged before the kernel closes it
/// (`TCP_USER_TIMEOUT`): the SSE write stall, so a subscriber that stops taking writes is
/// disconnected at the socket, not only once its buffers fill (DECISIONS.md, 4.2: where the
/// SSE write stall is enforced).
pub const UNACKNOWLEDGED_DURATION_MAX: Duration = SSE_WRITE_STALL;

/// How long a connection stays idle before TCP keepalive probes it: the write stall, so an
/// idle connection to a peer that vanished is noticed like a stalled one.
pub const KEEPALIVE_IDLE: Duration = SSE_WRITE_STALL;

/// The gap between keepalive probes: the SSE heartbeat interval. An unanswered probe past
/// `UNACKNOWLEDGED_DURATION_MAX` closes the connection.
pub const KEEPALIVE_INTERVAL: Duration = SSE_HEARTBEAT_INTERVAL;

/// How long a stopping server waits for requests in flight before it exits: the request
/// duration, which bounds every request but the SSE streams (which never end by themselves)
/// and the assistant's turns (whose writes each commit whole or not at all).
pub const SHUTDOWN_DRAIN_MAX: Duration = REQUEST_DURATION_MAX;

/// How often the metrics exporter folds its histograms (the Prometheus exporter's upkeep):
/// the heartbeat interval, since a scrape every few seconds is the finest anyone reads.
pub const METRICS_UPKEEP_INTERVAL: Duration = SSE_HEARTBEAT_INTERVAL;
