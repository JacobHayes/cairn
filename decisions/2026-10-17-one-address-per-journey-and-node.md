# One address per journey and node, a real Home, and no redirects

Question (from the unit that gave every journey and node one address): what do the app's addresses name, which page is the root, and what happens to the paths that came before?

Call: the path names the thing and the query says how to look at it, and the path mirrors the API. Home is `/` (what is waiting on the viewer in each active journey, then their active journeys, rendered in the first paint); the journey index is `/journeys` with its filters in the query; the new-journey form is `/journeys/new`; a journey is `/journeys/<id>`, a node selected in it `/journeys/<id>/nodes/<key>`, an edge's card `/journeys/<id>/edges/<from>~<to>`. The view is `?view=graph|list|timeline|summary|walk`, written only off its default. Routes replaces the Library: its index is `/routes` (one table of routes and segments with the type filter, as before), the new-route form is `/routes/new`, and the rail reads Home, Journeys, Routes, Entities, each lit only on its own pages. `/proposals/...` is unchanged (superseded by `2026-10-17-a-proposal-lives-under-its-destination.md`: a proposal now lives under its destination). The PRD's wording that named "the Library" (C19) and the cross-journey "mine" list (C16) changes with it.

Nothing is aliased or redirected: `/mine`, `/new`, `/library`, the per-view paths under a journey (`/journeys/<id>/graph` and the rest) and every `<Navigate>` used as a redirect are gone, and an address that names nothing shows "no such page" at the address it was given. A link to a node is the node's own address and needs no extra parameters; the view draws the node (see `2026-10-17-a-selected-node-is-always-drawn.md`). Home's rows open that address, and a row's actions still act in place.

Because `/routes/new` is the form, a route cannot be called `new` (see `2026-10-17-a-route-id-can-never-be-new.md`).

Alternatives: keep redirects from the old addresses (a second address for each page, none of them removable once links exist, and the redirects were what made the page jump); keep "Library" as the name (a segment is a route of kind segment, so Routes names both).

What would change it: proposals moving under their destination, which would give them addresses of their own.
