# The engine plans a removal and reads draft placeholders

Question (from the work that made the editors ask the engine): the
web app kept its own walker over every place a node, role or kind can
be named, to work out what a removal leaves dangling and what to
rewrite, and its own grammar for the placeholders in a message draft.
Both are rules the engine owns. Where should they live?

Call: the engine. `plan_removal` walks the graph once through the
node reference walker, with a translator that answers "gone" for what
is being removed, and returns the removal, the nodes it takes and
every rewrite to send first (conditions, dates, milestone and role
slots, participations, snoozes, default owner, message drafts). The
removal read that tells a removal's outside references uses the same
translator, so its own hand-written walker is deleted. The placeholder
grammar (roles by id or key, decisions by path or key, journey
fields, a draft stored with keys) is one module in the engine's
format code. The browser asks both through the derive worker, like the
condition check, and shows "Working out what else changes" while the
plan loads.

A message draft that loses a placeholder keeps its other text; a
draft left with nothing else is removed.

Alternatives: keep the TypeScript copies and test them against the
engine (two owners, drift between them); add server endpoints (the
browser host has no server, and the plan is only ever asked of a
graph the browser already holds).

Would change this: a host that needs the plan without the worker, in
which case the same functions are exposed there.

The restore-from-tombstone flow still rewrites a restored node's
references in TypeScript; it translates the other way (back into a
graph) and is left for its own change.
