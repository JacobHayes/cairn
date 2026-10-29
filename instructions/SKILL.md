---
name: cairn
description: Work with Cairn, a process tracker where routes are templates and journeys are live graphs of decisions, deliverables, actions, milestones, and groups. Use it to read where a journey stands, walk its open decisions, record answers, break down placeholders, author routes in conversation, resolve date conflicts, and propose changes for people to review.
---

# Working with Cairn

Cairn tracks processes as graphs. A **route** is a template with numbered, immutable
published versions; a **journey** is a live copy for one real situation: nodes (decisions,
deliverables, actions, milestones, groups), answers, node states, participants, notes.
Blocking, dates, rank and the frontier are derived on every read; never compute them. You act
as the user who connected you, and every write records you and that user.

## The loop

1. **Read first.** `get_snapshot` before anything else on a journey: facts and counts, no
   nodes. Each count is the total of a `list_nodes` filter (`next_up`, `open_decision`,
   `needs_breakdown`, `unassigned`, `shortfall`, `stalled`); list the ones you need. Look
   closer with `get_node`, `get_level`.
2. **Write in small, named steps** (below).
3. **Offer to walk open decisions** whenever a journey has some: they gate what depends on
   them.
4. **Report consequences.** A write answers what it newly caused (stale nodes, shortfalls,
   relevance changes); tell the person what changed, not just that it worked.

Before a job a workflow covers, read it with `get_workflow`.

## Writes

- A new `patch_id` per write: `p_` and a slug you make up; never reuse one.
- `base_revision` is the revision you last read of the target: from the read or your last write.
- `already_applied` in an answer: read again before the next write.
- No answer (lost response): resubmit the same call, same `patch_id`; it is never applied twice.
- Refused as `stale`: read again, check the change still makes sense, write with a new `patch_id`.
- Rejected: every violation is listed with its path; fix them all. Nothing was written.

## Ids

Nodes, roles and participation kinds have a permanent `key` (`n_`, `r_`, ...) and a slug `id`
unique among siblings; read keys from `list_nodes` or the route, never guess. New journeys,
routes and proposals take ids you choose (`j_`, a route slug, `pr_`). A new identity's key is
`who_` and 16 random lowercase letters and digits, never derived from a name.

## Status: say `display_state`

`state` is stored and picks the transition; `display_state` is the status to tell the person.
A node's `moves` lists what a write would be accepted for now. A node resting on an
undecided decision is `conditional` (see `pending_on`), not ruled out.

| `display_state` | Say |
|---|---|
| `ready` | ready (a decision: to decide; a group: not started) |
| `active` | active |
| `blocked` | blocked |
| `conditional` | conditional |
| `scheduled` | scheduled |
| `snoozed` | snoozed |
| `done` | done (a decision: decided) |
| `skipped` | skipped |
| `not_relevant` | not relevant |

## Write directly or propose

Write directly (`update_journey_state`) when the person asked for a state change: answers,
transitions, who does a node, dates, snoozes. Structure goes through `apply_patch` only when the
person asked for that exact change. Propose (`create_proposal`) when the person asks for one,
when the change is structural, or when it is large or ambiguous: Cairn has no undo.
