# Hard delete is a deployment patch, for journeys and for unused routes

Question: deleting several journeys must be one atomic write, and routes nobody uses need a way to go. Where does a hard delete belong, and when is a route unused?

Call: a hard delete is a deployment mutation, `delete_journey` or `delete_route`, each naming the revision it was read at. One deployment patch carries any number of them and commits all or none; anything that moved since, or is already gone, makes the patch stale. This is the one write that reaches across domains. A route is in use while a journey's lineage names any of its versions; the check reads the records the whole patch leaves, so a patch deleting a route and the journeys on it passes in any order, and the commit checks again for journeys started since the read. Deleting a route removes its draft, versions, proposals and events. A deleted id is recorded and never reused. The screens offer deleting a journey only once it is archived, and a route only when nothing uses it.

Alternatives: deletion inside the journey's own domain plus a bulk call (a second write primitive, and exemptions from the archived-write ban and the event log); a route-domain delete (the same cross-domain checks, with its event in a log the deletion removes); deleting a route that old journeys follow (orphans their lineage).

What would change it: deleting a journey through a proposal, or a route with too many users to load for a rejection.
