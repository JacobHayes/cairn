# Every journey view shares one view state, and hiding is one engine level

Question (from making the journey's views agree): Detail, Frontier, Decisions, the Filter and the search meant something different on the graph, the list, the timeline and the walk through. Each view had its own Filter body, kind pickers and ladder default, and some hid nodes in the tab. How should the views share these settings, and where should hiding happen?

Call: one `ViewState` (`web/app/src/journeys/view.ts`) holds Detail, Frontier, Decisions, the Filter and the search. It is parsed from the address once, and every switch of view carries all of it and none of the view's own settings. Pure functions (`journeys/requests.ts`) turn it into each view's requests.

- **Hiding is one level request.** `LevelQuery` gains the next list's `flags`, `owner` and `text`. A node is drawn only when it matches every one or holds a match. One predicate, shared with `list`, decides a match, so the list and the level cannot disagree. The level takes the viewer, for `mine`. What the Filter or search hides rolls up, re-targets its edges and keeps the hidden-prerequisites marker, as a collapse does.
- **Stages' top-level rule moves into the engine** as `LevelQuery.top`. A node whose top-level ancestor is not a group or a milestone is left out at Stages, with no stand-in. This replaces a tab-side filter that only the graph applied.
- **The level counts C1's hidden nodes** as `Level.hidden_by_class`: the nodes hidden only for their display state. The graph and the list badge the sum, the timeline only conditional, and the walk through nothing.
- **One Filter body**: Only mine, Owner, overdue, stale, unassigned, short of days, snoozed, then Show conditional and Show not relevant. Kinds belong to Detail. `flag=mine`, `in=`, `state=`, `kind=` and flags the Filter does not offer are no longer read. `show=` lists the shown classes, so an old `show=notrelevant` now also hides conditional nodes. The trigger is pressed whenever the Filter is off stock.
- **One default step, All**, on every view; the timeline no longer opens at Stages.
- **The list draws the level.** Its table reads unfiltered rows and keeps those the level draws, so a container drawn for a match beneath it keeps its row when sorted.
- **Frontier and Decisions are highlights, and neither changes the level.** They form one lit set on the graph and the timeline, and everything unlit fades to 0.3. On the list and the walk through, Decisions also narrows the rows or cards to the decisions; the level is the same with it on. On the list, frontier rows carry their reason and action under the title. The lowest opacity wins between a highlight and a trace.
- **What a view cannot honour stays set, disabled, with its reason.** This covers the walk through's Frontier and display-state switches, and the timeline's Show not relevant.
- **Reset the view** returns to All, the stock Filter, no search and Decisions off. Frontier stays, since it hides nothing.
- **The summary counts the whole journey.** It carries the state and says so.

Alternatives:

- Filter the level by a key set from a list read: a two-phase read with revision skew, up to 2,000 keys in a query, and a filter in the tab (ARCHITECTURE, Canvas).
- Filter in TypeScript from the derived journey: it would duplicate the engine's text search.
- Keep each view's own settings: the same chip would keep meaning different things.

What would change it: a view that needs a setting the others cannot carry, or Decisions being reworked into something other than a highlight.
