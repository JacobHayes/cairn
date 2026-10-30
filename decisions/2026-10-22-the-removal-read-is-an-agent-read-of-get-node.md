# The removal read lists only what a patch is refused for

Question: an agent needs the exact removal for a node and what else the same patch must change. Where does the read live, and what counts as an outside reference?

Call: the engine projects it from the same removal plan the app uses, and `get_node` returns it when `removal` is true, never in an ordinary answer. The REST API serves it too, because every service operation has an endpoint. Outside references are the places a patch is refused for if left alone: conditions, date rules, fed milestones, message drafts and snooze targets, paged with a cursor; the removal itself is always whole. A role filled by a removed decision is not listed, since the role stays valid unfilled.

Alternatives: a separate tool for one lookup; listing role fills (noise: nothing is refused).

What would change it: a screen that previews a removal would use the REST read.
