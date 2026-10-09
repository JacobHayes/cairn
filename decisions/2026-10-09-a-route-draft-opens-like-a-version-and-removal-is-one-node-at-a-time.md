# A route draft opens like a version, and removing a selection confirms one node at a time

Question: the authoring and proposal review in the frame (the unit that moved route drafts, Edit
structure and review into the Plan frame) had to settle two things its brief left open: where a
route draft's ladder opens, and what Remove does to several selected nodes.

Call:
- **A route draft opens at the same step as a version.** A route has no current stage, so it
  opens at Stages, or at Decisions when it has no groups, drafted or not: a draft that opened
  at All would draw every node as a card too small to read. Adding a node moves the ladder to the
  first step that draws its kind (and selects the node), so it is always on screen. The step
  is still the viewer's to pick, and it rides in the address.
- **Remove in the selection bar confirms one node at a time**, each with its cascade shown
  and previewed, the outermost selected nodes first. A removal's rewrites are computed from the
  graph as it stands, so merging several into one patch could rewrite one reference twice; one
  patch each cannot.

Alternatives: opening a draft at All (the first screen is a crowd of truncated cards, against the
approachability call); one merged patch for the whole selection (shorter, but
two removals sharing a reference could undo each other's rewrite).

What would change it: a merged cascade built in the engine (one removal naming several roots)
would make the selection a single confirmation.
