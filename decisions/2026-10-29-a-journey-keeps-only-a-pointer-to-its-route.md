# A journey keeps only a pointer to its route

- Question (from cutting route upgrades): a journey copies its route's nodes, and Cairn merged later route versions back into it with per-field local-edit markers, tombstones, orphans and re-linking. That machinery was the largest part of the engine's proposal code, for a flow no one had used. What does a journey keep of its route?
- Call: only the route and the version it was made from, shown on the journey. Nothing is merged, marked or re-linked afterwards. A journey's changes reach a route by saving it as a route, a plain copy. Stored markers and tombstones are dropped by a migration, and so are open proposals holding the removed operations and items. Saving a journey as a route no longer excludes breakdown children by default: the reviewer excludes what they do not want, as with any node.
- Alternatives: keep upgrade for journeys only (still needs markers and tombstones); keep re-link (needs markers to mean anything).
- What would change it: real use where journeys must follow a route's later versions, which would be designed from that use.
