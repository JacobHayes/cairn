# One voice for dates, due words, snoozes, flags and trouble counts

Question: dates, due words, snoozes, flags and trouble counts appear on many surfaces. Which words does each fact take, and where do they live?

Call: each fact has one home that every surface calls: dates and due words in `status/when.ts`, snooze endings in `status/snooze.ts`, the trouble table in `status/words.ts`, and one list of a node's flags. Words:

- The derived flag and every count of nodes say "overdue"; a duration says "N days late". A shortfall flag or count says "short of days"; a quantity says "N days short".
- A decision's date is "Decide by <date>"; other work is "Due today", "Due tomorrow", "Due in N days" within two weeks, then "Due <date>". One window for the words, one (a week) for the warning ink.
- A date's origin is "pinned", "recorded" or "worked out".
- A milestone's date is nearness then the date: "in 18 days (Nov 23)".
- A snooze on a node ends "<node> is done".
- Screens with no journey take today from the session's clock in the tab's zone, never the UTC date.

Alternatives: a second, seven-day window for card due words (two wordings of one fact); "Estimated" for a derived date (one word must win).

What would change it: the deployment's time zone reaching the client outside a journey, which today's session clock should then use.
