# On a phone the journey card is in the inspector, and stays behind the full-screen map

Question (from polishing the inspector column): on a phone the journey card followed the projection in the page and the inspector's tabs came after it, so the tab bar sat a whole card's height below the map and the Assistant opened off screen. Where does the card go, and what happens to it when the map opens full screen?

Call: on a phone the journey card is the inspector's empty state, as at full width: it is in the Inspector tab of the column stacked right under the projection, so the tabs follow the map. When the map opens full screen, an inspector holding only what was there without being asked for (the journey card) stays in the page behind the map instead of becoming a sheet over it; a node, an edge, a selection, the stepper, or another tab in view still opens as the sheet. The page behind keeps its height, so closing the map returns to where it was, and a fit of the preview or the map pads only for a sheet that lies over it (the column stacked under the page covers nothing). DESIGN's Responsive section says so.

Alternatives: keep the card inline after the projection and move only the tab bar (the tabs would then sit above content that is not theirs); show the card in a sheet over the full-screen map (it covers half the map the person just asked for); take the card out while the map is open and restore the scroll on close (the card's summary is read again on mounting, so the page is short for a moment and the restore lands short).

What would change it: the tablet's sheet holding the journey card too, or the full-screen map gaining its own way to the inspector's tabs.
