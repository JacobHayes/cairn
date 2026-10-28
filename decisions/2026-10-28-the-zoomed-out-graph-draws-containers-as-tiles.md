# The zoomed-out graph draws containers as tiles

Question: zoomed out, a complex graph is all small cards and reads as intimidating. How does the canvas show structure there without moving anything (work: the graph overview, tiles)?

Call: below the reading zoom (60%, the near zoom, the same for every journey) a container is drawn as a tile at its own laid-out box: one sheet carrying its title, state chip, progress, due date, exceptional message and at most one fact (what is selected inside it, or how many search matches it holds). It opens in place at the reading zoom, or earlier when everything directly inside it is itself a container at least 120 x 40 px on screen. One rule for every container at every level, decided by the zoom alone; a pan never changes it and the layout is untouched. Cards outside every container, milestones included, are drawn whole at every zoom. This narrows the zoomed-out-cards decision: cards stay whole; containers become tiles.

Details settled while building:
- Short of room, pieces give way in the order fact, date, chip, progress words (with the exceptional message), meter. The title goes last; with no room for its first word the tile is a bare sheet whose name stays in its tooltip and accessible name. The chip sits beside the title unless that costs the title a line, then on a row of its own.
- A line with an end inside a tile is drawn to the tile along the outer part of one stably chosen member's route (strongest kind, then id); lines between the same two drawn ends are one, in the strongest member's style, and its card lists every member. The card is found for any two cards that hold others and lists every link between them, so the address does not depend on the zoom: at a zoom where a direct line and a line between their children are drawn apart, the card still lists both, since a card that changed its list as the viewer zoomed would make a shared address say two things. A card is chosen from the tile pair's own line first, then the exact one.
- A tile holding a traced, lit or proposal-marked card is in focus as that card would be, and carries the rank, trace and Signal tags of the cards it holds; its search count is the cards inside it that match themselves, a matching container included (the level says which nodes match and which only hold a match).
- Double-click, double-tap and Enter on a container zoom into it, framed at least at its open zoom so it is drawn open; folding stays on the fold control. A collapsed container has nothing to zoom into, so those gestures still expand it.

Alternatives: one global threshold (a big stage zoomed into shows as a blank tile until 60%); opening by size alone (puts unreadable cards back on screen for long flat containers); a compact stage layout cross-faded at a zoom (positions move); a title-only card level (a second card look).

What would change it: a flicker when a pinch hovers at a threshold (a small hysteresis band would be added), or a graph so large that rebuilding node data on each flip is measurably slow.

## Date lines on a very large canvas

A date line is routed round the tiles as it is round any card: a tile is a plain card in the route search, never a frame. The search grid has a fixed point budget, which the largest programs at their fit exceed, and the line then fell back to a plain elbow that can cross a tile. The router now shrinks the cards' boxes until the coarsest grid covers them, searches there, and grows the routes back, so the budget is unchanged and a line still goes round. The gutters and stubs shrink with the boxes, which at that distance is a few pixels on screen. What would change it: if lines at that scale read as cramped, a grid over only the drawn tiles and cards.

## The cost of a flip at the node limit

A flip is one re-render per band crossing, never per frame: `bandOf` is constant within a band. On a synthetic graph at the node limit (2,008 nodes, 80 groups in 8 stages, 1,919 lines) the pure work of a flip (the tile table, the tiling for the new band, the rolled lines) took under 3 ms at every zoom measured, so the cost that remains is React rebuilding the node list and React Flow reconciling it, the same work any change of the graph's data costs. The node data is not rebuilt selectively; that is only worth doing if a measurement of the rendered page shows a stall. No hysteresis is added: a band edge is where a container's children reach a readable size, and a pinch that hovers on it flips between two states that are both correct.
