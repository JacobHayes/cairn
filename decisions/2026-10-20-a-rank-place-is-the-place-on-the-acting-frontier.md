# A rank place is the place on the acting frontier

**Question.** Every screen that shows "#N" for a node needs one order to number it by. Which order is it?

**Call.** "#N" is the node's one-based place on the acting frontier, in the engine's order, read through one function in the app. A node that ranks but is off the acting frontier (blocked, snoozed, scheduled) has no place, so its rank cell is empty. A List sorted by rank can therefore show a blocked node above "#1" with an empty cell: it ranks higher but cannot be acted on, and the empty cell says so.

**Alternatives.**
- A second order over every ranked node, for the List only: two meanings of "#N" on one screen.
- The rank score itself: not an order a person reads at a glance.

**What would change it.** A need to rank blocked work for planning, which would be a new column with its own name, not a second "#N".
