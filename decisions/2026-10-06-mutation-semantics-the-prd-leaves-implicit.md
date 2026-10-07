# Mutation semantics the PRD leaves implicit

- Question: the PRD defines what can change but not every mutation's shape; three readings shape the engine.
- Call: (1) start and reach take no date: apply records today (a derive input) and `set_recorded_date` edits it, so a mutation never carries a default the server must trust (F1, F2). (2) A guard bypass is an override mutation applied in the same patch as the transition it covers, so it is its own event as J1 lists ("guard bypassed"), and the engine records the specific failures present on it (D4). (3) In a journey, editing a node's weight or participations is state (B5, glossary), every other field edit is structural, and every change to a route is structural; applying a proposal is structural, since its content is not in the patch.
- Alternatives: dates on the transitions; a bypass flag on the transition mutation (one event, which J1's separate event type argues against).
- What would change it: the engine (2.1) finding a guard bypass in a separate mutation awkward to tie to its transition.
