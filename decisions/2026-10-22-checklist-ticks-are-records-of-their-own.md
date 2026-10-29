# Checklist ticks are records of their own

Question (from the work that added checklists to deliverables and actions): where does a
tick live, what happens to it when its item goes, and what does ticking an item that is
already ticked do?

## Call

- A tick is a journey record of its own, one per ticked item (`Tick` with node and item), not
  a set on the node's state.
- A tick is removed exactly when its item leaves the node's checklist, in the same write:
  a field edit, a whole-node replace, an upgrade merge, and removing the node. An old tick
  therefore never attaches to a later item with the same id, and a recorded invariant
  rejects a state holding a tick for an item the node does not list.
- `set_ticked` sets the item's state and is symmetric. Sending the state the item already has
  (ticking a ticked item, unticking an unticked one) is accepted and writes nothing: no
  record changes and the history shows no line. The engine keeps one event per mutation, so
  the event exists with an empty delta, and the history projection leaves such events out.
  An item that is not on the checklist is refused.
- The bypass failures for an item leave with the item as well (same writes as the tick), so
  a bypass never covers a later item that takes the removed item's id.
- The guard `all_ticked` fails once per unticked item, and a bypass records those items, so
  an item added or unticked later makes a finished node stale.
- A kind change on a node that holds ticks is a shape conflict in an upgrade (the ticks
  would have nothing to attach to); the review offers keeping the journey's node or
  clearing its state.
- Limit: 50 items per node, signed off by the owner.

## Alternatives

- Refusing a repeated tick: noisy for agents that resend what they believe is current.
- Dropping the empty event: the one-event-per-mutation rule is an engine invariant other
  code relies on, so filtering the history is the smaller change.
- Keeping a tick when an item is removed and re-added with the same id: a stale tick could
  count for work nobody checked.

## What would change it

Per-item owners or dates would need items to be records with identity of their own (and
ticks could move onto them); a need to show a no-op tick in the history would end the filter.
