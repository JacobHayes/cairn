# The zoomed-out graph draws containers as tiles

Question: zoomed out, a complex graph is all small cards and reads as intimidating. How does the canvas show structure there without moving anything?

Call: zoomed far out, below the tile zoom (30%, where a frame's cards are specks), a container is drawn as a tile at its own laid-out box: one sheet with its title, state, progress, due date, exceptional message and at most one fact. It opens in place as a frame at the tile zoom, or earlier once everything directly inside it is a container large enough on screen. One rule for every container, decided by the zoom alone: a pan never changes it and the layout is untouched. Cards outside every container, milestones included, are drawn whole at every zoom. Short of room, a tile's pieces give way in a fixed order, the title last.

- A line into a tile is drawn to the tile; lines between the same two drawn ends merge into one, and its card lists every link between the two containers whatever the zoom, so a shared address means one thing.
- A tile carries the focus, tags and search count of the cards it holds.
- Double-click, double-tap or Enter on a container zooms into it, framed so it is drawn open; folding is the fold control's alone. A collapsed container has nothing to zoom into, so those gestures expand it.
- A flip re-renders once per zoom band crossed, never per frame. No hysteresis: both states either side of a band edge are correct.

Alternatives: one global threshold (a big stage zoomed into stays a blank tile); opening by size alone (unreadable cards return for long flat containers); a compact layout cross-faded at a zoom (positions move); double-click folding (changes depth when the intent is to look closer); tiles from the near zoom down, where a container's head simplifies (they cover the middle zooms, where open frames still read); a tile that lets its cards show through faintly (their words clash with the tile's).

What would change it: flicker when a pinch hovers at a threshold, or a graph large enough that a flip measurably stalls.
