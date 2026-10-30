# Checklist ticks are records of their own

Question: where does a checklist tick live, what happens to it when its item goes, and what does repeating a tick do?

Call: a tick is a journey record of its own, one per ticked item, not a set on the node's state. It leaves in the same write as its item (a field edit, a whole-node replace, removing the node), and so does any bypass failure that names the item, so a later item that reuses the id never inherits an old tick or bypass. Setting an item to the state it already has is accepted and writes nothing; the history leaves the empty event out. An item not on the checklist is refused. The ticked-checklist guard fails once per unticked item, so an item added or unticked later makes a finished node stale.

Alternatives: refusing a repeated tick (noisy for agents that resend what they believe is current); keeping a tick across removal and re-adding of the same id (it would count for work nobody checked).

What would change it: per-item owners or dates, which would make items records with identity of their own.
