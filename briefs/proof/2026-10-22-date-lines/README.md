# Date lines: a node's date relationships, drawn when it is selected

Select a node and its date relationships appear beside what it waits on: the due-by and not-before rules it has or that read it, its stage's close and opening, and a date decision that sets a milestone. They are dashed ink lines, never faded, with an open arrowhead at the later end only where the order is fixed. They have no text; hover gives the sentence. The inspector's Linked dates section (with its count) lists the same relationships, each with the other node as a link.

![Before: the launch journey at rest on main](before-rest.png)

Before: at rest on main. Nothing shows a node's date relationships.

![After: the same journey at rest](rest.png)

After, at rest: unchanged. Date lines are never drawn without a selection. The launch journey is now laid out in date order, so Launch is last.

![Documentation selected, hovering its line](docs-selected.png)

Documentation is due 7 days before Launch: one line, arrowhead at Launch, the sentence on hover.

![Launch selected with Linked dates open](launch-selected.png)

Launch has five relationships; the far ends stay in focus while the trace fades the rest, and the inspector lists them.

![Beta starts selected](beta-start-selected.png)

Beta starts: a not-before after the code freeze (arrowhead) and a due-after tied to Beta ends (no arrowhead, since it fixes no order), each at a port of its own.

![At phone width](phone.png)

At phone width the same line is drawn over the canvas.

Video of the main flow: [date-lines.webm](date-lines.webm) selects Documentation, hovers its line, selects Launch, opens Linked dates and follows a link.

## Ports, routes and the date order

Lines meeting at one side of a card take evenly spaced places down it (Launch above has four arrowheads, not a pile) and run through the gutters between cards and stage frames. On the fixtures with every card selected in turn (30 lines):

| | Before | Now |
|---|---|---|
| Lines crossing a card | 5 | 0 |
| Lines crossing a stage frame holding neither end | 6 | 0 |
| Ordering lines drawn right to left (launch) | 8 of 9 | 0 |

The layout now reads a bounded date order (one binding source per rule, each stage close and opening, each fed date). At the node limit it is 143 KB in a 5.6 MB level, 2.6% more; the decision record has the numbers.

## Known limits

- A date line to a node with no card at the current depth is not drawn; the inspector still lists it.
- The date order makes the launch fixture about three times as wide (it reads in time order), so its fit view is small; vendor evaluation was already as wide.
- A canvas too large for the routing grid draws a plain elbow that may cross cards.
