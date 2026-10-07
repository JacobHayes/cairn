# The store's lists page by the history page limit

- Question: PRACTICES (Explicit limits) names a page size only for history and the snapshot (200), but the journey index, text search, the auth log, and a user's conversations are lists that grow with use.
- Call: every list the store returns pages at 200 items (`PAGE_ITEM_COUNT_MAX` in `crates/store/src/limits.rs`, beside the store connection limits), with a cursor; no new limit. Hits within one journey's search result are bounded by the graph's own cap. A conversation is stored whole and held to the 16 MiB graph cap.
- Alternatives: new named limits for each list (sign-off needed); unbounded lists.
- What would change it: an index that needs larger pages, or a conversation that outgrows the cap.
