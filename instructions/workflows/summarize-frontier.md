---
description: Summarize where a journey stands and what to do next, from its snapshot and ranked frontier.
---

# Summarize the frontier

1. **Read the snapshot.** `get_snapshot` on the journey. Its counts say how much is in
   scope and done; `acting_frontier` is what can be acted on now, in rank order.
2. **Lead with what is next.** Name the top few frontier items with their owners and why
   they rank high (a deadline, how much waits on them). `get_node` explains one item's
   rank, blocking, and dates when the person asks why.
3. **Then what needs attention.** Open decisions that block work; placeholders that need
   breakdown; unassigned items; shortfalls (plans that can no longer be met); overdue and
   stale items (`list_frontier` with the matching `filters`).
4. **When nothing can move.** If the snapshot is `stalled`, say what it waits on: a date, a
   snooze, or a blocker outside the journey.
5. **For one person.** `list_frontier` with `mine` (and `for_viewer` to rank for them)
   gives their own next steps.

Keep it short: what is next, what is stuck, and what needs a person's call.
