# apply_patch lists its mutations as a type sketch

An agent connecting to Cairn now reads `apply_patch`'s mutations as one line of
type signatures per operation, instead of the full JSON Schema, and still writes
every structural change correctly from them.

A line looks like this (the sketch is generated from the same schema the server
parses with; `?` may be left out, `| null` takes null as a value):

    set_node_field {node: NodeKey, value: NodeFieldValueResolved}
    type NodeFieldValueResolved = ... | {relevant_when: ConditionResolved | null} | ...

## What the tool definition costs

Measured on a running demo server's tool list, counted with the o200k_base
tokenizer on compact JSON (a stand-in for a model's own tokenizer).

| `apply_patch` definition | Before | After |
|---|---:|---:|
| tokens | 15,489 | 4,412 |
| compact JSON bytes | 63,850 | 17,156 |

The remaining weight outside the sketch is the patch envelope's own descriptions
and shared types, which the schema compaction work removes next.

## A model using only the list

A Sonnet session through Claude Code (`claude -p`), with Cairn's tools as its
only tools, was asked in words to author a segment route, then on a demo
journey to add a decision and an action with a condition, set the action's
estimate, clear its condition with `null`, add an edge, insert the segment under
a group, and remove the action. Two runs, ten tool calls each: every patch was
accepted on its first try, the `null` clear took effect (the server reported the
action unlocked), and the removal needed only the bare node because the model
read its removal check first. No refusal came from a misread shape.

## Known limits

- The `mutations` items are an open object with an `op`, so a client that
  validates arguments against the listed schema no longer catches a wrong field
  before the server does; the server's own check is unchanged and names the path.
- A provider's strict structured-output mode cannot take that open object; Cairn's
  adapters do not use it.
