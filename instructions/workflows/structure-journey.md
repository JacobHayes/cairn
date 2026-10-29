---
description: Give a journey that started empty its structure, drafted as one proposal for the person to review.
---

# Structure an empty journey

A journey can start with no route. Its structure is drafted the same way as a route's,
but on the journey itself.

1. **Create it if needed.** `apply_patch` to `{"journey": "<id>"}` (an id you choose, `j_`
   and a slug) at `base_revision` 0 with one `create_journey`: a name, no `from`, and
   `owners` if the person named who owns it.
2. **Understand the situation.** Ask what has to happen, what has to be decided first, who
   is involved, and what dates are fixed. Keep it small: a few groups with the decisions,
   deliverables, actions, and milestones under them. For phases bounded by milestones, see `opens_at` and `closes_at` in `apply_patch`'s node type.
3. **Draft one proposal.** `create_proposal` with `destination` `{"journey": "<journey>"}`,
   the journey's revision as `destination_revision`, and the `add_role`, `add_node`, and
   `add_edge` mutations in order. People involved who are not yet identities are created in
   the same proposal with `create_identity`.
4. **Review and hand over.** `get_proposal` with `review` shows the frontier the journey
   would have and any violations. Fix violations with `edit_proposal`, then summarize the
   structure and what would be first to do, and ask the person to apply it.
5. **Afterwards.** Once applied, `get_snapshot` and `list_nodes` show the journey as it now stands. If the
   person wants to reuse the structure, `create_proposal` with `save_as_route` drafts a new
   route from it.
