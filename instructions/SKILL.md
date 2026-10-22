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
   counts. Scope it with `subtree` and `depth` on a large journey. It also says when
   completing the journey is suggested (nothing left in scope, or the final milestone
   done); completing it is the person's call.
2. **Look closer only where needed.** `list_frontier` pages the ranked frontier or, with
   `filters`, the nodes that need something (`decisions_needed`, `blocked`, `stale`, and
   the others its schema lists; `blocked` is the display state). `get_node` explains
   one node: why it is relevant or blocked, its dates, and why it ranks where it does.
   `get_level` shows the graph at one level of containment.
3. **Write in small, named steps.** Each write is one patch. Give it a new `patch_id` you
   make up (`p_` and a short slug, unique per write) and the `base_revision` you read.
   The answer's `receipt.revision` is the base of your next write on that journey, unless the
   answer says `over` or `already_applied`: then read again first (see Revisions, retries, and
   errors).
4. **Offer to walk open decisions.** Whenever a journey has open decisions (you just
   created it, or you are back at it later), offer to walk them with the person
   (walk-decisions): they gate what depends on them.
5. **Report consequences.** A write answers what it newly caused (nodes that went stale,
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
| `ready` | ready (a decision: to decide; a group: not started) | nothing is in the way; work can start now |
| `active` | active | started, or a container with work beneath it started |
| `blocked` | blocked | a gate it waits on is unsatisfied |
| `conditional` | conditional | it may apply, once a decision is answered |
| `scheduled` | scheduled | waits for a date ahead: a milestone's day, or work's own not-before date |
| `snoozed` | snoozed | set aside until a date or a node |
| `done` | done (a decision: decided) | finished |
| `skipped` | skipped | skipped, or under a skipped container |
| `not_relevant` | not relevant | an answer ruled it out, whatever it recorded |

A node that rests on a decision which is itself still undecided is `conditional`, not ruled
out, though its relevance reads `not_relevant` and counts for nothing until then
(`pending_on` names the decision). A container is never `blocked` by its own children.

## Revisions, retries, and errors

- **Stale.** A write against a revision that moved is applied anyway when nothing changed
  since touches what it changes. Its answer then carries `over`, naming the revisions that
  moved and what others changed. Your view is behind by those changes: read again
  (`get_snapshot` or `get_node`) before your next write, because the receipt's revision
  includes changes you have not seen. Otherwise the write is refused as `stale`, naming what
  moved. Read again, check the change still makes sense, and write again against the new
  revision with a new `patch_id`.
- **Lost response.** If a call fails without an answer, resubmit it unchanged with the same
  `patch_id`. A write that already landed answers `already_applied` from its receipt; it is
  never applied twice. That answer cannot say whether the write was applied over others'
  changes, so read again before your next write, as after `over`. Never reuse a `patch_id`
  for a different write.
- **Rejected.** An invalid write is refused with every violation, each with its path, a
  code, and a message. Fix all of them at once; nothing was written.
- **Arguments.** A call whose arguments do not match the tool's schema is refused with the
  path that did not match.
- **Identities.** A write that names identities (an identity answer, explicit participants)
  carries the `deployment_revision` the snapshot reported. An identity has a name, emails,
  or both; a person who only signed in has emails and no name. An identity that anything
  refers to (an answer, a participant, an owner, a condition) cannot be deleted: merge it into
  the identity that remains, naming the journeys that refer to it. `list_identities` finds an
  identity by name or email; `list_journeys` with `referencing` finds the journeys that
  refer to one.
- **Deleting.** Archiving a journey or retiring a route sets it aside; deleting is permanent,
  and a deleted id is never used again. Delete only when the person asks. One `apply_patch`
  to the `deployment` (its revision is the `deployment_revision` a snapshot reports) deletes
  any number at once: a `delete_journey` with the `journey` and its `revision`, or a
  `delete_route` with the `route` and its `revision`, each revision from the `list_journeys`
  or `list_routes` row. They all go or none do: one that moved since its row was read makes
  the whole patch stale. A route or segment is deleted only while nothing uses it (no journey
  on any of its versions, no insertion of it in any journey or route version); the refusal
  names what does. Delete those journeys in the same patch, or retire the route instead.

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
`relink`). A proposal you lost the id of is found again with `list_proposals` (the ones
under a destination, the ones waiting on the caller, or the ones that would create a journey
or route); a proposal stays listed until it is applied or discarded.

Work marked `requires_note` cannot be completed until it has a note of its own (a link,
or a note on a child or the journey, does not count). Send the note and the completion as
one `apply_patch` (`add_annotation` with the node and the text, then the `complete`
transition); `transition_node` alone is refused with the `has_note` guard. Bypass it with
`override` only when the person says no note is needed. Removing the last note later marks
a finished node `stale` with "missing note".

## Editing structure

Structural edits are mutations, in a proposal's `mutations` or, when the person asked for
the change, in `apply_patch` (a journey, a route's draft, or the deployment; the tool for
a state change, when there is one, is simpler). Several edits are one patch, one revision.

- **One field of a node:** `set_node_field` with the field and its new value. Prefer it
  for any small edit. It does not take `requires`.
