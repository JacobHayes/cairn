# The file format names a node's parent rather than its full path

- Question: PRD Identity and references says paths are how files refer to nodes. A file could give each node its full path, or its id and its parent's path.
- Call: a node in a file has `id` and `parent` (the parent's path, absent for a root), and every reference (`requires`, conditions, rules, stage bounds, `feeds_milestone`) is a full path. The graph form has the same three fields with `parent` a key, so the two forms share one node type and one wire shape (ARCHITECTURE, File format: one schema). Nodes are a flat list, so parsing never recurses through containment (PRACTICES, No recursion); a canonical file lists them as written, and export (2.7) sorts.
- Alternatives: `path: setup/access` on each node (the path is visible at a glance, but the graph form needs a different identity shape, and a node's id is repeated in its children's paths); nested `children:` (reads as a tree, but recursion in parsing and deep indentation at depth 16).
- What would change it: authors finding parent-plus-id harder to read than full paths in practice.
