---
description: Handle a proposal whose destination moved since it was drafted, by refreshing it and asking for a new review.
---

# Handle a stale proposal

A proposal records the destination revision it was drafted against. When the destination
moves (someone else wrote to it), applying is refused as `stale`, and `get_proposal` with
`review` reports `stale` with what changed.

1. **See what moved.** The review's `stale` names the revisions and what the intervening
   writes touched. `get_history` on the journey shows who changed what.
2. **Refresh it.** `edit_proposal` with `change` `"refresh"` and the proposal's editing
   revision drafts it again against the destination as it stands, keeping the reviewer's
   choices. An upgrade, save, or re-link is drafted again by Cairn; any other proposal is
   re-based as it stands.
3. **Check it again.** `get_proposal` with `review`: the refreshed proposal may now have
   violations, or items to choose again. Fix them with `edit_proposal` and `replace`.
4. **Ask for a new review.** A refreshed proposal has a new editing revision and must be
   reviewed again before it is applied; tell the person what changed.

When the change no longer makes sense against the new state, discard it (`edit_proposal`
with `"discard"`) and say why.
