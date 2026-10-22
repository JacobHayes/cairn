# The removal read lists only what a patch is refused for

Question: agents need the exact removal for a node and what else the same patch must
change (the agent guide work). Where does the read live, and what counts as an outside
reference?

Call: the engine projects it (`DerivedJourney::removal`, built on the existing full
removal), the service exposes it, and the agent tool `get_node` returns it when `removal`
is true, instead of the node, never in an ordinary answer (paged by `references_cursor`).
The REST API serves the same read at `nodes/{key}/removal`, whole, because every service
operation has an endpoint; the browser does not use it. Outside references are the
places a patch is refused for if left alone: conditions, date rules, stage bounds, fed
milestones, message drafts, and snooze targets. Edges are already in the removal. A
role filled by a removed decision is not listed, because the role stays valid unfilled.

Alternatives: a separate tool (a thirty-fourth tool for one lookup); listing role fills (noise: nothing
is refused).

Would change it: a screen that removes nodes with a preview would use the REST read.
