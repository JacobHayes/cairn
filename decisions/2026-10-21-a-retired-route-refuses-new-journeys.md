# A retired route refuses new journeys, not only hides them

Question (from the rule-journey-screens unit): a retired route is "hidden from new-journey creation". The screens hid it, but the engine accepted `create_journey` from a retired route, so an agent could start what no screen offers, while inserting a retired segment was already refused. Is retirement a filter on the screens or a rule the engine enforces?

Call: the engine refuses it. Creating a journey from a retired route is rejected as an invalid lineage, with a message saying the route is retired. Journeys already on the route keep it: they still upgrade, re-link and save as before. This matches the segment half, where a retired segment cannot be inserted. On the screens one predicate, `startsJourneys` in `web/app/src/routes/model.ts`, says whether a route starts journeys (not a segment, not retired); the new-journey form, the version card's Start a journey and the default version all read it.

Alternatives: hiding only (the agent surface and the screens would keep disagreeing); refusing re-link to a retired route as well (re-link moves an existing journey's lineage, which retirement leaves alone).

What would change it: a need to start a journey from a retired route on purpose, for example to reproduce an old process, which would want an explicit override rather than a silent path.
