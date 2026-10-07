//! The assistant's limits (PRACTICES, Explicit limits): hard-coded and named, each the bound
//! of the loop or wait that enforces it.

use std::time::Duration;

/// Nodes one direct assistant write may touch (I5): "mark these three done" applies at once;
/// past this a misheard request is expensive to undo, so the whole change becomes one
/// proposal.
pub const DIRECT_WRITE_NODE_COUNT_MAX: u32 = 10;

/// Tool calls run from one model reply: a reply asking for more is answered with a refusal
/// for each call past this, so a reply cannot run past the turn's limits on tool calls
/// alone. Borrowed from the iteration limit
/// (decisions/2026-10-07-the-ten-node-limit-holds-across-a-turn-and-what-touched.md).
pub const TOOL_CALL_COUNT_PER_REPLY_MAX: u32 = TOOL_LOOP_ITERATION_COUNT_MAX;

/// Provider calls in one turn: a runaway stop, not a budget. Structuring a journey is a few
/// reads, one proposal, and a report.
pub const TOOL_LOOP_ITERATION_COUNT_MAX: u32 = 32;

/// One non-streamed model response, a long proposal included; past this the provider is
/// down and the turn reports it.
pub const PROVIDER_CALL_DURATION_MAX: Duration = Duration::from_secs(120);

/// One turn, every provider call and tool call in it: the iteration limit at typical
/// provider latency, with room.
pub const TURN_DURATION_MAX: Duration = Duration::from_mins(10);

/// What a turn keeps of its limit to save its conversation: its work ends this far before
/// the turn limit. The API's request duration, borrowed: a store write past it is stuck
/// (decisions/2026-10-07-every-wait-in-a-turn-is-held-to-the-turns-limit.md).
pub const TURN_SAVE_RESERVE: Duration = Duration::from_secs(5);

/// Turns in flight per process: each is an open request waiting on the provider, whose rate
/// limit binds well before this. Overflow is refused, to be tried again.
pub const TURN_IN_FLIGHT_COUNT_MAX: u32 = 32;

/// Messages a conversation keeps, the newest: the page limit every list that grows is held
/// to
/// (decisions/2026-10-06-conversations-are-one-per-target-per-user-keep-their-newest.md).
/// Each message is a Markdown body, so a whole conversation stays inside the size the
/// store keeps one in.
pub const CONVERSATION_MESSAGE_COUNT_MAX: u32 = cairn_schema::limits::PAGE_ITEM_COUNT_MAX;

/// Bytes of a conversation replayed to the provider each turn, the newest kept: four
/// message bodies, about 64 thousand tokens, well inside every current model's context
/// (decisions/2026-10-07-the-ten-node-limit-holds-across-a-turn-and-what-touched.md:
/// pending the owner's sign-off as a named limit).
pub const DIALOGUE_REPLAY_BYTES_MAX: u32 = 4 * cairn_schema::limits::BODY_BYTES_MAX;

/// Bytes of one provider response read: the largest body Cairn takes in a request, since a
/// reply's tool calls are bounded by what the tools accept
/// (decisions/2026-10-07-the-ten-node-limit-holds-across-a-turn-and-what-touched.md).
pub const PROVIDER_RESPONSE_BYTES_MAX: u32 = cairn_schema::limits::REQUEST_BYTES_MAX;

// A provider call fits in a turn, several times over.
const _: () = assert!(PROVIDER_CALL_DURATION_MAX.as_secs() * 4 <= TURN_DURATION_MAX.as_secs());
// A conversation at its cap, every message at the body cap, is stored whole.
const _: () = assert!(
    (CONVERSATION_MESSAGE_COUNT_MAX as u64) * (cairn_schema::limits::BODY_BYTES_MAX as u64 + 256)
        < cairn_schema::limits::GRAPH_BYTES_MAX as u64
);
