# Date relationships are drawn for the selected node only, and the layout does not read them

Question: the canvas draws what waits on what, but not the authored date relationships (a due-by, a not-before, a stage close, a stage opening that does not gate, a date decision feeding a milestone). How are they shown, and does the layout read them?

Call: the level projects them (relevance-pruned, rolled up to stand-ins, each with a direction by meaning and a sentence) and carries them only for the card of the level query's selected node. The canvas draws them for one selected node only, both directions, as thin dashed ink lines that are never faded, with an open arrowhead at the later end only when the relationship fixes the order. A line has no text; hovering gives the sentence. A line is not clickable into a line card; the inspector's linked-dates fold is where it leads. There is no show or hide setting. The old "dates only" kind of waits-on line is removed: a stage opening that does not gate is a date relationship, not an edge.

The layout does not read dates. The graph's layout serves the dependencies and the overview; date order has its own view (the Timeline), and a date line that runs right to left reads by its arrowhead and its sentence.

Three ways of giving the layout the date order were built and measured on the fixtures. (a) Feeding ELK every ordering pair needed 252,097 pairs at the date limits (a 45 MB level against 5.3 MB of edges, and the browser's model build ran out of its 4 GB heap). A bounded form (each rule's binding source, each stage close and opening, each fed date, filtered against the dependencies) cost 143 KB in a 5.57 MB level at the limits and laid out in under 100 ms, so cost was never the problem. The shape was: it chains the cards into one long strip. (b) Not feeding the dates is the shipped choice. (c) The best middle option found: only the orders into milestones, laid out with ELK's MULTI_EDGE wrapping.

Fit zoom of the whole graph at rest, in a 1280 x 800 window (the launch journey, Depth: Actions), and ordering date lines drawn right to left, from the layout alone:

| Fixture | (a) bounded feed | (b) no feed | (c) milestones and wrapping |
|---|---|---|---|
| launch, window fit | 0.16 | 0.45 | 0.35 |
| launch, 988 x 588 canvas | 0.26, 0 of 9 backwards | 0.70, 8 of 9 | 0.53, 2 of 9 |
| vendor evaluation | 0.27, 0 of 2 | 0.30, 2 of 2 | 0.32, 0 of 2 |
| stress journey | 0.29, 0 of 1 | 0.41, 1 of 1 | 0.20, 0 of 1 (tall) |

Wrapping the full feed (MULTI_EDGE 0.44, SINGLE_EDGE 0.35 on launch, both with 2 of 9 backwards) and ELK's aspect ratio (no effect) did not bring the fit near (b). Applying the date order only while a node is selected was rejected: the layout would move on a click, which breaks the rule that a small change moves few nodes and makes selecting a node rearrange the picture the viewer is reading.

Routes. A date line leaves and enters a card by ports of its own: the lines meeting at one side of a card take evenly spaced places down it, ordered by the height of their other ends, so arrowheads never stack. Each line is routed by a cheapest-path search on a grid (bends, a stretch another line already uses, and the inside of a stage frame that holds neither end all cost; cards never can be crossed), which finds the gutters between cards and stage frames by itself. A canvas too large for the grid (more than 40,000 points at the coarsest step) is routed with a plain elbow, and so is a line once the selection's searches have settled 9,000 grid points between them or after the first 60 lines, so a selection costs the main thread a bounded few milliseconds (the fixtures' worst selection, 3.5 ms warm; a synthetic worst case at the budget, 11 to 17 ms warm; the routes are computed when the selection or the layout changes, not on every render). On the fixtures with every card selected in turn (30 lines), lines crossing a card went from 5 to 0 and lines crossing a stage frame that holds neither end from 6 to 0, and no two lines of one selection share a stretch. Measured on the fixtures with every card selected in turn (30 lines): lines crossing a card, 5 before and 0 now; crossing a stage frame that holds neither end, 6 before and 0 now; stacked arrowheads 0.

Alternatives: the layout feed above in its three forms; a toggle for drawing all date lines (a second vocabulary across the whole canvas at rest, and a setting); drawing every line at rest (same, and unreadable at the limits); feeding ELK edge priorities without filtering (priorities are only preferences, and one dependency against twelve date paths reversed the dependency); a new limit capping the pairs a level carries (more machinery for a layout nicety, and it would change layouts with graph size).

What would change it: a wrapping or layering method that keeps the fit zoom within about 10% of no feed, with most ordering lines forward; or a measured complaint that date lines running backwards are not read by their arrowheads and sentences.
