---
description: Author or revise a route (a process template) in conversation, as proposals on its draft that a person reviews and publishes.
---

# Author a route in conversation

A route is authored on its draft. Conversation is the expected way to author one: you
draft, the person reviews on the canvas, and publishing makes the next version.

1. **Find or start the route.** `list_routes` shows the routes and whether a draft is open.
   For a new route, choose a slug for it and call `open_draft` with `create` (its name and
   description) and `base_revision` 0. For an existing route with no draft open, call
   `open_draft` against its `revision`. `get_route` reads the draft (or a published
   `version`) with its roles, participation kinds, and a page of nodes.
2. **Ask before you draft.** Learn the stages of the process, who takes part (roles such as
   an owner or a reviewer), what has to be decided up front, and which dates matter. Model:
   - questions as decisions (with an answer type and, for choices, the choices);
   - work as deliverables (an output) and actions (a step), grouped into groups;
   - points in time as milestones;
   - "only if" as a condition on a decision's answer;
   - "after" as an edge (`requires`), and dates as date rules (`due_by`, `not_before`)
     measured from a milestone or a date answer;
   - work each journey must break down as a `placeholder`.
3. **Draft it as a proposal on the route.** `create_proposal` with `destination`
   `{"route": "<route>"}`, the route's current revision as the draft's
   `destination_revision`, and `add_role`, `add_participation_kind`, `add_node`, and
   `add_edge` mutations in order (parents before children). Give every node a `key`
   (`n_` and a slug) and an `id` (a slug unique among its siblings).
4. **Check it.** `get_proposal` with `review` lists every violation the draft would cause
   (a cycle, an unknown reference, a date rule that contradicts another). Fix them with
   `edit_proposal` and `replace`, against the proposal's editing revision. The review also
   lists **notices**: work with no chain to or from the route's `final` milestone, which
   neither its priority nor its dates feel (usually a missing edge or date rule; a decision
   that fills a role is exempt). A notice never blocks anything: add the missing edge, or
   tell the person it is intended.
5. **Hand it over.** Tell the person what the proposal adds and ask them to review and
   apply it. When they are happy with the draft, `publish_draft` makes it the next
   version; journeys on older versions can then upgrade. `publish_draft` and `import_route`
   list the same notices in their result.

A route file can also be imported whole with `import_route` (YAML), and any version
exported with `export_route`.
