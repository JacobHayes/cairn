# The schema's journey row and graph registry

- Question: ARCHITECTURE's Schema outline put a journey's fields and revision on its `graphs` row. A commit's first write is its domain's revision row, and a journey's fields arrive with its first commit, so the row a commit locks must exist (or be created) before the graph rows a commit writes.
- Call: `journeys` holds the journey domain row (fields and revision), as `routes` does for a route, and `graphs` is the registry every content and state row belongs to; drafts and versions have their own small tables; the entities an answer, role fill, or participation names are rows of their own, which is how the journeys referencing an entity are found (E6); events have an `event_nodes` index for a node's history. ARCHITECTURE's outline is updated to match.
- Alternatives: the outline as written, with nullable journey fields on `graphs` (CHECK constraints could not hold them).
- What would change it: nothing expected; the layout is the implementer's within the outline.
