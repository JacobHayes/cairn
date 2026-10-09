# A segment is a route of kind segment

Question (from the segments work, PRD A21): is a segment its own patch domain, with its own versions, drafts, tables, events, endpoints, and tools, or a route with a kind?

Call: a route with a kind. `create_route` takes `kind: process | segment`, defaulting to `process`, and the kind never changes. A segment shares the route's id space, header, revision, versions, draft, publish, retire, import and export, events, notifier, authoring canvas, and proposals. What differs is four engine checks: a segment graph has no `final` milestone, no `default_owner`, and at most one root (exactly one to publish); no journey is created from a segment; no lineage, re-link, or save as route names one; an imported file's kind matches its route. The store gains one `kind` column, the route index a `kind` filter, and the Library shows both kinds in one table. Files carry `kind` and no insertions, since lineage is local to a deployment.

Alternatives:

- A separate domain (the earlier plan). It duplicates the whole lifecycle across schema, engine, both stores, service, API, MCP, and the screens, with a second copy of every lifecycle test, to buy a separation the four checks already give.
- A kind that can change. Journeys and lineage could then hang off a segment, and insertions off a process route.

What would change it: segments needing lifecycle behaviour that routes must not have.
