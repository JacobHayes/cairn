# Compact date chains

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
214-node journey (a chain of 200 nodes). Dates are the part of an answer this change touches;
the rest of the answer is unchanged.

| Journey (nodes read) | Tokens before | Tokens after | Change |
|---|---:|---:|---:|
| Two-week bake-off (8) | 3,448 | 3,173 | -8% |
| Hiring (13) | 7,320 | 6,347 | -13% |
| Product launch (15) | 12,761 | 9,894 | -22% |
| Vendor evaluation (24) | 13,942 | 12,352 | -11% |
| Large journey (3) | 15,132 | 9,438 | -38% |
| Longest answer, large journey (`n_s0_a0`) | 8,091 | 4,561 | -44% |

## Cut and continued

A chain longer than 50 constraints answers its first 50 and a `total`; `get_node` with
`explanations` and the field `earliest_start`, `latest_start`, `due` or `shortfall` pages
the rest in chain order. The pages put together are the whole chain. The same holds for
`GET /api/journeys/{id}/nodes/{key}` and its explanations endpoint. A rejection's chains are
not cut: no later read can continue them, and the existing limits on contradictory chains
and nodes bound them.

Known limits:
- `latest_start` still repeats the due chain it is built from, and `dates.links` still
  repeats the dates for the canvas; both are left as they are.
- A chain's fixed dates are cut to 50 as well, but a chain holds a handful.
