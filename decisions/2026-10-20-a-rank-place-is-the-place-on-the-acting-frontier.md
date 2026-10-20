# A rank place is the place on the acting frontier

**Question.** The List, the inspector, the canvas tags and the Signals lens each said "#N" for a node, and the List computed its numbers from a ranking of its own (every ranked node, blocked ones included) while the others used the acting frontier. The same node could show two different numbers. Which order does "#N" mean? (From moving derived values out of the web app and into the engine.)

**Call.** "#N" everywhere is the node's place on the acting frontier, one-based, in the engine's order (rank, slack, gravity, key): `Derived.acting_frontier`, read through one function in the app. A node ranked but off the frontier (blocked, snoozed, waiting) has no place, so its rank cell is empty. Sorted by rank, the List can therefore show a blocked node that outranks "#1" with an empty rank cell above it: it ranks higher but cannot be acted on, and the cell says so by saying nothing.

**Alternatives.**
- A second derived order over every ranked node for the List only. Rejected: two meanings of "#N" on one screen pair, and a second order for the engine to keep in step.
- The rank score (a 0 to 1 number) in the cell. Rejected: it is not an order a person can read at a glance, against the Approachable by default principle.

**What would change it.** People asking why the top row of a rank-sorted List has no number, or a need to rank blocked work for planning, which would be a new column with its own name, not a second "#N".
