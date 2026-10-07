# Proof for brief 2.5: Gravity, leverage, rank

A journey's frontier now comes in rank order, and every rank explains itself: how much work
waits downstream of a node (gravity), how much completing it would unblock (leverage), and how
close it is to a deadline. A viewer can re-rank the next list for themselves without changing
the shared order.

## The vendor evaluation's ranked frontier

Read on 2026-10-06 under the default rank constants. Kickoff leads while it gates Setup; once
it is reached, environment access takes over.

| After | Node | Gravity | Leverage | Rank |
|---|---|---|---|---|
| creation | `n_kickoff` | 15.5 | 2 | 0.4500 |
| creation | `n_partner_runs` | 10 | 0.5 | 0.2113 |
| creation | `n_decision_meeting` | 10 | 0 | 0.1613 |
| kickoff reached | `n_access` | 13.5 | 1 | 0.4500 |
| kickoff reached | `n_decision_meeting` | 10 | 0 | 0.1852 |
| kickoff reached | `n_workload` | 1 | 0 | 0.0185 |

## Prioritize for me

The product launch with its feature work given to a writer. Kickoff frees someone else's work
globally but the writer's own work for the writer, so the beta start moves up for them:

| Node | Global rank | For the launch lead | For the writer |
|---|---|---|---|
| `n_kickoff` | 0.6464 | 0.6464 | 0.5464 |
| `n_launch` | 0.2500 | 0.2500 | 0.2500 |
| `n_beta_start` | 0.1750 | 0.1750 | 0.2750 |

The shared ranking is unchanged after both per-viewer rankings.

## Effort-adjusted ordering

After access is done and the workload broken down, ordering by gravity per estimated day puts
the unestimated decision meeting last: `n_plan_draft`, `n_workload_ingest`,
`n_workload_query`, `n_decision_meeting` (by rank, the meeting is second).

Values from the `rank_walkthrough` example over the fixtures.

## Known limits

- At the node limit (2,000 nodes, every weight at its maximum) ranking takes 1,749,097
  operations as stored and 1,790,826 with every node open, against a budget of 1,843,747; the
  largest gravity is 2,000,000 weights.
