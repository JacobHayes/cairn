# Blocked always means the display state blocked

**Question.** The engine's internal blocking flag is true for a container that cannot finish until its children do, while the display state never calls a container blocked by its own children. Which meaning do filters, counts and served fields use?

**Call.** Everything a person or an agent reads says the display state: the `blocked` list filter, snapshot counts (`by_display_state`), a row's `display_state`, and a container's `all_blocked` (every relevant open child shows blocked). The blocking flag stays internal, feeding readiness, dependents' gates and the hidden-prerequisites marker; `blocked_by` lists a container's unfinished children, because that is what its unfinished state is made of. No served field duplicates another's meaning under a second name.

**Alternatives.** Serving the flag beside the display state (two counts of blocked work that disagree); removing the container case from the flag (it feeds the container's own finish and its dependents' gates).

**What would change it.** A client that needs "cannot finish until its children do", which would get its own clearly named field.
