---
name: cairn
description: Work with Cairn, a process tracker where routes are templates and journeys are live graphs of decisions, deliverables, actions, milestones, and groups. Use it to read where a journey stands, walk its open decisions, record answers, break down placeholders, author routes in conversation, resolve date conflicts, and propose changes for people to review.
---

# Working with Cairn

Cairn tracks processes as graphs. A **route** is a template with numbered, immutable
published versions and at most one draft. A **journey** is a live copy for one real
situation: its nodes, the answers given, each node's state, who participates, pins, and
notes. Nodes are decisions (typed questions), deliverables and actions (work), milestones
(dates), and groups (containers). Edges are hard dependencies. Conditions on answers decide
what is relevant. Everything else (blocking, dates, gravity, rank, the frontier) is derived
by Cairn on every read; you never compute it yourself.

You act as the user who connected you, with the same capabilities. Every write records you
and that user.

## The loop

1. **Read state first.** Call `get_snapshot` before anything else on a journey. It answers
   the answers in effect, a page of in-scope nodes, the ranked acting frontier, open
   decisions by rank, placeholders needing breakdown, unassigned items, shortfalls, and
   counts. Scope it with `subtree` and `depth` on a large journey.
2. **Look closer only where needed.** `list_frontier` pages the ranked frontier or, with
   `filters`, the nodes that need something (`decisions_needed`, `needs_breakdown`,
   `unassigned`, `active`, `blocked`, `stale`, `overdue`, `shortfall`). `get_node` explains
   one node: why it is relevant or blocked, its dates, and why it ranks where it does.
   `get_level` shows the graph at one level of containment.
3. **Write in small, named steps.** Each write is one patch. Give it a new `patch_id` you
   make up (`p_` and a short slug, unique per write) and the `base_revision` you read.
   The answer's `receipt.revision` is the base of your next write on that journey.
4. **Report consequences.** A write answers what it newly caused (nodes that went stale,
   shortfalls, relevance changes, and finished work that may not apply because the decision
   its relevance reads is unanswered). Tell the person what changed, not just that it worked.

## Status: read `display_state`

Every node carries a stored `state` (what its transitions act on: `todo`, `open`, `done`,
and so on) and a derived `display_state`. To say what a node's status is, use
`display_state`; keep `state` for which transition applies. A decision that an answer ruled
out still has the stored state `open`, and its `display_state` is `not_relevant`. The counts
come both ways: `by_state` counts by stored state and `by_display_state` by display state,
both over the in-scope nodes.

| `display_state` | Say | Meaning |
|---|---|---|
| `ready` | ready (a decision: to decide) | nothing is in the way; work can start now |
| `active` | active | started, or a container with work beneath it started |
| `blocked` | blocked | a gate it waits on is unsatisfied |
| `conditional` | conditional | it may apply, once a decision is answered |
| `scheduled` | scheduled | an auto-reach milestone whose date is ahead |
| `snoozed` | snoozed | set aside until a date or a node |
| `done` | done (a decision: decided; a milestone: reached) | finished |
| `skipped` | skipped | skipped, or under a skipped container |
| `not_relevant` | not relevant | an answer ruled it out, whatever it recorded |

A node that rests on a decision which is itself still undecided is `conditional`, not ruled
out, though its relevance reads `not_relevant` and counts for nothing until then
(`pending_on` names the decision). A container is never `blocked` by its own children.

## Revisions, retries, and errors

- **Stale.** A write against a revision that moved is refused as `stale`, naming what
  moved. Read again (`get_snapshot` or `get_node`), check the change still makes sense, and
  write again against the new revision with a new `patch_id`.
- **Lost response.** If a call fails without an answer, resubmit it unchanged with the same
  `patch_id`. A write that already landed answers `already_applied` from its receipt; it is
  never applied twice. Never reuse a `patch_id` for a different write.
- **Rejected.** An invalid write is refused with every violation, each with its path, a
  code, and a message. Fix all of them at once; nothing was written.
- **Arguments.** A call whose arguments do not match the tool's schema is refused with the
  path that did not match.
- **Entities.** A write that names entities (an entity answer, explicit participants)
  carries the `deployment_revision` the snapshot reported.

## Direct writes or proposals

Write directly when the person asked for a state change: answers (`answer_decision`),
transitions (`transition_node`), participations (`assign`), pins and actual dates
(`set_date`), snoozes (`snooze`, `unsnooze`), and overrides (`override`).

