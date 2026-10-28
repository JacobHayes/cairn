# Going ahead past a guard is a transition, not an override

Question (from simplifying "go ahead anyway"): a completion that fails its guards used to need
a guard-bypass override listing the exact guards, beside the completion in the same patch.
Models asked to "complete it anyway" reached for the group-only mark done instead.

Call:

- A new transition, `complete_anyway` with an optional `reason`, replaces the pair. It bypasses
  whichever guards fail and the engine records which, on the node's stored bypass, as before:
  the specific failures, each of which names its guard. The stored bypass no longer keeps a
  separate list of guards, which only repeated the failures. The guard-bypass override, its guard
  list, its removal and its own event type are gone.
- The reason stays optional, as the requirements say of a bypass; an absent reason is `{}`.
- It is a separate transition rather than a field on `complete`, so the plain `"complete"`
  keeps its wire form and an impossible pairing (a reason on a skip) cannot be written.
- Only `complete_anyway` is added, not a start variant: starting never has a guard (order is
  not one), so there is nothing to bypass. On a milestone or any node without guards it is
  refused as an illegal transition.
- The refusal of a failed guard names `complete_anyway` as the way through, and the violation
  no longer carries a separate `bypassable` guard: its `failures` already say which guards.

Stored data: migration 23 rewrites every stored occurrence, so the runtime knows only the new
shape. In pending proposals, a guard-bypass override and the completion of the same node become
that completion going ahead anyway with the bypass's reason; a bypass with no completion beside
it covered nothing and is dropped, as is the removal of a bypass. Events are append-only in
meaning, and their encoding may change in a format migration: an event that applied a bypass
(`guard_bypassed`) becomes `override_applied`, its delta still holding the bypass record, and a
proposal's mutations inside event deltas are rewritten like the proposals themselves. The
stored bypass (guards, reason, failures) is unchanged. Route files and exports hold graphs
without journey state or mutations, so no import path carries the old shape; a patch or
proposal sent with it is refused as an unknown value.

Alternatives: an `anyway` field on every transition (rejects nonsense pairings at runtime
only, and changes the wire form of `complete`); keeping the guard list as an assertion of what
the caller expects (the duplicate that made models fail).

What would change it: a start that gains a guard would take the same shape.
