//! The limits the tools hold their outputs to (PRACTICES, Explicit limits). A list that can
//! grow is paged at the schema's page limit, the snapshot
//! top-N and history page, and a call's duration is the API's, which the endpoint is mounted
//! under (`request duration, API and MCP`).

/// Items in one page of any list a tool answers: the frontier, a level's nodes, a node's
/// children and notes, the indexes, search, and history.
pub use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;

// The context budgets (PRACTICES, Explicit limits): what an agent loads or reads is held to
// a size, measured as compact JSON bytes (about four bytes a token), and tests fail past it.

/// Bytes in the whole tool list, every tool's name, description, input schema and
/// annotations.
pub const TOOL_LIST_BYTES_MAX: u32 = 48 * 1024;
/// Bytes in any one tool's definition, but for the mutation sketch, which `apply_patch`
/// carries once for every tool that takes mutations.
pub const TOOL_DEFINITION_BYTES_MAX: u32 = 4 * 1024;
/// Bytes in the mutation sketch: the type signatures of the whole mutation vocabulary.
pub const MUTATION_SKETCH_BYTES_MAX: u32 = 14 * 1024;
/// Bytes per listed node row in a page of nodes an agent reads, on each fixture journey.
pub const NODE_ROW_BYTES_MAX: u32 = 320;
