# The assistant writes state changes directly, with no cap on the nodes they touch

Question (from the assistant's tool loop work): the assistant redirected a state change touching more than ten nodes, or a turn's direct writes together touching more than ten, to a proposal. Should that cap stay?

Call: remove it, along with the per-turn ledger that carried it across calls and the automatic redirect. A state change the person asks for is applied directly and reported with what it caused, as an external agent's is. The assistant's instructions say it proposes when a change is structural, or when the person asks, or when a change is large or ambiguous enough that they should see it whole before it lands.

Why: the cap existed so that a misheard bulk request is seen whole before it lands. But Cairn has no undo, so the cap only ever protected the assistant's own users, while external agents already write uncapped through the same tools; the cap also cost a model call and a review step for correct bulk requests, and the ledger was a second source of truth for what a turn had written. A model's judgment about "large or ambiguous", told in its instructions, covers the misheard case without a number.

Alternatives: keep the cap with a per-change count (a parked attempt did this); raise the number; confirm in conversation before a large write.

What would change it: a misheard bulk write reaching a person in practice, which would point at a confirmation step rather than a count.
