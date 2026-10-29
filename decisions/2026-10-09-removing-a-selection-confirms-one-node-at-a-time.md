# Removing a selection confirms one node at a time

Question: authoring and proposal review had to settle what Remove does to several selected nodes.

Call: Remove in the inspector confirms one node at a time, each with its cascade shown and previewed, the outermost selected nodes first. A removal's rewrites are computed from the graph as it stands, so merging several into one patch could rewrite one reference twice; one patch each cannot.

Alternatives: one merged patch for the whole selection (shorter, but two removals sharing a reference could undo each other's rewrite).

What would change it: a merged cascade built in the engine (one removal naming several roots) would make the selection a single confirmation.
