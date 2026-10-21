# A route's canvas selects and traces as a journey's does

Question (from the route-trace work): a route draft, a published version and a segment drew
their cards through a path of their own that could not trace a selected node. How should a
click on a route's node behave, and how much of the journey's path should it share?

Call:

- A route's shown graph is derived once in the worker, as a journey that starts on day 0 and
  in which nothing has happened, and held under a fresh slot for each load. Level, trace and
  schedule are the journey's projections read from that derivation; the per-call route level
  and schedule operations are deleted. The page pairs the graph with its derivation in one
  snapshot, so an answer is shown only for the graph it was read from, and a route's reads keep
  their last answer on screen across loads while an edit is re-derived.
- One canvas hook (`canvas/graph.ts`) serves both canvases; `JourneyCanvas` and `RouteCanvas`
  are thin shells that supply their cards and actions. Selecting a node on a route lights the
  lines it needs and unblocks, fades the other cards, tags NEEDS and UNBLOCKS, and offers
  Reveal and Hide for the cards that stand for hidden nodes, as on a journey.
- A node address names a node of the graph shown, a version's too (`?version=<n>` is kept). A
  version's node opens in a read-only panel in the inspector. The old fail-closed address with
  a notice is gone.
- The nodes that add to the selected node's gravity carry a filled dot on the UNBLOCKS tag, on
  routes and journeys alike.
- The trace for cards standing for hidden nodes is one multi-source walk in the engine, not a
  join of per-node traces in the page. Measured on a journey at the node limit (2000 nodes), the
  join took 5.4 s in a debug native build, with 3.9 million keys and 37 MB of JSON; the walk
  answers in one projection.
- What does not come to routes, and why: the Signals lens (state, rank and slack signals do
  not exist on a route, where nothing has happened; a Key button holds the place); multi-select
  (the route's inspector is a form or a read-only panel for one node, and there is no action on
  a set to offer); the node context menu (its entries are journey actions). Edge selection and
  the edge card are a follow-up that rebuilds the journey's edge card from the engine's edges
  and gives routes the same card.

Alternatives: a one-shot route trace operation beside the level one (a second path to keep in
step, and it re-derives per click); a synthesized journey document for the route (invents
records that do not exist); selection kept in component state (breaks links and Back).

What would change it: routes gaining state of their own (a rehearsal), which would make the
Signals lens meaningful there; or a set action on a route's nodes, which would make
multi-select worth having.

Superseded in part: edge selection and the edge card are done (`2026-10-21-a-drawn-line-opens-the-links-it-stands-for.md`). A route's lines are selectable and open the one card a journey's lines open.
