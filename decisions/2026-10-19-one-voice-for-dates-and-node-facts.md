# One voice for dates, due words, snoozes, flags and trouble counts

Question (from the systematic removal of drifted duplicates in the web app): the same fact was worded by up to nine helpers (dates, due, where a date comes from, where a snooze ends, a node's flags, a journey's trouble counts). Which wording does each fact take, and where does it live?

Call: one home. `status/when.ts` owns dates (`dateWords`, `dueWords`, `whenWords`, `relativeDays`, `originWord`, `daysUntil`, the near and soon windows), `status/snooze.ts` owns where a snooze ends, `status/words.ts` owns the trouble table, and `flagsOf` in `detail/model.ts` is the one flag list every surface renders. Every surface calls them; the per-screen copies are deleted.

- A duration past a date is "N days late"; the flag and every count of nodes is "overdue" (the PRD's word for the derived flag). A shortfall is "short of days" wherever it is a flag or a count (PRD C18); a quantity is still "N days short".
- A decision's date is "Decide by <date>" at any distance and "Decide today"; other work is "Due today", "Due tomorrow", "Due in N days" within two weeks, then "Due <date>". One window (two weeks) for the words and one (a week) for the warning ink.
- Where a date comes from is "pinned" (a pin), "recorded" (an actual) or "worked out" (derived), on the inspector, the Summary tooltip and the explanation chain. The timeline's milestone rows say "Reached", "Pinned" or "Worked out".
- A milestone's date is the nearness then the date: "in 18 days (Nov 23)", "3 days late (Oct 2)", "reached Nov 20". It follows a title of any kind, with no verb of its own ("is reached on"). A milestone that reaches itself says so in plain words ("counts as reached by itself that day", the authoring label's words), never "auto-reach".
- A snooze that waits on a node ends "<node> is done" on every surface (it was also "is finished" and "is settled").
- A route's screens have no journey and the client is not given the deployment's time zone outside a journey's document, so their today is the session's clock in the tab's own zone, through `Session.today()`; never the UTC date.

Alternatives: keep the card's seven-day window for due words (rejected: a second window is the drift); "Estimated" for a derived date (rejected: the inspector's "worked out" is already the plainer word and one word must win); expose the deployment's zone in the deployment read so routes use it (the better end state, but it adds an engine field; record it if routes' evening dates matter).

What would change it: the deployment's time zone becoming available to the client outside a journey, at which point `Session.today()` should use it; the engine giving the days late, at which point the late-days arithmetic in the web app goes.
