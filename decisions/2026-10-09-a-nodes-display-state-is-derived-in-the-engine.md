# A node's display state is derived in the engine

Question (from the web redesign, PRD D8, C1, C8): stored state is a per-kind machine, relevance is derived, and B2 deliberately keeps `open` or `done` on a node that stops applying. Only groups had a derived display state, so every other surface printed stored state and the browser patched it in four places; a decision ruled out by another answer still read "open". Where should the state people see be composed?

Call: once, in the engine's derive, as `DisplayState` (D8): not relevant, skipped, done, snoozed, active, conditional, blocked, scheduled, ready, first match wins. The group rule moves from the level projection into the derive and now covers every node; a container waiting only on its own children is never blocked for display. Every surface, the API, the snapshot, and MCP read this value beside stored state, which stays the subject of transitions and the secondary detail ("recorded Done"). The display words ("to decide" for a ready decision, "not started" for a ready container) live in one web module mirrored in the MCP instructions. This replaces the group vocabulary in `2026-10-06-what-a-canvas-level-draws-marks-and-rolls-up.md` (waiting, not started) and the status summary's counts by stored state in `2026-10-06-what-the-decision-view-timeline-status-summary-list-and-mine.md`, which now count by display state beside them.

Alternatives:

- A browser-side mapping from stored state and flags. That is today's bug multiplied: four surfaces, and agents seeing something different from people.
- Changing stored state when relevance changes (writing `skipped` or a new `not_relevant` state). It would break B2's promise that recorded state survives, and make a derived fact into an event.
- Replacing the D3 flags with the composite. The flags are independent of each other on purpose; the composite is derived from them.

What would change it: a surface that needs a state the precedence hides (an active node's blocker, say) as its primary signal rather than as secondary detail.
