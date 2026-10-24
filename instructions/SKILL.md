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
what is relevant. Blocking, dates, rank, and the frontier are derived by Cairn on every
read; never compute them yourself. You act as the user who connected you, with the same
capabilities; every write records you and that user.

## The loop

1. **Read state first.** Call `get_snapshot` before anything else on a journey. It is an
   overview with no node list: the answers in effect, counts, the top of the ranked acting
   frontier, open decisions by rank, placeholders needing breakdown, unassigned items, and
   shortfalls. Scope it with `subtree` on a large journey. It says when completing the
   journey is suggested; completing it is the person's call.
2. **Look closer only where needed.** `list_nodes` pages nodes in rank order: every node,
   or with `filters` those that need something (`next_up` is the acting frontier,
   `decisions_needed`, `blocked`, `stale`, and the others its schema lists). Scope it with
   `within` and `depth`. `get_node` explains one node: why it is relevant or
   blocked, its dates, and why it ranks where it does. `get_level` shows the graph at one
   level of containment.
3. **Write in small, named steps**, by the rules below.
4. **Offer to walk open decisions.** Whenever a journey has open decisions (you just
   created it, or you are back at it later), offer to walk them with the person
   (walk-decisions): they gate what depends on them.
5. **Report consequences.** A write answers what it newly caused (nodes that went stale,
   shortfalls, relevance changes, and finished work that may not apply because the decision
   its relevance reads is unanswered). Tell the person what changed, not just that it worked.

## Rules every write shares

- **A new `patch_id` per write**: `p_` and a short slug you make up. Never reuse one for a
  different write.
- **`base_revision` is the revision you last read** of what you write: the snapshot's (or
  `get_node`'s) `revision`, or the `revision` your last write there answered.
- **`over` or `already_applied` in an answer: read again** (`get_snapshot` or `get_node`)
  before the next write. `over` means the write landed although the revision had moved,
  and names what others changed; your view is behind, and the answer's revision includes
  changes you have not seen.
- **No answer (a lost response): resubmit the same call with the same `patch_id`.** A write
  that landed answers `already_applied` from its receipt and is never applied twice.
- **Refused as `stale`**: what moved touches what you change. Read again, check the change still
  makes sense, and write against the new revision with a new `patch_id`.
- **Rejected**: every violation is listed with its path; fix them all at once, nothing was
  written. Arguments off the tool's schema are refused with the path that did not match.

## Ids and keys

Every node, role, and participation kind has a `key` (`n_`, `r_`, and so on, with a slug)
that never changes and is never reused; its `id` is a readable slug unique among its
siblings. New journeys, routes, and proposals take ids you choose: `j_`, a route slug, and
`pr_`, with a slug. A new identity's key is `who_` and 16 random lowercase letters and digits;
never derive it from a name. Read keys from the snapshot or the route; never guess them.

## Status: read `display_state`

A node carries a stored `state` (`todo`, `open`, `done`, and so on), which says which
transition applies, and a derived `display_state`, which says its status: use it when you
tell the person. A decision an answer ruled out is still `open` but `not_relevant`. The
counts come both ways: `by_state` and `by_display_state`, over the in-scope nodes.

| `display_state` | Say | Meaning |
|---|---|---|
| `ready` | ready (a decision: to decide; a group: not started) | nothing is in the way; work can start now |
| `active` | active | started, or a container with work beneath it started |
| `blocked` | blocked | a gate it waits on is unsatisfied |
| `conditional` | conditional | it may apply, once a decision is answered |
| `scheduled` | scheduled | waits for a date ahead: a milestone's day, or work's own not-before date |
| `snoozed` | snoozed | set aside until a date or a node |
| `done` | done (a decision: decided) | finished |
| `skipped` | skipped | skipped, or under a skipped container or a group marked done |
| `not_relevant` | not relevant | an answer ruled it out, whatever it recorded |

A node that rests on a decision which is itself still undecided is `conditional`, not ruled
out, though its relevance reads `not_relevant` and counts for nothing until then
(`pending_on` names the decision). A container is never `blocked` by its own children.

## Identities and deleting

