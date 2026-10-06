//! The assistant's limits (PRACTICES, Explicit limits): hard-coded and named, each the bound
//! of the loop or wait that enforces it.

use std::time::Duration;

/// Nodes one direct assistant write may touch (I5): "mark these three done" applies at once;
/// past this a misheard request is expensive to undo, so the whole change becomes one
/// proposal.
pub const DIRECT_WRITE_NODE_COUNT_MAX: u32 = 10;

/// Provider calls in one turn: a runaway stop, not a budget. Structuring a journey is a few
/// reads, one proposal, and a report.
pub const TOOL_LOOP_ITERATION_COUNT_MAX: u32 = 32;

/// One non-streamed model response, a long proposal included; past this the provider is
/// down and the turn reports it.
pub const PROVIDER_CALL_DURATION_MAX: Duration = Duration::from_secs(120);

/// One turn, every provider call and tool call in it: the iteration limit at typical
/// provider latency, with room.
pub const TURN_DURATION_MAX: Duration = Duration::from_mins(10);

/// Turns in flight per process: each is an open request waiting on the provider, whose rate
/// limit binds well before this. Overflow is refused, to be tried again.
pub const TURN_IN_FLIGHT_COUNT_MAX: u32 = 32;

/// Messages a conversation keeps, the newest: the page limit every list that grows is held
/// to (DECISIONS.md, 4.4). Each message is a Markdown body, so a whole conversation stays
/// inside the size the store keeps one in.
pub const CONVERSATION_MESSAGE_COUNT_MAX: u32 = cairn_schema::limits::PAGE_ITEM_COUNT_MAX;

// A provider call fits in a turn, several times over.
const _: () = assert!(PROVIDER_CALL_DURATION_MAX.as_secs() * 4 <= TURN_DURATION_MAX.as_secs());
// A conversation at its cap, every message at the body cap, is stored whole.
const _: () = assert!(
    (CONVERSATION_MESSAGE_COUNT_MAX as u64) * (cairn_schema::limits::BODY_BYTES_MAX as u64 + 256)
        < cairn_schema::limits::GRAPH_BYTES_MAX as u64
);
