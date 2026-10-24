---
description: Walk a person through a journey's open decisions in rank order, one at a time, recording each answer.
---

# Walk the open decisions

Offer this whenever a journey has open decisions: right after it is created, and when you
return to it later.

Open decisions gate the work behind them: until one is answered, what depends on it is
undecided. Walking them in rank order unblocks the most first.

1. **List them.** `get_snapshot` gives `open_decisions` in rank order; `list_frontier` with
   `filters` `["decisions_needed"]` gives the actionable ones with their breadcrumbs.
2. **Take one at a time.** For each, `get_node` shows its prompt, its answer type, its
   choices, who owns it, and what its answer affects (the nodes whose relevance it
   decides, a role it fills, a milestone it pins). Its `answer_effects` say, per choice, how
   many nodes the answer brings in, drops (and how many of those have progress), and leaves
   to be decided later, so you can say "answering yes brings in 6 and opens 2 more
   decisions". Ask the person the question in plain words, offering the choices. If the
   person explains the choice, keep it as the answer's `rationale`.
3. **Record what they have said** with `answer_decision` or `apply_patch` (see
   record-answers). Answers the person has already given go in one write. Before asking
   the next question, read again whenever an answer changes which questions apply (it
   made work relevant or not, filled a role, pinned a date, or opened a decision that was
   undecided before): the write's consequences say so, and `get_snapshot` has the
   decisions now open.
4. **Move on.** Take the next decision in rank order. Stop when the person wants to, and
   summarize what was decided and what is now next.

Do not answer for the person. When they are unsure, leave the decision open and say what
waits on it.
