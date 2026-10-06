---
description: Break down a placeholder (work the route expects each journey to split) into children, as a proposal.
---

# Propose a breakdown

A placeholder is a deliverable or action the route expects each journey to break down. It
cannot be completed until it has children or is marked `atomic`.

1. **Find them.** `get_snapshot` lists `needs_breakdown`; `list_frontier` with `filters`
   `["needs_breakdown"]` ranks them.
2. **Understand it.** `get_node` shows its description, resources (the route's guidance on
   how to break it down), owner, and dates.
3. **Ask, then draft.** Ask the person what the parts are. Draft a proposal on the journey
   (`create_proposal`, `destination` `{"journey": "<journey>"}`) with an `add_node` per
   child under the placeholder (`parent` is its key), each with a new `key`, an `id`, a
   `kind` (deliverable or action), a `title`, and an `estimate` in days when known. For
   "one per person in a role", add one child per member, each naming that member as its
   owner in its `participations`.
4. **Or mark it atomic.** When it really is one piece of work, `apply_patch` with
   `set_atomic` and `atomic` true, after the person agrees.
5. **Review and hand over.** `get_proposal` with `review` shows the frontier after; ask the
   person to apply it.
