# A route's version is picked in its header

Question: where does a person say which version of a route they are looking at, and what does the bare address mean?

Call: one page per route, with a version selector in its header listing the draft (when open) and every published version. Picking keeps the view and the view state. The bare address floats: the draft when one is open, else the latest version. A link that names a version always carries `?version=<n>`, so a saved link keeps meaning that version after newer ones are published. A node address names a node of the graph shown; a version's node opens read-only.

Alternatives: separate pages for the draft and each version (several frames to keep alike); a tab row of versions (does not scale); a pinned bare address (a link would silently change meaning when a draft opens).

What would change it: routes with many versions, which may need a search in the selector.
