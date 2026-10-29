---
description: Reuse a route, a saved piece of a process, by proposing its insertion into a journey or a route draft.
---

# Insert a route

Any route can be inserted under a node of a journey or a route draft. Inserting copies a
published version in one step; afterwards the nodes are ordinary and can be edited like any
other. The copied root keeps a pointer to the route and version it came from, nothing more.

1. **Find it.** `list_routes` lists the routes. `get_route` with a `version` reads its nodes,
   roles, and participation kinds, so you know what it asks (decisions, work, and the roles
   it uses). Use the latest published version unless the person names another.
2. **Choose where.** Pick the `parent` (a group, deliverable, or action; leave it out for
   the top level) and what the route's root should wait for and what should wait for it.
   Wiring goes through the root: containment carries it to everything inside.
3. **Draft it as a proposal.** `create_proposal` on the journey (or route) with one
   `insert_route` mutation: a new `insertion` key (`i_` and a slug; every new key derives
   from it), the `route` as `{"route": ..., "version": ...}`, the `parent`, and `edges`, each
   joining one node of the route and one node of the graph:

   ```json
   {"op": "insert_route", "insertion": "i_review", "parent": "n_launch",
    "route": {"route": "security-review", "version": 1},
    "edges": [
      {"node": {"route": "n_review"}, "requires": {"host": "n_design"}},
      {"node": {"host": "n_release"}, "requires": {"route": "n_review"}}]}
   ```

   Roles and kinds default by id: a role of the route takes the journey's role with the same
   id and cardinality, and is added when there is none. Name a choice only to differ:
   `"roles": {"r_reviewer": {"existing": "r_lead"}}`, or `"add"`.
4. **Check it.** `get_proposal` with `review` lists every violation. If two decisions would
   fill one role (the journey already has a *Who reviews?*), leave the route's out with
   `"omit": ["n_who_reviews"]`; a node left out takes its subtree with it. An edge that
   would close a cycle rejects the whole insert.
5. **Hand it over.** Say what the insertion adds and where, and ask the person to review
   and apply it. Each root gets the next free id among its siblings (`security-review-2`
   when the first is there); for a route with one root, name it with `root_title` if the
   journey needs a clearer one.

To insert into a published route, put an `open_draft` mutation (`source: edit`) before the
insertion in the same proposal.
