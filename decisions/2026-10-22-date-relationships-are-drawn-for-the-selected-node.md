# Date relationships are drawn for the selected node only, and the layout does not read them

Question: how does the canvas show the authored date relationships (a due-by, a not-before, a date decision feeding a milestone), and does the layout read them?

Call: the canvas draws them as thin dashed lines for one selected node, both directions, only while the inspector shows that node's Dates section open. The section holds the due date, the linked dates (each row naming the other node and the relationship), the start dates and slack, and the shortfall, so the lines always have their words beside them; the section's fold is the switch, with no setting of its own. A line carries an arrowhead only when the relationship fixes the order, and its sentence on hover or focus. Lines are routed round cards and tiles in bounded time per selection.

The layout does not read dates: it serves the dependencies, and date order has its own view, the Timeline.

Alternatives: feeding the date order to the layout (it chains the cards into one long strip and roughly halves the fit zoom); applying it only while a node is selected (a click would move the picture); drawing every date line at rest, or a canvas toggle (a second vocabulary over the whole canvas, unreadable at scale, and a control apart from the words).

What would change it: a layout method that keeps the fit zoom near today's with most ordering lines forward, or people who want lines with the section folded.
