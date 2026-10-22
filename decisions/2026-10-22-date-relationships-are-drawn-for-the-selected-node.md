# Date relationships are drawn for the selected node only, and do not feed the layout

Question: the canvas draws what waits on what, but not the authored date relationships (a due-by, a not-before, a stage close, a stage opening that does not gate, a date decision feeding a milestone). How are they shown, and does the layout read them?

Call: the level projects them (relevance-pruned, rolled up to stand-ins, each with a direction by meaning and a sentence) and carries them only for the card of the level query's selected node. The canvas draws them for one selected node only, both directions, as thin dashed ink lines that are never faded, with an open arrowhead at the later end only when the relationship fixes the order. A line has no text; hovering gives the sentence. A line is not clickable into a line card; the inspector's linked-dates fold is where it leads. There is no show or hide setting. The old "dates only" kind of waits-on line is removed: a stage opening that does not gate is a date relationship, not an edge.

The layout does not read date relationships. The layout feed was prototyped (ordering relationships as ELK edges, filtered against the dependencies so ELK never has to reverse one) and measured:

| Fixture | Ordering date lines | Drawn right to left without the feed | With the feed |
|---|---|---|---|
| launch | 9 | 8 | 0 |
| vendor evaluation | 2 | 2 | 0 |
| hiring loop, bake-off | 0 | 0 | 0 |
| stress journey | 1 | 1 | 0 |

It helps where there are date lines, but it needs every ordering relationship in the level whatever is selected. At the node limit with every node carrying two rules of 64 sources, that is 252,097 pairs: a level of 45 MB against 5.3 MB for its edges, built in 350 ms, and the model build in the browser ran out of its 4 GB heap before layout could be timed. The plan's condition for keeping the feed was that the cost at the limits grows by at most a quarter; it grows eight-fold. The feed is dropped and the numbers are kept here.

Alternatives: a toggle for drawing all date lines (a second vocabulary across the whole canvas at rest, and a setting); drawing every line at rest (same, and unreadable at the limits); feeding ELK edge priorities without filtering (priorities are only preferences, and one dependency against twelve date paths reversed the dependency); a new limit capping the pairs a level carries (more machinery for a layout nicety, and it would change layouts with graph size).

What would change it: a bounded way to state the order the dates imply that costs the level a quarter or less at the limits (for example the engine reducing the pairs against the dependencies before they leave it), or a measured complaint that the selected node's lines run backwards often enough to matter.
