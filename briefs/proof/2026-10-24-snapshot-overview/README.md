# Proof: the snapshot is an overview, nodes come from `list_nodes`

An agent's first read of a journey is now a small overview of where it stands. It no longer
carries a node list; `list_nodes` (the renamed `list_frontier`) lists nodes when the agent wants
them: every node, or those a filter holds for, scoped to a subtree with `within` and `depth`,
200 to a page. `GET /snapshot` answers the same overview, and `GET /nodes` takes the same `depth`.

## What the first read costs

Tokens (o200k_base, compact JSON) of the first read, from the demo's journeys and a generated
journey of 214 nodes (ten chained stages of twenty actions, two decisions). Measured on the built
binaries over MCP, before and after.

| Journey | Snapshot before | Snapshot after | A full `list_nodes` walk after |
|---|---:|---:|---:|
| bakeoff | 736 | 375 | 363 |
| hiring | 1,075 | 241 | 880 |
| launch | 1,415 | 242 | 974 |
| vendor evaluation | 2,046 | 306 | 1,625 |
| large (214 nodes) | 19,555 | 448 | 14,719 (2 pages) |

Before, reading every node meant paging the snapshot (22,330 tokens on the large journey, the
overview repeated on each page). Now the overview is read once and the nodes only when needed.

## What the overview still holds

The answers in effect, counts by state and by display state, the top of the ranked acting
frontier (the rest as keys), the top ten of each list of open decisions, placeholders needing
breakdown, unassigned work and shortfalls (with titles, and their totals in the counts), and whether completing the journey is suggested.

## Known limits

- A row in `list_nodes` no longer carries blockers or other participations; `get_node` has them.
- `list_nodes` ranks for the caller only through `mine`; the frontier's per-viewer ranking option
  is gone.
- Each attention list shows only its top ten; `list_nodes` with the matching filter lists the rest.
