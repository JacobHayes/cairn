# A route has a journey's views and view state

Question: the route-views unit (a route page with the journey's Graph, List and Timeline) had to
settle how a route draws its graph, since a route had its own canvas with its own ladder, a
drill-in, and no search or filter, so its views behaved unlike a journey's.

Call:
- A route page uses the journey's view state: Depth, Fade, the Filter and the search, shared by
  every view and carried by every switch, with the address holding exactly what someone chose.
  A route offers a subset (`ROUTE_OFFERS`): no Frontier and no Only mine, since a route has no
  state or owners. A parameter a route does not offer is dropped from the address on arrival.
- The route's level is the engine's level projection over the shown graph (wasm `routeLevel`),
  as a journey's is, so a collapse, a hidden node and a selected node behave the same. The
  drill-in, the route's own ladder helpers and the route-only canvas settings are removed;
  expanding and collapsing happen in place.
- A route opens at the stock view state (Actions depth). A selected node is drawn whatever the
  depth or search would hide, as on a journey.

Alternatives: keeping the route canvas and adding a search to it (two canvases to keep in step,
the drill-in left as a second way to look at depth); a fixed route-only ladder (the case this
replaces).

What would change it: a route concept with no journey equivalent (pinned positions per route,
which the PRD defers) would need a view setting of its own.
