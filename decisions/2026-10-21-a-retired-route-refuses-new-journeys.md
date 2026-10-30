# A retired route refuses new journeys, not only hides them

**Question.** Is a route's retirement a filter on the screens, or a rule the engine enforces?

**Call.** The engine enforces it: creating a journey from a retired route, or inserting one, is rejected, with a message saying the route is retired. Journeys already on it keep their lineage. The screens read one predicate for whether a route starts journeys, so they offer what the engine accepts.

**Alternatives.** Hiding only (an agent could start what no screen offers); changing existing journeys' lineage on retirement (retirement leaves them alone).

**What would change it.** A need to start a journey from a retired route on purpose, which would want an explicit override rather than a silent path.
