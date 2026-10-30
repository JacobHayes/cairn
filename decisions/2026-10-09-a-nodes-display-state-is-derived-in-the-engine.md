# A node's display state is derived in the engine, beside stored state

Question: stored state is a per-kind machine, relevance is derived, and a node that stops applying keeps its recorded state, so stored state alone misleads (a decision ruled out by another answer still reads "open"). Where is the state people see composed, and how does it reach clients that read stored state?

Call: once, in the engine's derive, by a fixed precedence. A container waiting only on its own children is never blocked for display. Every surface, the API, the snapshot, and MCP read it beside stored state, which stays the subject of transitions and the secondary detail. Display-state fields are always added beside stored-state fields, never in their place: `state` keeps meaning stored state, and counts by display state sit beside counts by stored state.

Alternatives: a mapping in the browser (each surface would compose it, and agents would see something different from people); writing a new stored state when relevance changes (recorded state would no longer survive, and a derived fact would become an event); repurposing `state` as the display state (every existing reader would silently change meaning).

What would change it: a surface that needs a state the precedence hides as its primary signal.
