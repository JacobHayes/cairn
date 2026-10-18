# Proof: the views share one look

A journey and a route are drawn the same way on Graph, List and Timeline. The leftovers of
the route-views work, which sat in code both screens share, are fixed once for both. Each
pair is the same page, before and after.

| | Before | After |
|---|---|---|
| Journey graph | ![](before-journey-graph.png) | ![](after-journey-graph.png) |
| Journey list | ![](before-journey-list.png) | ![](after-journey-list.png) |
| Journey timeline | ![](before-journey-timeline.png) | ![](after-journey-timeline.png) |
| Route graph | ![](before-route-graph.png) | ![](after-route-graph.png) |
| Route list | ![](before-route-list.png) | ![](after-route-list.png) |
| Route timeline | ![](before-route-timeline.png) | ![](after-route-timeline.png) |
| Version card | ![](before-route-version-card.png) | ![](after-route-version-card.png) |

The same search on each view, the box alone carrying it:

| | Graph | List | Timeline |
|---|---|---|---|
| Journey | ![](search-journey-graph.png) | ![](search-journey-list.png) | ![](search-journey-timeline.png) |
| Route | ![](search-route-graph.png) | ![](search-route-list.png) | ![](search-route-timeline.png) |

What changed:

- **Controls line up.** What floats on the graph (Depth, Filter, Search, Signal, the zoom
  stack) starts where the header's title and the other views' toolbar start, and ends where
  the header's last button ends.
- **One toolbar.** The search is its box on every view. It no longer also appears as a
  removable chip on List and Timeline alone; the Filter's settings stay chips there.
- **Ruled canvases wear corner ticks** (graph, timeline, phone map), as DESIGN asks.
- **The timeline and the route's list are trees.** Container rows are banded (across the
  timeline's track too) with a 600-weight title, with the Plan list's guide lines, elbows
  and 24px fold buttons (one shared component now), and done work is faded as in the Plan
  list. The route's list is paper rows in a bordered panel, in true tree order, its kind a
  quiet word at the row's end. The guides are `--color-border`, as DESIGN says.
- **Axis text is mono 11px** with tabular figures; a tick name that would cross the end
  line is left out.
- **Inspector cards breathe evenly.** A heading's or paragraph's own margin no longer adds
  to the card's gap; the title sits with its date, and the description, Start a journey and
  the upgrade link are one group.

Known limits:

- A container's heading on a wide graph at fit still shrinks below 13px on screen when its
  head has no room for more; DESIGN allows it, and both screens use the same rule.
- The phone timeline, the phone header and inspector are other work, so no phone shots here.
