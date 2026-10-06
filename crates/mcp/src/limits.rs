//! The limits the tools hold their outputs to (PRACTICES, Explicit limits). The tools add
//! none of their own: a list that can grow is paged at the schema's page limit, the snapshot
//! top-N and history page, and a call's duration is the API's, which the endpoint is mounted
//! under (`request duration, API and MCP`).

/// Items in one page of any list a tool answers: the frontier, a level's nodes, a node's
/// children and notes, the indexes, search, and history.
pub use cairn_schema::limits::PAGE_ITEM_COUNT_MAX;
