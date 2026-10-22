# Reading the removal of a node

An agent can now ask for exactly what removing a node takes, instead of assembling the
removal by hand: `get_node` with `removal` true returns the removal object for that node
as the journey stands, plus the places elsewhere that still mention what it removes.

## Observed on a small journey

A group `n_phase` holds a decision `n_ask`, carries one note, and has an action
`n_after` that requires it. A separate action `n_extra` is relevant only when `n_ask` is
answered yes.

| Read | Value |
|---|---|
| removal.node | `n_phase` |
| removal.descendants | `n_ask` |
| removal.edges | `n_after` requires `n_phase` |
| removal.annotations | the note |
| outside_references | `n_extra`, field `relevant_when`, naming `n_ask` |

Sending the removal alone is refused: `n_extra`'s condition would name a node that is
gone. Sending the same removal together with a `set_node_field` clearing `n_extra`'s
`relevant_when`, as the outside reference says, applies in one patch; `n_after` and
`n_extra` remain, `n_phase`, `n_ask` and the note are gone.

## Known limits

- The read is a snapshot: if the revision moves before the patch, the patch is refused as
  stale and the read is repeated.
- A role filled by a removed decision stays defined and becomes unfilled; the read does
  not list it, since nothing refuses on it.
