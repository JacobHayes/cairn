# Proof: what agents are told agrees with itself

An agent reading Cairn now gets one answer to each question: how much is blocked, what a
group's status is called, whether a proposal needs a refresh, why a draft failed, and which
endpoints refuse it. Screens no longer show requirement codes, and every browser test now
fails on any key, requirement id or document name shown on a screen.

## Blocked, counted one way

The vendor evaluation fixture, walked through its scenario. Before, a snapshot's
`counts.blocked` counted the engine's internal flag, which is true for any container whose
children are unfinished; `by_display_state.blocked` and the blocked list filter counted the
display state. Now only the display state is served.

| Scenario step | Old `counts.blocked` | Display state blocked (served now) | Where they differed |
|---|---|---|---|
| 1 | 18 | 13 | Testing (ready), and four nodes waiting on a decision (conditional) |
| 3 | 13 | 10 | Setup and Testing (ready), Baseline (conditional) |
| 5 | 13 | 8 | Setup (active), Workload, Plan and Testing (ready), Baseline |
| 8 | 3 | 2 | Reporting (active) |

A container's "all blocked" badge follows the same rule, so a stage whose only open child is
a deliverable waiting on its own pieces no longer reads as all blocked.

## The rest, observed

- The deprecated `group_state` is gone from the level; `display_state` is every node's
  status.
- The agent guide says a group that has not started is "not started", as its chip does; the
  guide's words are checked against the ones the screens use.
- A proposal listing carries `stale`, worked out once in the store: the web row's "Needs a
  refresh" and the agent's `list_proposals` agree, including when the destination was removed.
- Asking for an upgrade of a journey that does not exist answers "not found" on every host;
  one that follows no route is refused as the caller's to change, never a host failure.
- An agent may list its user's tokens: the API document no longer says it is refused there.
  Minting, revoking and the assistant refuse an agent from one flag the document also reads.
- The list filters an agent can use are the API's own, each described by what it matches.

## Known limits

- None known: a key the page sets in capitals is caught too, and the deprecated child-gravity
  field is gone, since nothing read it.
