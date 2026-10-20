# The inspector on a phone is a bottom sheet

On a phone (and a tablet) the inspector is now a sheet over the page, with a handle that really drags, a real close, and no bar or blank strip left behind when it is closed. It is the same sheet for a journey's graph, list and timeline, a route's page, and the Proposals tab.

![The sheet at its peek: the tabs and the top of the pane, under a graph that ends above it](1-peek.png)
The peek: a grip, the tabs, and the top of the pane. The graph ends above it, so the whole graph still pans.

![A node picked on the graph opens the sheet at its peek](2-node-at-peek.png)
Picking a node opens it at the peek, showing the node's kind, status and title.

![Dragged up to half](3-half.png)
Dragged up (finger or pointer) it settles on half; the node's form is in reach and the graph is still above.

![Full height](4-full.png)
Full height, where the sheet's one scroller reads the node; a tap on the grip or the arrow keys step it back.

![Closed: a pill and nothing else](5-closed.png)
Dragged down below the peek (or the hide button, or Esc) it closes: one labelled pill, the graph fills the window to its foot, and its bottom controls ride above the pill.

| Sibling views at the peek | |
| --- | --- |
| ![List](6-list.png) | ![Timeline](7-timeline.png) |
| ![Proposals tab](8-proposals-tab.png) | ![A route](9-route.png) |

`sheet.webm` is the main flow: pick a node, drag the sheet to half and full, close it, open it from the pill, pan the graph.

Known limits:
- A node opened by address (a link) or a tap lands at the peek, so its form is one drag or tab tap away; that is the design's choice for keeping the graph usable.
- A landscape phone has little room above a 160px peek; the sheet still works but the graph gets short.
