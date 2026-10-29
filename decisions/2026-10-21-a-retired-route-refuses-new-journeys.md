# A retired route refuses new journeys, not only hides them

Question (from the rule-journey-screens unit): a retired route is "hidden from new-journey creation". The screens hid it, but the engine accepted `create_journey` from a retired route, so an agent could start what no screen offers, while inserting a retired route was already refused. Is retirement a filter on the screens or a rule the engine enforces?

Call: the engine refuses it. Creating a journey from a retired route is rejected as an invalid lineage, with a message saying the route is retired. Journeys already on the route keep their lineage. This matches insertion, where a retired route cannot be inserted either. On the screens one predicate, `startsJourneys` in `web/app/src/routes/model.ts`, says whether a route starts journeys (not retired); the new-journey form, the version card's Start a journey and the default version all read it.

Alternatives: hiding only (the agent surface and the screens would keep disagreeing); changing a journey's lineage when its route retires (retirement leaves existing journeys alone).

What would change it: a need to start a journey from a retired route on purpose, for example to reproduce an old process, which would want an explicit override rather than a silent path.
