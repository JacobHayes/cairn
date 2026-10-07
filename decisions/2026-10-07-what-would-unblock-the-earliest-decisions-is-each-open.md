# "what would unblock the earliest decisions" is each open decision's immediate dependencies, earliest start first

- Question: C11 has the walkthrough, when no decision is actionable, show "the milestones and dependencies that would unblock the earliest ones" without saying what earliest orders by or how far upstream to look.
- Call: the open, in-scope decisions off the acting frontier, by derived earliest start (F3), none last, then key; the first five, each with its immediate unsatisfied dependencies: its own (explicit, condition gate, stage opening) and those an ancestor holds for it (named as that ancestor's), never an ancestor's own children, plus a snooze's target or date when one holds; milestones first.
- Alternatives: rank order (rank orders the frontier, and these are off it); the whole upstream chain (the canvas's trace already draws it, and one step is what to finish next).
- What would change it: people asking why a decision waits when its immediate blocker is itself blocked, which would add the chain to the nearest actionable node.
