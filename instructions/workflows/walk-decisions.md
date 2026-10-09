---
description: Walk a person through a journey's open decisions in rank order, one at a time, recording each answer.
---

# Walk the open decisions

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
3. **Record the answer** with `answer_decision` (see record-answers), then read what it
   changed: the answer may make work relevant or not, fill a role, pin a date, or open a
   new decision that was undecided before.
4. **Move on.** Use the revision from the receipt for the next answer, and take the next
   decision in rank order. Stop when the person wants to, and summarize what was decided
   and what is now next.

Do not answer for the person. When they are unsure, leave the decision open and say what
waits on it.
