# Work done early on purpose is not a shortfall

Question: finished work ahead of a stage opening (gating with a reason, or not gating) or ahead of a dependency read short on the work, the milestone and the stage, and so did starting early, so a person who did something early on purpose was told the plan could no longer be met. Which constraints hold finished and started work, and how is "done early" read? (From the early-work unit.)

## The call

Three statements, all in the execution layer of the date network; the plan layer, the plan-consistency check, blocking and the guards are unchanged.

1. **Started work is held at its finish.** Starting is never guarded, so for work with a recorded start (stored `active`, or `done` with a start), every constraint that would end at its start (its own and inherited dependencies, condition gates, a container's entries and containment links, its own `not_before`) ends at its finish. A start recorded while the work is blocked is deliberate (the app offers it and labels it "started early"), so starting early is never a shortfall, and a prerequisite of started work is capped by that work's due, not by the day it started. This is the coordinator's call; the research had left it out because dates alone cannot tell such a start from an ordinary one made before a prerequisite was reopened.
2. **A stage opening that does not gate dates only work that has not started.** The opening holds each unstarted node of its stage that is not a container directly, and no started or finished one. A container waits for its contents, so it is held exactly while some unstarted content is, and a stage whose contents all finished early no longer carries the opening on to what depends on the stage (on main, a dependent of such a stage finished normally pulled the opening short too).
3. **A finished node is not held by what it was finished ahead of on purpose:** every requirement its stored guard bypass recorded as an open dependency (by every route it reaches the node, so an inherited one included), its own `not_before` when the bypass recorded a failure to wait for its own date, and a done milestone's own `not_before` (marking a milestone done is never guarded by its date). Those constraints are exempt: kept in the network, disabled, so the node's own date and `done_early` read them and no bound or shortfall does. Where an exempt requirement reaches the node through its container's entry, that hop is replaced by one constraint per requirement it carries, each keeping its source (inherited from the group, or a stage opening) and whether its path is conditional, so the requirements that were not accepted still hold the node.

`done_early` is derived for a finished node when an exempt constraint into it would, on today's dates, have put its finish later than the recorded one: the largest gap, with the date and the constraint's source. A requirement counts by its earliest date; the date shown, and the days counted to it, are a milestone's date as shown (actual, pin or due) when that is later, so the days match what the person sees on the milestone (a milestone that could happen in 2 days but is planned in 9 reads 9 days, not 2). It sits beside any other shortfall the node has and is a reading of today's dates, not a stored label. Everything else keeps its constraints, so a dependent finished normally whose prerequisite is reopened and finished after it stays a shortfall (shown on the prerequisite, as before), and a bypass for one dependency never exempts another.

This amends point (2) of `2026-10-06-how-the-execution-layer-reads-facts-slack-and-overdue.md`: a fact earlier than the chain before it requires is still seen at that chain's upstream end, except where the node had started, or finished ahead of it on purpose. It supersedes the line in `2026-10-22-a-date-ahead-holds-a-node-off-the-acting-frontier.md` that "the shortfall already records that the work finished before its date": `done_early` says it now.

## Alternatives

- Judging an early start by dates alone, as not deliberate: rejected, the app offers the start and labels it.
- Expanding every hop for date-only openings too, as for bypassed requirements: rejected, up to 2.6 million constraints for a common case (a stage that does not gate), against 32,000 for one constraint per node.
- Dropping exempt constraints from the network: rejected, the node's own date (scheduled, stale) and `done_early` read them.
- A "done early" chip or filter: rejected, one concept in one form: done is done, and the sentence says how.
- Showing days ahead of an unfinished prerequisite: rejected, the number moves with today; the sentence says only that it is not done yet.
- Counting every date as the earliest the source could happen: rejected for milestones, which the person reads at their shown date.

## Cost

The cost test (`cost_dates.rs`) at the node limit, main against this change (constraints, execution operations, network memory):

| graph | main | this change |
|---|---|---|
| plan limits (every node dated, nothing finished early) | 310,202; 1,603,747 | 310,202; 1,603,374; 13.8 MB |
| the same with early work (every other stage opening not gating, all finished work under a bypass of what it inherits) | 310,202; 1,313,133 | 426,091; 1,744,935; 18.9 MB |
| the worst case (15 nested stages each requiring 64 things, 1,921 finished actions under a bypass of everything inherited) | not measured, one constraint per hop | 1,728,024; 3,455,172; 76.2 MB |

Operations stay within the per-constraint budget (about 4 per constraint in the limit graphs, 2 in the worst case, which has no cycle). A constraint is 36 bytes with its two adjacency slots 44, so the documented bound of 2,592,000 constraints is 114 MB and the test holds the network to 120 MB. No limit changed. The worst case runs in about a second.

## Known limits

- A start recorded before a prerequisite that was not yet reopened is not told apart from a deliberate one: a prerequisite reopened later and finished after the start is judged against the dependent's finish, not its start.
- A node finished while undecided, whose condition decision is answered later, still shows that decision short.
- Work finished ahead of an `auto_done` milestone it waits on records no bypass, because finishing it fixes the milestone's latest date at the finish and the milestone then reads as reached by its date: the milestone still reads short, as it did before.
- A bypass exempts only what it recorded, so a dependency added after completion holds the node.
- A finished node's `done_early` reads today's dates: an unfinished prerequisite's earliest finish moves with today, and the sentence gives no day count for it.

## What would change it

Wanting early finishes inside a gating stage to need no reason; or the theoretical worst case of the expansion (1.7 million constraints, 76 MB at the node limit) showing up in real journeys, which would move the exempt requirements out of the network into a read on demand.
