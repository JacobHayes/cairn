# There is one kind of route, and an insertion leaves a pointer

Question (from removing the segment kind): a route was either a process (started into journeys) or a segment (inserted into graphs), with insertion records, a one-root rule, and segment-only screens. Is the split worth its code?

Call: no. Any route can start a journey or be inserted. Inserting copies the route's nodes in with fresh keys, maps roles and kinds by id as before, and keeps no insertion record. Each inserted root gets a `from_route` pointer, the route and version it came from, shown the way a journey's lineage is and available for a future upgrade to diff against. The insertion key stays only as the seed every new key derives from; it is not stored.

- The pointer is informational. Only a journey's lineage keeps a route from being hard-deleted; an inserted root's pointer may name a route that is gone.
- A route may have several roots. Every inserted root gets the pointer and its own free id among its new siblings; a root title or id can be given only when the route has one root.
- The pointer is an ordinary field of the node, so a route file writes and reads it with the node.
- A retired route cannot be inserted, as it cannot start a journey.
- Stored segment routes become plain routes, and insertion records become the pointer on the inserted root (the member copied from the version's top node, else the first remaining member).

Alternatives:

- Keep the kind and drop only the one-root rule: leaves the type filter, the kind choice, the kind-specific copy on every screen and the insertion records for no capability the single kind lacks.
- Keep insertion records beside the pointer: a second source of truth for where a node came from, and the reason deleting a route had to scan every graph.

What would change it: upgrading an inserted piece to a newer route version would need to know every node of the piece, not only its root; the pointer would then move onto each node, or a record would return.
