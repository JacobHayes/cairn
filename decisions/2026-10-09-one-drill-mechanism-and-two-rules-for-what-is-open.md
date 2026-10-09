# One drill mechanism, expand in place, and two rules for what is open

Question (from the web redesign, PRD C2, C4, C5): C4 allows a sub-canvas or expand in place, and an early design layered five rules deciding which containers were open (the step, current stages, finished stages folding, a node-count threshold, per-container overrides). Which drill model, and what decides openness?

Call: expand in place only; there is no sub-canvas or focus mode with boundary stubs. A container expands one step with its plus control, a double-click, or Enter, collapses with minus, and an expand refits to it. Two rules decide what is open: the step (at Stages, current stages, those holding an acting-frontier or active node, open to Work and are labeled current; at other steps every container is open to that step) and the viewer's clicks, kept in the address (`open=`, `shut=`) until a step is picked, which clears them. The engine's level request carries the collapsed containers and the display states shown, and rolls a collapsed subtree into its card under C2's rules (re-targeted edges, merged duplicates, the hidden-prerequisites marker).

Alternatives: a sub-canvas per container (a second mental model and a second set of edge rules); the five-rule layering (nobody could tell which rule opened a stage); collapse computed in the tab (the level API would disagree with what the canvas draws).

What would change it: journeys too large for find plus fit to stand in for a focused view of one stage.
