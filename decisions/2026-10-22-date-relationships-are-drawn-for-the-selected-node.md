# Date relationships are drawn for the selected node only, and the layout reads a bounded date order

Question: the canvas draws what waits on what, but not the authored date relationships (a due-by, a not-before, a stage close, a stage opening that does not gate, a date decision feeding a milestone). How are they shown, and does the layout read them?

Call: the level projects them (relevance-pruned, rolled up to stand-ins, each with a direction by meaning and a sentence) and carries them only for the card of the level query's selected node. The canvas draws them for one selected node only, both directions, as thin dashed ink lines that are never faded, with an open arrowhead at the later end only when the relationship fixes the order. A line has no text; hovering gives the sentence. A line is not clickable into a line card; the inspector's linked-dates fold is where it leads. There is no show or hide setting. The old "dates only" kind of waits-on line is removed: a stage opening that does not gate is a date relationship, not an edge.

The layout reads a bounded date order, not the relationships. Feeding ELK every ordering pair needed 252,097 pairs at the date limits (a 45 MB level against 5.3 MB of edges; the browser's model build ran out of its 4 GB heap), so that form was dropped. The bounded form is what ships: the level carries, for every card, `date_order`: for each rule on a node, the one source that binds (the latest for a not-before, the earliest for a due-by, by the effective date, then the due date, then the earliest start; an undated source binds after a dated one), plus each stage close and opening and each date a decision feeds. That is at most one pair per rule, stage bound and fed date, the order of the edges. The canvas lifts the pairs to sibling cards and gives ELK only those that agree with a ranking of the dependencies (ELK's edge priorities are only preferences, so one dependency against twelve date paths reversed the dependency until the filter was added).

Measured on the fixtures, ordering date lines drawn right to left:

| Fixture | Ordering lines | Without the order | With the bounded order |
|---|---|---|---|
| launch | 9 | 8 | 0 |
| vendor evaluation | 2 | 2 | 0 |
| hiring loop, bake-off | 0 | 0 | 0 |
| stress journey | 1 | 1 | 0 |

At the date limits (2,000 nodes, two rules of 64 sources each), the order is 4,332 pairs, 143 KB in a 5.57 MB level (2.6% of it), of which 59 survive the dependency filter; the engine builds the level in 265 ms and the web model and layout request in 88 ms and 81 ms. The plan's condition (cost at the limits grows by at most a quarter) holds. The layout of the limit graph itself could not be timed in-thread on either side: ELK runs out of a 4 GB heap on its 46,000 edges with or without the order.

The cost is width: the launch fixture is 3,732 px wide with the order against 1,300 without, since the dates now chain Build, Code freeze, Beta, Materials, Launch and Retrospective into columns. Vendor evaluation was already 3,235 and is 3,571. A graph that follows its dates reads left to right in time, and a wide one is fitted smaller; that is the trade.

Routes. A date line leaves and enters a card by ports of its own: the lines meeting at one side of a card take evenly spaced places down it, ordered by the height of their other ends, so arrowheads never stack. Each line is routed by a cheapest-path search on a grid (bends, a stretch another line already uses, and the inside of a stage frame that holds neither end all cost; cards never can be crossed), which finds the gutters between cards and stage frames by itself. A canvas too large for the grid (more than 160,000 points at the coarsest step) is routed with a plain elbow. Measured on the fixtures with every card selected in turn (30 lines): lines crossing a card, 5 before and 0 now; crossing a stage frame that holds neither end, 6 before and 0 now; stacked arrowheads 0.

Alternatives: a toggle for drawing all date lines (a second vocabulary across the whole canvas at rest, and a setting); drawing every line at rest (same, and unreadable at the limits); feeding ELK edge priorities without filtering (priorities are only preferences, and one dependency against twelve date paths reversed the dependency); a new limit capping the pairs a level carries (more machinery for a layout nicety, and it would change layouts with graph size).

What would change it: a measured complaint that the date order makes a journey too wide to read at its fit (the order could then be fed only for the selected node's lines, at the cost of the layout moving with the selection), or a graph so large that the routing grid's limit is hit often (a limit to raise in the router with the evidence).