A snooze sets a node aside until a date or until another node is done. Snooze a container
(a group, or work with children) to set its whole subtree aside in one move: every
descendant is then held off the next list, names the container in `snoozed_via`, and stays
actionable in the model, still blocking and counting for priority. A target inside the
subtree, or one that depends on anything in it, is refused. A descendant held only through
its container cannot be unsnoozed on its own (the refusal names the container to unsnooze);
a descendant's own snooze is independent of the container's, and completing a descendant
does not lift the container's. Any transition on the container itself, such as starting it
or skipping a group, clears its snooze.

Draft a **proposal** (`create_proposal`) when the change is structural (adding, removing, or
moving nodes; edges; roles; conditions; date rules), when it touches many nodes, or when the
person should see it whole first. A proposal has an id you choose (`pr_` and a slug), its
own editing revision, and the destination revision it was drafted against. Review it with
`get_proposal` and `review`; a person applies it with `apply_proposal`. Upgrades, saving a
journey as a route, and re-linking are always proposals (`upgrade`, `save_as_route`,
`relink`).

`apply_patch` takes any patch to a journey, a route's draft, or the deployment, when no
other tool fits.

Work marked `requires_note` cannot be completed until it has a note of its own (a link,
or a note on a child or the journey, does not count). Send the note and the completion as
one `apply_patch` (`add_annotation` with the node and the text, then the `complete`
transition); `transition_node` alone is refused with the `has_note` guard. Bypass it with
`override` only when the person says no note is needed. Removing the last note later marks
a finished node `stale` with "missing note".

## Reuse a segment

A **segment** is a reusable piece of a process (a security review, a reference check): a
route whose `kind` is `segment`, with one root, inserted and never started. Before you
structure a recurring piece by hand, look for one with `list_routes` and `kind` `segment`.
Insert it with an `insert_segment` mutation in a proposal, under the right `parent`, with `edges` that
make its root wait for what comes before and make what comes after wait for its root. Roles
default by id, so a segment's reviewer role becomes the journey's role with the same id. If the journey
already has a decision that fills that role, `omit` the segment's own. Propose it rather than
writing it directly, and let a person apply it. After insertion the nodes are ordinary: edit,
skip, or remove them as you would any other. A segment cannot be started as a journey, and
it declares no final milestone or default owner. The insert-segment workflow has an example.

## Ids and keys

Every node, role, and participation kind has a `key` (`n_`, `r_`, and so on, with a slug)
that never changes and is never reused; its `id` is a readable slug unique among its
siblings. New journeys, routes, proposals, and entities take ids you choose: `j_`, a route
slug, `pr_`, and `e_` with a slug. Read keys from the snapshot or the route; never guess
them.

## Workflows

Each workflow is also served as an MCP prompt by its name.

- [author-route](workflows/author-route.md): author or revise a route in conversation.
- [insert-segment](workflows/insert-segment.md): reuse a segment by proposing its insertion.
- [structure-journey](workflows/structure-journey.md): give an empty journey its shape.
- [walk-decisions](workflows/walk-decisions.md): walk the open decisions with a person.
- [record-answers](workflows/record-answers.md): record answers given in conversation.
- [propose-breakdown](workflows/propose-breakdown.md): break a placeholder down.
- [summarize-frontier](workflows/summarize-frontier.md): say where a journey stands.
- [resolve-date-conflict](workflows/resolve-date-conflict.md): fix contradictory dates.
- [handle-stale-proposal](workflows/handle-stale-proposal.md): refresh a proposal whose
  destination moved.

## Tools

| Need | Tools |
|---|---|
| Where things stand | `get_snapshot`, `list_frontier`, `get_node`, `get_level`, `get_history` |
| Find things | `list_journeys`, `list_routes`, `get_route`, `search` |
| Change state | `answer_decision`, `transition_node`, `assign`, `set_date`, `snooze`, `unsnooze`, `override`, `resolve_date_conflict` |
| Change structure | `create_proposal`, `get_proposal`, `edit_proposal`, `apply_proposal`, `apply_patch` |
| Journeys and routes | `create_journey`, `open_draft`, `publish_draft`, `import_route`, `export_route` |
| Lineage | `upgrade`, `save_as_route`, `relink` |
| People and teams | `manage_entity` |

Lists that can grow come a page at a time: pass the `next` you were given back as the
cursor the list names (`cursor` or `after`; a node's children and notes take
`children_cursor` and `annotations_cursor`, a route's versions `versions_cursor`). A list
answered with only a `total` says where to read the rest. A page is a few hundred items at most; prefer filters and
scopes over reading every page.
