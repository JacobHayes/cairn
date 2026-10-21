# The Filter counts what a class hides beside its box, not on the toolbar

Question (from the filter-classes unit): decision `2026-10-09-not-relevant-nodes-are-hidden-by-default.md` said the count of hidden not-relevant nodes is always shown, which the toolbar's Filter badge did for both classes together. Where should the count live, and which views offer which classes?

Call: the count moves into the open Filter, in neutral (muted, small) type beside the box of the class it counts: "Show not relevant 4", "Show conditional 2". The toolbar's Filter button wears no badge any more (Depth keeps its badge); on a phone the Controls menu's badge is Depth's alone, and its Filter section shows the same counts. A count is shown only for a class the view can draw and that hides something; conditional follows the same rule as not relevant, so the two read alike. This supersedes the "always shown" count in the earlier decision: the count is shown wherever the Filter is open, not on the closed toolbar. Which classes each view can draw is one table (`CLASS_REASONS` in `journeys/view.ts`): the graph and list draw both, the timeline draws conditional only, the walk draws neither; a class a view cannot draw is shown disabled with its reason and no count. The boxes, their reasons and the counts all read that table.

Alternatives: keeping the combined toolbar badge (it mixed two classes under one number and told nothing about which box would reveal them); counting only not relevant (the two boxes would read differently for no reason); a separate hidden-count chip on the canvas.

What would change it: finding that people miss hidden nodes without a count on the closed toolbar, which would argue for a quiet marker on the Filter button.
