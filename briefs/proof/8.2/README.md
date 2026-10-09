# Proof for brief 8.2: Priority explanations and informational consequences

Priority now explains itself where a person or an agent reads it. A container reports the
gravity of its whole area. A node's detail lists what finishing it would free and
what it would not yet free, with what else each waits on. Every write reports what it
unlocked and what it moved in or out of scope, apart from its warnings. The gravity, leverage
and rank formulas are unchanged.

## A container's subtree gravity

A container reports the gravity of its whole area: itself, its open descendants and everything
downstream of any of them, each node once. A stage holds two actions, each of weight 1, and a
weight-5 action waits on both.

| State | Each action's gravity | The stage's subtree gravity |
|---|---|---|
| Nothing done | 6 | 7 (the shared action counts once, not twice) |
| One action done | | 6 (the other action and what waits on it) |
| Both done | | 0 |

## What finishing a node frees, and what it does not

A small journey: a gate milestone, a weight-10 action behind it, an action that also waits
on a decision, and an action that also waits on another action. Node detail now lists:

| Gate's list | Node | Detail |
|---|---|---|
| Unblocks (leverage 10) | `n_freed` | weighted 10 |
| Still waiting | `n_cond` | also waits on `n_flag` (condition) |
| Still waiting | `n_both` | also waits on `n_other` |

Answering the decision moves `n_cond` into the unblocks and leaves `n_both` waiting. Vendor
evaluation, node `n_plan_draft`: still waiting is `n_plan_review`, which also waits on
`n_access` (through its parent) and on `n_kickoff` (the setup stage's opening).

## What a write reports besides its warnings

| Write | Unlocked | Out of scope | Into scope |
|---|---|---|---|
| Hiring loop: offer decision answered | `n_offer` | `n_close_out` | none |
| Hiring loop: offer decision reopened | `n_make_offer` | none | `n_close_out` |
| Vendor evaluation: partner testing answered "no" | none | `n_criteria`, `n_partner_led`, `n_partner_results` | none |

All three halves are computed from the two derivations at the same revision, so another
person's concurrent change is never credited to the write. Over HTTP, MCP and the browser
host the same lists arrive in the consequences of the patch answer, and none of them counts as
a warning (`Consequences::has_warnings`).

Values are asserted by the `explanations::` and `consequences::` engine tests and the HTTP, MCP
and browser-host tests beside them.

## Known limits

- A node the write adds is never reported as unlocked or in scope: the patch names it already.
- `max_child_gravity` stays on the wire, marked deprecated, until no client reads it.
