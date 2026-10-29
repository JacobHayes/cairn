# A route's Stages list runs in schedule order

Question: what does the Stages list show for a route, and in what order, from the work that
brings the journey card's Stages list to the route's card?

Call: the same list component, with a row per top-level group showing only what a route defines:
the title and the days the stage spans (the timeline's span of what it holds and its words), in
order of the earliest day each starts and then finishes (as the timeline orders its rows), stages with no day after, then as the route lists them. No
state chip, meter or due date, since a route has none; the days and their span come from the one
place the timeline reads them, so the rule is not copied.

Alternatives: the rule-foot text the canvas card carries (it names a rule, not when the stage runs,
and many stages have none); counting the items inside as progress (a count that says nothing about
the route's order).

What would change it: a stage whose span says too little to choose it by, or a route-side value the
engine derives per stage that the list should show instead.
