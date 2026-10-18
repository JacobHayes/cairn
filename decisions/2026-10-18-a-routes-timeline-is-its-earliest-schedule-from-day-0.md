# A route's timeline is its earliest schedule from day 0

Question: a route has no dates, state or today, so what does its Timeline show?

Call: the route read as a journey that starts on day 0 with nothing answered. Each in-scope
node sits at the earliest day its dependencies, estimates, date rules and stage bounds allow
(the engine's `schedule`, wasm `routeSchedule`), on a relative axis labelled "Day N" with the
start line where a journey's axis has today and the final milestone as the end anchor. A node
no bound reaches gathers in a "No day yet" list; a row whose date rule waits on an unanswered
date decision says which; and the end anchor says "at the earliest, if the branch is taken"
when its day passes through an undecided node. The axis works in day numbers, so it can never
show a made-up calendar date. The chart is the journey's, shared.

Alternatives: no timeline on a route (the views would differ by kind); dates computed from an
invented start date (a route would show calendar dates it does not have); showing every
branch's schedule (the earliest is what an author compares, and conditionals are marked).

What would change it: estimates that vary by role or owner would make the earliest schedule an
optimistic bound that needs a second, latest-case line.
