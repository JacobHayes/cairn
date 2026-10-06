---
description: Give a journey that started empty its structure, drafted as one proposal for the person to review.
---

# Structure an empty journey

A journey can start with no route. Its structure is drafted the same way as a route's,
but on the journey itself.

1. **Create it if needed.** `create_journey` with an id you choose (`j_` and a slug), a
   name, and no `from`.
2. **Understand the situation.** Ask what has to happen, what has to be decided first, who
   is involved, and what dates are fixed. Keep it small: a few groups with the decisions,
   deliverables, actions, and milestones under them.
3. **Draft one proposal.** `create_proposal` with `destination` `{"journey": "<journey>"}`,
   the journey's revision as `destination_revision`, and the `add_role`, `add_node`, and
   `add_edge` mutations in order. People involved who are not yet entities are created in
   the same proposal with `create_entity`.
4. **Review and hand over.** `get_proposal` with `review` shows the frontier the journey
   would have and any violations. Fix violations with `edit_proposal`, then summarize the
   structure and what would be first to do, and ask the person to apply it.
5. **Afterwards.** Once applied, `get_snapshot` shows the journey as it now stands. If the
   person wants to reuse the structure, `save_as_route` drafts a new route from it.
