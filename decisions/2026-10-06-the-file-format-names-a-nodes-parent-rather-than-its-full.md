# The file format names a node's parent rather than its full path

Question: a file can give each node its full path, nest children under their parent, or give each node its id and its parent's path.

Call: a node in a file has `id` and `parent` (the parent's path, absent for a root), in a flat list, and every reference is a full path. The graph form has the same fields with `parent` a key, so both share one node type and wire shape, and parsing never recurses through containment.

Alternatives: a full path on each node (the graph form would need a different identity shape, and a node's id repeats in every child's path); nested `children` (reads as a tree, but parsing recurses and indentation runs deep).

What would change it: authors finding parent-plus-id harder to read than full paths in practice.
