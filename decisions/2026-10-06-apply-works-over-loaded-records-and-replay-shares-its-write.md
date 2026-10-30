# Apply works over loaded records, and an event's delta is the writes it made

Question: a patch can read more than its domain (a journey create copies a route version, an identity create rides in a journey patch, applying a proposal reads it, a merge checks the journeys that reference its identities), and replay must equal apply exactly, for a store that does not depend on the engine.

Call: `apply` takes `Records`, everything the host loaded for the patch, and each mutation turns into writes in one record vocabulary shared with the store: put a record's after-state, remove an address (which may cover a node, a graph or a domain, and takes everything under it), or copy a graph. The candidate is built by applying exactly those writes with the store's semantics in memory, and an event's delta is the list of writes its mutation made, side effects included (a snooze a transition clears, a bypass a reopen clears). Replay applies the same writes and re-derives nothing, so replay equals apply by construction. The memory store is the reference for what each write means, and the conformance suite holds every backend to it.

Alternatives: a typed delta per event type (replay would re-implement each mutation and must match apply exactly); full graph snapshots as deltas (exact but large); per-record removals only (a missed row is an orphan).

What would change it: a store whose rows need writes this vocabulary cannot express.
