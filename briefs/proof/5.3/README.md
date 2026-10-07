# Proof for brief 5.3: List, next, triage, decision walkthrough

A journey now has three acting screens beside its canvas: the next list in rank order with why
each item ranks there, the list as a filterable table whose bulk actions go as one patch, and
triage one card at a time, whose decisions-only mode is the walkthrough a new journey opens on.

## Screens

A fresh vendor evaluation opens the walkthrough on every decision actionable at its start:
![The walkthrough at the start](1-walkthrough-at-the-start.png)

Answering that a partner runs the testing surfaces the partner-led work in the same pass:
![The answer surfaces partner work](2-the-answer-surfaces-partner-work.png)

Triage of every kind, one card at a time:
![Triage of every kind](3-triage-every-kind.png)

A placeholder's card offers mark atomic and snooze, and no done:
![A placeholder's card](4-placeholder-card.png)

With every decision skipped, the walkthrough shows what would unblock the next ones:
![What would unblock the next decisions](5-what-would-unblock-the-next-decisions.png)

The product launch's next list, each item with its breadcrumb and why it ranks there:
![Ranked](6-next-ranked-with-why.png)

The same list re-sorted by slack:
![By slack](7-next-re-sorted-by-slack.png)

The only acting item snoozed: the journey is stalled, and the panel offers unsnooze:
![Stalled](8-stalled-with-unsnooze.png)

The list filtered to deliverables and actions, grouped by container:
![Filtered and grouped](9-list-filtered-and-grouped.png)

Search reads notes: "environment team" finds the node whose note says it:
![Search reads notes](10-list-search-reads-notes.png)

A bulk done where one node fails its guard is rejected whole, naming that node:
![Rejected](11-bulk-done-rejected-naming-the-node.png)

Two nodes snoozed in one patch until the beta ends:
![Snoozed](12-bulk-snoozed-in-one-patch.png)

The main flow: the walkthrough, the partner decision answered, a pass, triage, and the next
list: [main-flow.webm](main-flow.webm).

## The next list's order

| By rank | Rank | Slack | By slack | Slack |
|---|---|---|---|---|
| `n_launch` | 0.2500 | 47 | `n_docs` | 35 |
| `n_docs` | 0.0500 | 35 | `n_announcement` | 42 |
| `n_announcement` | 0.0500 | 42 | `n_beta_end` | 42 |
| `n_beta_end` | 0.0250 | 42 | `n_launch` | 47 |
| `n_retro` | 0.0250 | none | `n_retro` | none |
