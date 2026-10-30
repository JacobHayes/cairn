# One drill mechanism, expand in place, and two rules for what is open

Question: drilling into a container could open a sub-canvas or expand in place, and several rules could each decide which containers are open. Which drill model, and what decides openness?

Call: expand in place only; there is no sub-canvas or focus mode. Two rules decide what is open: the depth step (at the coarsest step only the current top-level groups, those holding acting-frontier or active work, are open; at the others every container is open to that step) and the viewer's own expands and collapses, kept in the address until a step is picked, which clears them. Collapse and the display states shown are part of the engine's level request, which rolls a collapsed or hidden subtree into its container's card under the shared roll-up rules.

Alternatives: a sub-canvas per container (a second mental model and a second set of edge rules); layered openness rules (nobody could tell which rule opened a container); collapse computed in the tab (the level read would disagree with what the canvas draws).

What would change it: journeys too large for find and fit to stand in for a focused view of one container.
