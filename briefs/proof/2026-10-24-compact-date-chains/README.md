# Compact date chains and a lean node read

A node's date chains (why a start or due date is what it is, and why a plan is short) are
written in about two thirds of the space, keep each constraint's source and whether it is
conditional, and are cut to 50 entries with a total like every other explanation list. The
app reads the same chains and shows the same words.

A constraint was a nest of objects; it is now four short fields:

```json
{"after":"n_report.start","before":"n_findings.finish","days":0,
 "source":"dependency:n_report:n_findings:inherited:n_stage","conditional":true}
```

A source is its kind and the nodes it names (`due_by:n_a`, `estimate:n_a`,
`containment:n_parent:n_child`, `dependency:n_a:n_b:` then `explicit`, `inherited:n_up`,
`condition:n_decision` or `opening:n_stage`); an instant is `n_x.start`, `n_x.finish`,
`n_x.answer` or `created`.

## `get_node` size, before and after

Real answers from `cairn demo` over MCP, as compact JSON; tokens by tiktoken `o200k_base`
(a stated proxy). Every node of each fixture journey, plus three nodes of the generated
214-node journey (a chain of 200 nodes). "Chains" is the compact chain alone; "all" adds the
rest of the lean `get_node`: the latest start's chain left out when it is the due chain
(led by the node's own estimate), no `path`, no `parent` beside the node's own, no date
links, `relevance` only when the node is not relevant, empty lists left out, and scores and
the rank to three significant figures.

| Journey (nodes read) | Before | Chains | All | Change |
|---|---:|---:|---:|---:|
| Two-week bake-off (8) | 3,448 | 3,173 | 2,571 | -25% |
| Hiring (13) | 7,320 | 6,347 | 4,574 | -38% |
| Product launch (15) | 12,761 | 9,894 | 6,959 | -45% |
| Vendor evaluation (24) | 13,942 | 12,352 | 9,321 | -33% |
| Large journey (3) | 15,132 | 9,438 | 8,137 | -46% |
| Longest answer, large journey (`n_s0_a0`) | 8,091 | 4,561 | 4,503 | -44% |

## Cut and continued

A chain longer than 50 constraints answers its first 50 and a `total`; `get_node` with
`explanations` and the field `earliest_start`, `latest_start`, `due` or `shortfall` pages
the rest in chain order. The pages put together are the whole chain. The same holds for
`GET /api/journeys/{id}/nodes/{key}` and its explanations endpoint. A rejection's chains are
not cut: no later read can continue them, and the existing limits on contradictory chains
and nodes bound them.

Known limits:
- A latest start whose chain was cut to the limit keeps its chain even when it is the due
  chain: what the cut hides is not known to agree, so long chains (the large journey's)
  save little beyond the compaction.
- The HTTP node detail keeps `path`, `parent`, the date links and the latest start's chain:
  the app and other clients render from them.
- A chain's fixed dates are cut to 50 as well, but a chain holds a handful.