- **A node rewritten whole:** `replace_node` with the full node, `requires` included. Use
  it to change a node's kind or answer type, or many fields at once; send everything, it
  replaces rather than merges.
- **Edges:** `add_edge` and `remove_edge`, each `{node, requires}`.
- **Removing a node:** `remove_node` takes a removal object that names the node, its
  descendants, every edge into or out of that subtree, and the resources, notes, and
  participations on it. One that leaves any of it out is refused (never widened). In a
  journey do not build it by hand: `get_node` with `removal` true returns the exact
  `removal` for now, and the `outside_references` the same patch must also change (a
  page at a time; `next` goes back as `references_cursor`). Each names a `holder` node
  outside, the `field` that mentions a node being removed, and the `target`. Clear or
  rewrite it with `set_node_field` (`relevant_when`, `due_by`, `not_before`, `opens_at`,
  `closes_at`, or `feeds_milestone`), `edit_resource` for a message draft, or `unsnooze`.
  Read again if the revision moved. A route's draft has no such read: assemble the removal
  from `get_route`, and expect the refusal to list anything missed.
- **Roles and kinds:** `add_role`, `edit_role`, `remove_role` (refused while a
  participation or `fills_role` names it), the matching `..._participation_kind` ops, and
  `set_default_owner`. `assign` is the tool for who does a node; a role no decision
  fills is filled with `fill_role` and emptied with `clear_role_fill`.
- **Resources and notes:** `add_resource`, `edit_resource`, `remove_resource`; and
  `add_annotation`, `edit_annotation`, `remove_annotation`.
- **A placeholder's breakdown flag:** `set_atomic`.
- **The journey:** `edit_journey` (name, description) and `set_journey_status` (completing
  or archiving is the person's call). `delete_journey` is permanent; only when the
  person asks. Also `discard_proposal` for a proposal nobody will apply.
- **Routes:** `create_route`, `edit_route`, `set_route_retired` (hides it from new
  journeys), and a draft's `discard_draft`; `open_draft` and `publish_draft` are also
  tools.
- **Leave to Cairn:** `set_provenance` and `mark_local_edit` record lineage for upgrades
  and are written by upgrades and re-linking, not by hand.

## Dates

Say each kind of date with the field made for it:

- **A deadline** is `due_by`: the node finishes by a milestone, a date answer, or the
  journey's start, give or take an offset. It never holds work back.
- **A structural not-until** is `not_before`: the work, or the decision, cannot happen before
  that date. While its date is ahead the node is `scheduled` and off the next list; it can
  still be started early, and finishing it before the date is refused with the
  `date_arrived` guard unless the person gives a reason to bypass it; finished that way,
  it reads done early.
- **A personal not-now** is a snooze: someone sets the node aside until a date or another
  node is done. It changes no plan date.
- **An event** is a milestone. Pinned, it is `scheduled` until its day, then ready to mark
  done; with `auto_done`, it marks itself done on its date; with no pin, it is ready, and
  someone marks it done on the day it happens. Marking a milestone done early is allowed and
  records when it happened.

Whatever waits on a `scheduled` node is `blocked`, and its `waits_on` entry carries the date.

## Stages

A **stage** is a group bounded by milestones that sit outside it. `opens_at` gates its
contents (they wait for the milestone) unless `gates: false`; `closes_at` bounds the
group's finish, and so its contents' due dates, unless `closes: false`. Consecutive
stages share a milestone: one stage's `closes_at` is the next stage's `opens_at`.

Put inside a stage the work done during it. Work that can as well be done ahead of time
can sit in a stage with `gates: false`: its contents can start and finish before the stage
opens, and the opening dates only contents not yet started. Starting work early is never a
shortfall, and neither is finishing it ahead of the opening, or ahead of any dependency with
a reason (a guard bypass): its `dates.done_early` says, on today's dates, how many days
early and ahead of what. Its other dates, such as its deadline, still apply. Prep that must
be done before the stage belongs outside it, with a `due_by` before the opening milestone.

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
siblings. New journeys, routes, and proposals take ids you choose: `j_`, a route slug, and
`pr_`, with a slug. A new identity's key is `who_` and 16 random lowercase letters and digits;
never derive it from a name. Read keys from the snapshot or the route; never guess them.

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
| Find things | `list_identities`, `list_journeys`, `list_routes`, `list_proposals`, `get_route`, `search` |
| Change state | `answer_decision`, `transition_node`, `assign`, `set_date`, `snooze`, `unsnooze`, `override`, `resolve_date_conflict` |
| Change structure | `create_proposal`, `list_proposals`, `get_proposal`, `edit_proposal`, `apply_proposal`, `apply_patch` |
| Journeys and routes | `create_journey`, `open_draft`, `publish_draft`, `import_route`, `export_route` |
| Lineage | `upgrade`, `save_as_route`, `relink` |
| People and teams | `manage_identity` (create, edit, merge, delete) |

Lists that can grow come a page at a time: pass the `next` you were given back as the
cursor the list names (`cursor` or `after`; a node's children and notes take
`children_cursor` and `annotations_cursor`, a route's versions `versions_cursor`). A list
answered with only a `total` says where to read the rest. A page is a few hundred items at most; prefer filters and
scopes over reading every page.
