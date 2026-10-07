# Completion is suggested from the status summary's remaining count or the final milestone reached

- Question: B11 suggests completion "when every relevant or undecided node satisfies dependencies or the graph's `final` milestone is reached"; the derived journey has no field saying either.
- Call: the overview asks the derive worker for the status summary (C18) and suggests completion when its `remaining` (in-scope nodes not closed: the engine's own count of nodes that do not yet satisfy dependencies) is zero, or when the `final` milestone is stored `reached` or reads as reached (F1's auto-reach). The suggestion highlights "Complete"; it never completes anything.
- Alternatives: a `completion_suggested` field on the derived journey (an engine change for one screen's hint); counting terminal states in the tab (it would miss skipped containers waiting on kept work, D1a, which `remaining` already accounts for).
- What would change it: an agent or another client wanting the same hint, which would move it into the engine's derive beside `stalled`.
