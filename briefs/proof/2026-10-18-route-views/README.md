# Proof: a route has a journey's views

A route is now read the way a journey is: one page, the same View picker (Graph, List,
Timeline), the same Depth, Filter and search, and a version selector in its header. The
Timeline is new for routes: the earliest schedule of the route, in days from its start.

![The route's graph, with its version selector and the version's card](1-route-graph.png)

The route's page at its bare address: Version 2 is the latest, so it is shown read-only,
with its card (the journeys that can upgrade to it, Start a journey) in the inspector.

![The version selector open](2-version-selector.png)

The selector names the graph shown. Each item is a link: a version says when it was
published and how many journeys are on it; the draft, when one is open, is the first item.

![The List at Stages depth](3-route-list-stages.png)
![The List with a search](4-route-list-search.png)

Depth and the search are the journey's. They ride in the address (`depth=`, `q=`) and
every switch between views keeps them.

![The Timeline of the same search](5-route-timeline.png)

The Timeline on days from the start: a Start line at day 0 and the final milestone as the
end anchor, which says when it is only reachable through an undecided branch.

![Version 1, picked in the selector](6-version-one.png)

Picking Version 1 keeps the view and the view state, drops Publish for "Open a draft to
edit", and shows the journeys on that version with their upgrade state.

![The product launch's schedule](7-launch-timeline.png)

A draft of the product launch: the feature work starts at day 0, the beta follows the
code freeze, and Launch ends the line on day 40. A row's title opens that node's form.

![The same on a phone](8-phone-timeline.png)
![The draft's graph on a phone: the header is three short lines](9-phone-graph.png)

Video of the main flow: [main-flow.webm](main-flow.webm).

## What changed for someone using it

| Before | After |
|---|---|
| A route's versions, its graph and its draft were three screens | One page, `/routes/<id>`: the draft if one is open, else the latest version; `?version=<n>` names one |
| The route's graph had its own ladder and a drill-in, and no search | The journey's Depth, Filter and search, shared by every view |
| No timeline for a route | Timeline from day 0 (Day 0, Day 5, ...), no calendar dates |

Known limits:

- Two versions cannot be compared side by side; the selector only switches between them.
- A route's timeline is the earliest case: estimates are not ranges.
