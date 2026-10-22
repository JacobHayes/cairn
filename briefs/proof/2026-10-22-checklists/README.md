# Checklists on deliverables and actions

A deliverable or action can now carry a checklist of items, ticked one by one, and
can require every item ticked before it is done. Agents tick with one patch op
(`set_ticked`) and can send the ticks and the completion together. The values below
come from the product launch fixture, with its hardening action given three items and
the requirement switched on.

## The node's snapshot row

| Step | state | display state | checklist count | stale |
|---|---|---|---|---|
| Before this change (no checklist field exists) | todo | ready | none | none |
| Three items, none ticked | todo | ready | 0 of 3 | none |
| Ticks for all three and the completion, one patch | done | done | 3 of 3 | none |
| `load` unticked afterwards | done | done | 2 of 3 | unticked: load |

Reopening a node keeps its ticks. Removing an item (a field edit or an upgrade) removes
its tick in the same write, so an old tick never attaches to a new item with the same id.

## The refusal

Completing with nothing ticked is refused once per unticked item, each bypassable with a
reason (the bypass records those items, so an item added later makes the node stale):

- completing this node needs its checklist item "Rollback plan" ticked; tick it in the same patch, or bypass it with a reason to complete anyway
- completing this node needs its checklist item "Load test" ticked; tick it in the same patch, or bypass it with a reason to complete anyway
- completing this node needs its checklist item "Runbook updated" ticked; tick it in the same patch, or bypass it with a reason to complete anyway

Ticking an item not on the checklist is refused: the node's checklist has no item "nope".
Sending the state an item already has is accepted and changes nothing: no record
changes and the history shows no line.

## Also now true

- The web app, the API and the agent tools read `checklist_count` (ticked and total) on a node's row and
  detail, and the ticked items on the detail.
- At most 50 items per node (the checklist limit).
- Upgrades merge the checklist as one field; an upgrade that adds an item to a finished,
  bypassed node leaves it stale for that item.
- Existing databases migrate by adding nullable columns and a ticks table.

## Known limits

- No checklist screen yet: the web shows the guard, the stale reason and the event in
  words, and Done on a card for such a node offers only the bypass.
- A kind change on a node that holds ticks is a shape conflict in an upgrade, resolved by
  keeping the journey's node or clearing its state.
