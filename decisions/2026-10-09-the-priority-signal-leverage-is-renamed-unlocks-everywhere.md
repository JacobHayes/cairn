# The priority signal leverage is renamed unlocks everywhere

Question (from the redesign's open questions, `2026-10-09-the-redesigns-open-questions-are-settled.md`, call 2): that call kept `leverage` in the PRD, API, and MCP while the UI said "Unblocks". The user chose a name that says what the number counts, so should the signal have one name on every surface?

Call: `unlocks`, a noun for what completing a node unlocks (a count with its weighted value), in the PRD, the engine, the wire and openapi, the MCP tool text, the `rank.unlocks` config key, the web app, and its sort and column addresses. The cut-over keeps no alias: nobody else deploys Cairn, the production config does not set the key, and nothing stored holds the name (derived values are computed on read), so an old `rank.leverage` key is now rejected as unknown and an old `sort=leverage` address falls back to the default sort.

`unlocks` is not the `unlocked` list of a write's consequences, which names the nodes that write brought onto the acting frontier; the glossary says so. The graph trace's "unblocks" (everything downstream of a selected node) keeps its name: it is a relation, not this signal.

Alternatives: keeping `leverage` on the wire with a friendlier UI word (rejected earlier, and agents kept echoing the old word to people); keeping `leverage` as a deprecated alias (nothing depends on it).

What would change it: people reading "unlocks" as the consequences' `unlocked` list.
