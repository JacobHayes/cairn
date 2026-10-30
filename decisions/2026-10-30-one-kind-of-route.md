# There is one kind of route, and an insertion leaves a pointer

Question: should routes split into two kinds, one that starts journeys and one that is inserted into graphs?

Call: no. Any route can start a journey or be inserted. Inserting copies the route's nodes in with fresh keys, maps roles and kinds by id, and keeps no insertion record. Each inserted root gets a `from_route` pointer, the route and version it came from, shown like a journey's lineage. The pointer is informational: only a journey's lineage keeps a route from being deleted, so a pointer may name a route that is gone. A route may have several roots; a root title or id can be given only when it has one. A retired route cannot be inserted.

Alternatives: a route kind (a type filter, a kind choice and kind-specific copy on every screen, for no capability); insertion records beside the pointer (a second source of truth for where a node came from, and a reason deleting a route must scan every graph).

What would change it: moving an inserted piece onto a newer route version, which would need the pointer on every node of the piece.
