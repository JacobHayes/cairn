# The derived guards on the write path

Question: how do the completion guards meet relevance, structural violations, bypasses and snoozes?

Call:

- Only a node that is not relevant is refused. Undecided work completes with a warning carried in the patch's consequences, which already reach every surface: it may not apply, since a decision it reads is unanswered. It is never forced into scope; a later answer can still rule it out.
- The derived guards run only when the candidate holds every structural invariant; otherwise the structural violations are reported instead.
- Each guard failure names the patch's other mutations that caused it.
- A bypass records the failures it accepted, and reopening the node clears it, so a recorded failure never covers a later completion.
- Snooze waits are a graph invariant over the full structural graph: a snooze whose target depends on the snoozed node, is its ancestor, or waits on it through another snooze is refused on every patch, whatever the answers. A snooze on effectively skipped work is refused.

Alternatives: refusing undecided work until answered (makes deciding a precondition of doing); a separate warnings list on the patch answer (a second channel threaded through every surface); keeping bypasses until removed by hand; checking snooze cycles over the pruned graph or only on snooze mutations (an answer or a later edge could make an accepted snooze illegal unseen).

What would change it: people surprised that finished undecided work was ruled out by a later answer, which would argue for confirming before finishing it.
