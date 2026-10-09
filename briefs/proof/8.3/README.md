# Proof for brief 8.3: Level collapse and answer effects

A level can now roll whole stages into their cards and hide nodes by how their relevance
reads, with edges and prerequisite markers following; and a decision tells each of its
choices' effects before anyone answers. Values below are from the vendor evaluation (25
nodes) and a hand-built chain, read through the engine; the API (`GET .../level`, `getNode`),
MCP (`get_level`, `get_node`) and the browser's worker answer the same bytes.

## Level: collapse and relevance classes

| Request | Nodes | Edges | What changed |
|---|---|---|---|
| every kind, nothing collapsed | 25 | 11 | the whole graph |
| collapse Setup and Testing | 15 | 7 | each card rolls up 5 nodes; the plan's edge into Testing and the kickoff's into Setup land on the cards; the partner decision's edge re-targets to Testing |
| after "partner runs: no", hide not relevant | 22 | 10 | the partner-led group and its two actions roll up into Testing; no line is left pointing at a hidden node |

A node that is not relevant only because the decision it reads cannot be answered yet is
`conditional`, not `not_relevant`: leaving "conditional" out hides it with the undecided
nodes, and leaving "not relevant" out does not.

## Answer effects

On the vendor evaluation before the partner decision is answered, "Does a partner team run
the testing?" reports a **yes** that brings in 3 (the partner-led group and its two
actions), and a **no** that drops the same 3. On a chain where answering the first decision
yes opens two more decisions and an action that hangs on the second:

| Choice | Brings in | Opens | Drops | Decided later |
|---|---|---|---|---|
| no | 0 | 0 | 4 | 0 |
| yes | 3 | 2 decisions | 0 | 1 (the action behind the second decision) |

Revising an answer counts progress: dropping a started action says so, and the recorded
choice reports no effect. A date decision names the milestone it pins and an entity decision
the role it fills.

## Budgets (browser worker, median of nine, including the message round trip)

| Generated journey | Level with collapse and a class left out | Trace |
|---|---|---|
| 500 nodes | 5.3 ms (budget 16) | 1.6 ms (budget 8) |
| 2,000 nodes | 23.4 ms (budget 60) | 0.5 ms (budget 30) |

## Known limits

- The canvas toggles pass the new request fields; the redesigned canvas (collapse controls, the
  "show not relevant" chip) is another brief's. A node of a shown kind that rolls up is not a
  checklist item.
- `still_waiting` on node detail and the decisions-needed tool's effects flag belong to the
  explanations work and are not built here.
