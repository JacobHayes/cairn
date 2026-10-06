---
description: Resolve a contradictory date chain or a shortfall by applying one of the resolution moves Cairn lists.
---

# Resolve a date conflict

Dates are constraints: rules, dependencies, estimates, and pins. Two problems come up:

- A **contradictory chain**: a write would make a date precede itself. The write is refused
  with a violation carrying the chain and its resolution moves.
- A **shortfall**: the plan can no longer be met given today or an actual date. It is a
  warning on the node (`get_node`, its dates), with the chain and the moves that would fix
  it.

1. **Read the chain.** Each step says which constraint it comes from (a rule, a dependency,
   an estimate, a pin) and how many days it is short.
2. **Offer the moves.** Each resolution is one mutation that alone gives the chain its
   missing days: shift, repin, or unpin a pin; revise the date answer behind a pin; loosen
   a rule's offset; shrink an estimate; drop a requirement. Explain each in plain words and
   let the person choose.
3. **Apply the choice.** `resolve_date_conflict` with the chosen `resolution` exactly as
   listed and, for a refused write, the refused mutations in `then`, in one patch. Moves
   that change the structure (a rule's offset, an estimate, a requirement) are better as a
   proposal when the person wants to review them.
4. **Check.** The write's consequences show whether shortfalls remain.
