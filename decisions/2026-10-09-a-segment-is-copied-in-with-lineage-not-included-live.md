# A segment is copied in with lineage, not included live

Question (from the segments work, PRD A21 and B13 to B15): when a segment is put into a route or journey, does the graph hold a live reference to the segment, so its nodes always reflect the segment's latest version, or a copy that remembers where it came from?

Call: a copy with lineage. Inserting a segment copies its nodes, edges, conditions, rules, and resources into the graph with fresh keys and records an insertion naming the segment version. The copied nodes are ordinary nodes from then on: a journey can edit, break down, skip, or remove them like any other. A newer segment version reaches an insertion only through an insertion upgrade, which merges like a route upgrade into a journey and is applied on confirmation. A segment is a route of kind segment (see `2026-10-09-a-segment-is-a-route-of-kind-segment.md`). Keys are minted from the insertion's key and each segment key, so one segment can be inserted more than once into the same graph; each member remembers its key in the segment, and upgrades match on that key within the insertion. A segment refers only to its own nodes, roles, and kinds and declares no `final` milestone or `default_owner`, so it can be inserted anywhere without carrying assumptions about the graph it joins; it is connected to that graph by edges and by role and kind mapping when it is inserted.

Alternatives:

- Live inclusion by reference. Every journey would change under its users whenever a segment is published, which breaks journey durability (route edits never touch in-flight journeys until an upgrade is confirmed), and a journey could not edit an included node without forking it anyway.
- A segment as its own patch domain beside routes. Weighed and rejected in `2026-10-09-a-segment-is-a-route-of-kind-segment.md`.
- Keeping the segment's keys on insertion. Simpler matching, but one segment could be inserted only once per graph, and a key retired from a graph could not come back through a later insertion.
- Letting a segment reference nodes outside itself (open ports). That gives more wiring power, but an unresolved reference would have to be checked against every graph it might be inserted into. Edges added at insertion cover the same need inside the patch, where they are validated.

What would change it: a need to push a segment change into many in-flight journeys at once would call for bulk insertion upgrades across journeys, not live inclusion. If role mapping and omission prove too little, the next step is binding a segment decision or milestone to a host node (PRD Later), not parameters.
