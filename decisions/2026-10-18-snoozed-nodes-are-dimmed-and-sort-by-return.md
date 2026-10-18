# Snoozed nodes are dimmed, and sort by when they come back

Question (from the card-overflow follow-up): a snoozed node is off the acting frontier (glossary, snooze) yet still blocks what depends on it and counts for gravity, so it stays on the graph, list and timeline. Should it look different from a resting node, and where should it sort?

Call, look: snoozed becomes a level of the status language in DESIGN.md ("fill says state, opacity says focus, lowest wins"), not a mechanism beside it: 1 resting, 0.7 snoozed (`--opacity-snoozed`, one token in `design/tokens.css`), 0.5 done, 0.3 out of focus, never multiplied, so a snoozed node out of focus is 0.3. It is applied identically to a card's title and owner, a list row's cells but the status and due ones, and a timeline label's title. The SNOOZED chip, the kind label and the foot date ("Until Oct 31") stay at full strength: they say why it is dim and when it returns. The dimmed ink measures about 7.4:1 on paper in both themes, so 0.7 clears the 4.5:1 floor (the kind label, steel ink, would not at 0.7, so it is not dimmed). 0.7 is between a resting node (1) and done (0.5), so it reads "not now", not "finished". The status language's rule holds: the lowest opacity applies, never a product, so a snoozed node out of focus is 0.3. A card's foot also says only "Until <date>" or "Until <node> is done" (the full sentence is the tooltip), since the chip already says snoozed.

Call, order (display order only; the shared rank is unchanged):
- Plan tree order, and the timeline which follows it: a snoozed node's date for sorting siblings is the later of its current date and the day its snooze lifts. A date snooze lifts on its date; a node snooze at that node's derived finish date, and with none the node counts as undated and goes after the dated. Every hold counts, the node's own and each holding ancestor's, and the latest wins; a node snooze lifts at the finish of the node it waits on (a milestone's effective date, else any node's due date). A container still sorts by the soonest thing beneath it.
- A flat list sorted by a signal (rank, slack, gravity, unlocks, due): snoozed rows come after the unsnoozed ones in either direction, keeping their order among themselves. Rank answers "what now", and the next list already leaves the snoozed out.

Alternatives:
- Leave snoozed undimmed and in place: at fit it is indistinguishable from work that can be done now, and it crowds the top of a plan it cannot act on.
- Dim to done's 0.5: confuses "not now" with "finished".
- Dim blocked and scheduled too: blocked work is still the plan's next problem, and scheduled milestones are no one's action, so neither is "not now".
- Dim the whole card, chip included: the chip and date lose the contrast they are there for.
- Change the shared rank to demote snoozed nodes: it would move them in every consumer; the rank is a statement about what to do, and the next list already excludes them.

What would change it: owner feedback that 0.7 is too light or heavy (change the one token), or a wish to dim other states.