- **Identities.** A write that names identities (an identity answer, explicit participants,
  a journey's owners) carries the `deployment_revision` the snapshot reported. An identity has a name, emails,
  or both; a person who only signed in has emails and no name. An identity that anything
  refers to (an answer, a participant, an owner, a condition) cannot be deleted: merge it into
  the identity that remains, naming the journeys that refer to it. `list_identities` finds an
  identity by name or email; `list_journeys` with `referencing` finds the journeys that
  refer to one, and with `owners` the journeys it owns.
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

To go ahead of a guard on purpose (start or finish ahead of what a node waits on or of its own
date, finish without a note, link, ticks or breakdown, answer ahead), send a guard bypass in
the same `apply_patch` as the move: an `apply_override` mutation with `guard_bypass` naming
the guards, then the transition or answer. The reason is optional. A bypass alone covers
nothing, and `transition_node` alone is refused. Do it only when the person says to; the
engine records what the move went ahead of.

A snooze sets a node aside until a date or until another node is done. Snooze a container
(a group, or work with children) to set its whole subtree aside in one move: every
descendant is then held off the next list, names the container in `snoozed_via`, and stays
actionable in the model, still blocking and counting for priority. A target inside the
subtree, or one that depends on anything in it, is refused. A descendant held only through
its container cannot be unsnoozed on its own (the refusal names the container to unsnooze);
a descendant's own snooze is independent of the container's, and completing a descendant
does not lift the container's. Any transition on the container itself, such as starting it
or skipping a group, clears its snooze.

To finish a group or stage while work inside it is still open, `mark_done` it with a reason
(`transition_node`): it reads done, and its unfinished work reads skipped as under a skip.
Kept work stays open and still holds the group's dependents; `reopen` removes the mark. Work
with children has no `mark_done`: complete it, with a guard bypass when its
children are not done.

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
transition); `transition_node` alone is refused with the `has_note` guard. Bypass it only
when the person says no note is needed. Removing the last note later marks
a finished node `stale` with "missing note".

A deliverable or action can carry a `checklist` (items with an `id` and a `title`, at
most 50), and with `requires_checklist` it cannot be completed until every item is ticked.
Tick with the `set_ticked` mutation (`node`, `item`, `ticked`) and send the ticks and the
completion as one `apply_patch`; `transition_node` alone is refused with the `all_ticked`
guard, one failure per unticked item. Ticking or unticking sets the item's state; sending
the state it already has changes nothing. An item not on the checklist is refused. Bypass
the guard only when the person says the items need not be ticked. Unticking
an item later, or adding one, marks a finished node `stale` with "unticked item". Ticks are
kept when a node is reopened, and removing an item removes its tick.

## Editing structure

Structural edits are mutations, in a proposal's `mutations` or, when the person asked for
the change, in `apply_patch` (a journey, a route's draft, or the deployment; the tool for
a state change, when there is one, is simpler). Several edits are one patch, one revision.

- **One field of a node:** `set_node_field` with the field and its new value. Prefer it
  for any small edit. It does not take `requires`.
- **A node rewritten whole:** `replace_node` with the full node, `requires` included. Use
  it to change a node's kind or answer type, or many fields at once; send everything, it
  replaces rather than merges.
  A `checklist` field value replaces the whole list, and a tick whose item is gone from the
  list is dropped with it.
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
- **The journey:** `edit_journey` (the name and description, both as given, so send the
  snapshot's description to keep it; and `owners`, kept when left out, replaced when given,
  cleared by an empty list) and `set_journey_status` (completing
  or archiving is the person's call). Owners are never assumed, the creator included: leave them empty unless
  the person names them. `delete_journey` is permanent; only when the
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
  still be started or finished early, but each is refused with the `date_arrived` guard
  unless it carries a guard bypass; finished that way, it reads done early.
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
opens, and the opening dates only contents not yet started. Starting or finishing work
early is never a shortfall. Ahead of a dependency or a gating opening it takes a guard bypass,
and a finished node's `dates.done_early` says, on today's dates, how many days early and ahead
of what. Its other dates, such as its deadline, still apply. Prep that must be done before the stage belongs outside it, with a `due_by` before the opening milestone.

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
| Where things stand | `get_snapshot`, `list_nodes`, `get_node`, `get_level`, `get_history` |
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
