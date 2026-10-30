# Direct MCP writes are patches, through two tools

Question: single-op wrapper tools give agents two ways to make each state change and fail on combined moves, such as a note or ticks that must ride with a completion. How are direct writes offered?

Call: every direct MCP write is a patch, through one of two tools, each listing every op it takes. `update_journey_state` records what happened in one journey: answers, transitions, notes, ticks, who does a node, pins and recorded dates, snoozes, overrides, the journey's own fields, and a new identity an answer names; any other op is refused, pointing at `apply_patch`. `apply_patch` takes any patch on any target, state and structure alike. Both can land the patch in a proposal instead. Identity writes and route import keep their own tools.

Alternatives: `apply_patch` alone (more refusals and fewer first writes accepted, with more schema loaded per task); tools split by purpose (atomicity breaks at the seams: a removal needs the unsnooze naming the node); wrappers for frequent actions (two ways to do each, and combined moves still fail).

What would change it: clients that approve by argument instead of by tool name, agents misusing the restricted tool, or a direct write the patch model cannot express.
