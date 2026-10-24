# A date ahead holds a node off the acting frontier

Question: future-dated milestones read as ready, and work before its own not-before date read
ready and sat on today's next list. The owner asked that future milestones (with or without
marking themselves done) and work held by its own not-before date read scheduled and leave the
frontier, that what depends on them read blocked naming the date, and that work held by a date
can be started early but not finished early without a reason. Which dates hold what, and how
does the rest of the engine follow?

## The call

- **Scheduled** is one per-node fact, decided once where blocking is derived: an unstarted,
  actionable node that a date ahead holds. It stays on the frontier (actionable is unchanged)
  and leaves the acting frontier, as a snooze does. Display state, the acting frontier, the
  waits a dependent names, and the stall diagnostic all read that one fact.
- **Which dates.** A milestone that marks itself done: its effective date, as before. Any
  other milestone: its pinned date, a pin or a fed date answer. A deliverable, action, or
  decision: its own not-before date, the latest over its own `not_before` rules of each
  source's earliest date plus the offset, read off those rules alone and never off the chain
  that binds its earliest start (so an upstream estimate never reads as its own date).
- **A milestone dated only by its derived due is not scheduled.** A derived due is a "by"
  date, a deadline, not the day the event happens; holding it would take every unpinned
  milestone with a deadline ahead off the frontier (the vendor evaluation's kickoff, due in
  three weeks, would leave the journey stalled at its second step). It stays ready, confirmed
  when it happens. This is the call the owner may most want to confirm.
- **Not held.** A group (it has no completing move); a container's children, by their
  container's date (its date is on its own start, which comes after its children); work
  already started, which stays active and on the acting frontier.
- **No guard.** A date ahead orders the work and never stops a move: work can be started or
  completed, and a decision answered, before its own not-before date as a plain move, and the
  date moving later never makes finished work stale
  (`2026-10-24-recorded-dates-never-warn.md`). Marking a milestone done records when it
  happened.
- **Stall diagnostic.** The auto-done cause becomes one scheduled cause: the node, the date,
  and whether it marks itself done. Every node a date holds off the acting frontier is named,
  so the diagnostic's assertion holds. The panel offers Mark done for a milestone and Start
  for work; a decision is answered from its own page.
- **Waits.** A blocker carries the date when a date holds it, so "Waiting on Launch on Nov 1".
- **Summary** lists scheduled work and decisions with their dates (milestones stay in the
  dates panel).

## Alternatives

- Count a derived due as a milestone's date (its effective date). Rejected for the reason
  above: a deadline is not an event's date.
- Hold an unpinned milestone by its own not-before rule (the launch's retrospective, five days
  after launch). Left out: the request scoped own-date holding to work and decisions, and an
  unpinned milestone is confirmed when it happens. Easy to add in the same place if wanted.
- Measure a not-before rule from its source's derived due rather than its earliest date. The
  earliest date is what the date network already uses for the node's earliest start, so the
  scheduled date and the earliest start agree.

## What would change it

The owner wanting unpinned milestones held by their derived due or their own not-before date,
or a deadline that holds work back, would move the line between ready and scheduled.
