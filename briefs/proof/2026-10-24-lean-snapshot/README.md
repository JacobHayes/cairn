# Lean snapshot and list rows

An agent's first read of a journey, `get_snapshot`, now costs about half as much on a
small journey and 57 percent less on a full page of 200 nodes, and `list_frontier` rows are
lean in the same way. `GET /api/journeys/{id}/snapshot` answers the same shape.

## Snapshot size, before and after

Real answers from `cairn demo`, over MCP, as compact JSON; tokens counted with tiktoken
`o200k_base` (a stated proxy). The large journey is a generated 214-node journey whose
first page is 200 nodes.

| Journey | Nodes listed | Tokens before | Tokens after | Bytes before | Bytes after |
|---|---:|---:|---:|---:|---:|
| bake-off | 8 | 1,818 | 736 (-60%) | 5,334 | 2,481 |
| hiring loop | 13 | 2,283 | 1,075 (-53%) | 6,526 | 3,309 |
| product launch | 15 | 3,237 | 1,415 (-56%) | 9,025 | 4,311 |
| vendor evaluation | 24 | 4,177 | 2,046 (-51%) | 11,806 | 6,092 |
| large journey | 200 | 45,546 | 19,555 (-57%) | 127,079 | 58,305 |

Other reads: `list_frontier` with the `blocked` filter on the large journey, a full page,
31,194 to 12,944 tokens; the vendor evaluation's frontier, 206 to 105.

## What a row carries now

- Its key, title, kind, stored and display state, parent (the frontier's rows carry the
  whole breadcrumb), owners, due, slack, shortfall, snooze, checklist count, the flags that
  hold, and its rank as one number to three figures.
- A container's blockers leave out its own children.
- A finished, skipped or ruled-out node keeps its row, dates, flags and containment, but
  no slack and no rank.
- `get_node` still explains priority in full.
- The snapshot's frontier lists each item's key, title, kind, display state, breadcrumb and
  rank; the full row is in the node list or `list_frontier`.

## Known limits

- The page size stays 200: a full page of a large journey is still about 19,500 tokens. A
  smaller page awaits the owner.
- Tokens are a proxy (tiktoken), not Anthropic's tokenizer.
