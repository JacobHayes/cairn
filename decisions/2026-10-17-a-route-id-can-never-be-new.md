# A route id can never be `new`

Question (from the unit that gave every page one address): `/routes/new` is the new-route form, and a route id is a user-chosen slug, so a route called `new` would have no page. Who refuses it, and what happens to a stored one?

Call: `RouteId` itself refuses `new` (`IdError::Reserved`), so every path that parses a route id rejects it: the API's paths and bodies, route-file import, save as route, the MCP tools, fixtures and the store's reads. The web form keeps its own early check only so the message appears as the person types; the schema is the rule. `new` is the only top-level word under `/routes/`; every other route address sits below an id (`/routes/<id>/draft`, `/routes/<id>/versions`), and journeys (`j_` prefix) and proposals (`pr_` prefix) cannot collide with `/journeys/new`. The PRD's invariants list says it.

No migration: nothing in the repository, its fixtures or its tests uses the id, no release has shipped, and the one internal deployment holds only the sample route. If a store did hold one, reading it would fail closed on the parse rather than serve a page nobody can reach; renaming it would mean rewriting every journey lineage and version that names it, which is not worth building for data that is not known to exist.

Alternatives: reserve it only in the form (the API and imports stay a back door); move the form to a path no id can take, such as `/routes/+new` (an odd address for the sake of a word nobody needs); migrate stored routes (no known data to move).

What would change it: a deployment found holding a route called `new`, which would need a one-off rename migration; or another top-level word under `/routes/`, which joins the same constant.
