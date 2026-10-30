# Tests take engine answers, and run-every-case lists derive from their sources

Question: web test supports need engine answers, and several lists drive "run every case". A hand-written answer can hold a state the engine cannot produce, and a hand-kept list can miss what it lists. Which is the owner of each?

Call: the engine and the thing listed. A shared test journey is built by patching a new journey through the real engine. A hand-written journey kept for exact-value tests is derived again in the engine by a test that requires every node's display state, relevance, blockers and failing guards to match. A list that drives every case (store conformance cases, request paths, states per kind, fixture journeys, test modules per ladder step) is read from its source, so a new case cannot be missed. A pin that must be written in several manifests is checked against its owner by `scripts/pins`.

Alternatives: deriving every test view from the engine (rewrites the exact-value tests for little added safety once the agreement test holds); keeping the lists with reminders (they drift from what they list).

What would change it: a screen test that needs a state the engine cannot reach, recorded here with the reason.
