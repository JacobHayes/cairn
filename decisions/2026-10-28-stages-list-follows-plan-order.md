# The Stages list runs in plan order

Question: in what order does a journey card's Stages list show the stages?

Call: the order the engine plans the nodes in (the Plan list's order): by the soonest date on or beneath each stage, stages with none after, key order between equal ones. The card is drawn on every view, including ones with no canvas, and the engine already says one order for every surface, so the card reads it instead of ordering stages itself.

Alternatives: the canvas layout order (a store the canvas writes and the card reads, empty on views without a canvas); ordering by each stage's due date in the card (a second copy of the date ordering).

What would change it: a view model that exposes the stages' laid-out order to every view.
