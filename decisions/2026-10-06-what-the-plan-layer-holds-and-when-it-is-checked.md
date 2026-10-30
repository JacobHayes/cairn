# The plan layer is the pruned date network, checked on every graph a write produces

Question: the plan layer (pins, rules, estimates, containment, dependencies; no actuals, no today) needs a definition of which graph it covers and which writes are checked against it.

Call: the plan layer is the date network over the relevance-pruned graph: constraints of not-relevant nodes dropped, those through undecided nodes kept, skipped work at zero duration, seeded with the pins of in-scope nodes and each date answer in effect fixed at its own instant. The journey's creation time is a fact, like an actual, so it enters only the execution layer. Every graph a write produces is checked, once the structural checks pass; no write is classified as not touching dates. Positive cycles are searched first, and pin-against-pin conflicts only when there are none.

Alternatives: the full unpruned network (rejects contradictions in branches that do not apply); the creation time in the plan (a rule relative to it could reject a patch on a fact); checking only writes that look date-related (skips, reopens, answers, and force includes all change which constraints apply, so classifying would re-derive anyway).

What would change it: people finding a reopen of skipped work rejected because the full estimates no longer fit the pins.
