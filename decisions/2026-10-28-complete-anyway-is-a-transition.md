# Going ahead past a guard is a transition, not an override

Question: how does a person or agent complete work whose evidence guards fail?

Call: the transition `complete_anyway`, with an optional reason. It bypasses whichever guards fail, and the engine records the specific failures on the node's stored bypass. It is a transition of its own rather than a field on `complete`, so `complete` keeps its wire form and a reason cannot be paired with a move it means nothing on. There is no start variant: starting has no guard to bypass. On a node with no guards it is refused as an illegal transition. A failed guard's refusal names `complete_anyway` as the way through.

Alternatives: an `anyway` field on every transition (nonsense pairings refused only at runtime); a separate guard-bypass override listing the guards beside the completion (models get the pairing wrong and reach for a group's mark done instead).

What would change it: a start that gains a guard would take the same shape.
