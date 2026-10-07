# Proof for brief 2.6: Projections

Every read view the PRD names (the canvas at any zoom, the trace, the decision view, the
timeline, the status summary, the list and next list, "mine", the agent snapshot, history, and
message drafts) is now a pure engine function over a derived journey, so the browser and the
server show the same thing.

## The vendor evaluation after kickoff

Zoomed out to groups only, every other node rolls into its group and carries its badges; two
edges into the final review collapse into one:

| Group | Rolled up | State | Badges |
|---|---|---|---|
| `n_setup` | access, plan and its two actions, workload | not started | needs breakdown; max child gravity 13.5; min slack 23 days |
| `n_testing` | baseline, comparison set | not started | all blocked; max child gravity 9.5; min slack 28 days |
| `n_reporting` | findings, findings reviewer, review opens | waiting | all blocked; max child gravity 7; min slack 31 days |
| `n_partner_led` | criteria, partner results | not relevant | max child gravity 0 |

Other views of the same journey:

- Next: `n_access`, `n_decision_meeting`, `n_workload`; within Setup, `n_access` and
  `n_workload`, with Setup as the first item's breadcrumb.
- Trace of the test plan: upstream are kickoff, access, and the plan's two actions;
  downstream are Setup itself, then testing and reporting and everything in them.
- Status summary: 16 nodes remaining, the review opening on 2026-11-06 and the decision
  meeting on 2026-11-20 coming up, and two decisions open.
- Agent snapshot: the acting frontier in rank order, the open decisions, `n_workload` needing
  breakdown, and 13 of 22 listed nodes blocked.
- The access request draft reads "Hello, [missing: roles.eval_owner.name] needs access..."
  before the up-front decisions and "Hello, Evaluation Lead needs access..." after them.
- With groups and milestones hidden before kickoff, Setup's contents move to the top level
  and each node waiting on the hidden kickoff is marked as having hidden prerequisites.

Values from the `projection_walkthrough` example over the fixtures.

## Known limits

- At the node limit every projection stays within its bound; in a debug build a canvas level
  takes 80 to 200 ms, the snapshot about 65 ms, and every other view under 40 ms.
