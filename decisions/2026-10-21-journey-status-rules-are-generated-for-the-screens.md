# Journey status rules are generated for the screens

Question (from the rule-journey-screens unit): the screens kept hand copies of the journey status rules: which moves each status allows, that an archived journey takes no change, and that a completed or archived journey holds no current work for anyone. The journey card said "You have 3 open items here" on a completed journey while its Summary and the index said there was nothing for the viewer. Where should these rules live?

Call: on `JourneyStatus` in the schema crate (`moves`, `accepts_changes`, `holds_current_work`), read by the engine's mutation checks and emitted into the generated rules (`JOURNEY_STATUSES`, `STATUS_MOVES`, `STATUS_ACCEPTS_CHANGES`, `STATUS_HOLDS_CURRENT_WORK`). The screens keep only the words. The journey card, the Summary, the index's Only mine and the figures read `STATUS_HOLDS_CURRENT_WORK`; the per-journey mine projection stays status-blind, since it answers what a viewer owns in one journey and the card already holds the journey's status.

Alternatives: making the engine's per-journey mine empty for a closed journey (it would change a served read every caller sees, and a closed journey's owned items are still worth listing as unfinished on its Summary); one hand-written `closed(status)` predicate in the web app (a copy of the engine's rule again).

What would change it: a served surface that lists a closed journey's work as current, which would argue for the projection itself to carry the rule.
