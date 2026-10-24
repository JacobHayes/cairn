# Recorded dates never warn

Question (from the unit that took the barriers out of early work): a start ahead of what a
node waits on, or ahead of its own date, was refused until the person chose "Start anyway";
finishing or answering ahead needed "Done anyway" or "Answer anyway"; the engine stored a
bypass of the latest move to tell work done early on purpose from work that fell short, and
the node's sentence said "Started early, ahead of A" or "Done early, 3 days before A". The
owner asked why Cairn puts barriers in the way of doing work. What does out-of-order work
mean, and what still warns?

## The call

- **Order is never a guard.** A start, a completion or an answer never stops for an
  unfinished dependency or for a date not yet reached. The dependencies-done and
  date-arrived guards are gone, and with them Start anyway, Answer anyway, the milestone's
  Done anyway, the stored order bypass and the derived start guards. Blocking and scheduling
  still order the work: they decide what the next list offers, never what a move may do.
- **Evidence stays a guard.** Completing a deliverable or action still needs the artifact,
  note, ticked checklist or breakdown its route asks for, bypassable with an optional reason.
  A bypass is sent with the completion it covers, in the same patch, and is cleared only by
  reopening.
- **Recorded dates never warn.** Shortfalls come only from the plan ahead. In the execution
  layer a finished node (done or decided, not skipped) answers to nothing: every constraint
  that holds it back (its dependencies, condition gates, stage openings, entry hops, and its
  own `not_before` and `due_by`) is kept in the network, disabled. An instant fixed by a
  recorded date never reports a shortfall; what falls short because of it shows on the
  remaining work it holds. Rules of other nodes that end at a finished node (a remaining
  node's deadline before a milestone that has happened, a stage's close) are not the
  finished node's own and still bind.
- **Early work reads like any other work.** No "started early" or "done early" value or
  sentence: active is active and done is done (the owner: "I'm not sure it's important to
  anybody").
- **Reopening is the one signal.** Reopening a finish records on the reopened node the day
  and the finish it undid (`NodeState.reopened`); finishing, answering, skipping or marking
  it done again clears it. Finished work that relied on it, a dependency done when that work
  finished, is stale while it is open again. A node whose state is derived (a group, a
  milestone that marks itself done) was done on the days everything it waits on was, read in
  one sweep in blocking's order, so a reopen inside a group reaches what requires the group,
  and a group that still had other work open was never relied on. Reopening a skip or a mark
  done records nothing: nothing was done to rely on.
- **Old data, migrated once** (migration 21): the removed guards and their failures are
  stripped from stored bypasses, proposals and events; a bypass left with no guard is
  removed, an override record left empty is deleted (and its write becomes a removal, so
  replay agrees), and a bypass mutation left with no guard is dropped from its proposal.

## Alternatives

- **Keep a per-completion record of what it relied on.** Exact, but it is the stored
  per-node record the owner asked to remove; the reopen record lives on the act that is the
  signal and costs nothing on a start or finish.
- **Back-fill reopen records from the event log.** The event time's day depends on the
  deployment's timezone, and the only deployment has no reopen events.
- **Derive "started early" and "done early" from recorded dates.** Worked out and then
  dropped at the owner's call: early work needs no label.
- **Exempt every constraint into a finished node.** It would drop a remaining node's
  deadline before a milestone once the milestone happened.

## Known limits

- A prerequisite reopened before this change carries no record, so what relied on it is not
  stale until it is reopened again.
- Dates are days, so a dependent finished on the day its prerequisite was reopened counts as
  having relied on it.
- Reliance is read against today's graph: a dependency added after the work finished, on
  something done then and reopened since, counts as relied on.
- A milestone that marks itself done, has no pin, and is dated only by a dependent's
  deadline loses that date once the dependent finishes.

## What would change it

People missing a warning when work goes ahead of something that then changes (beyond a
reopen), or wanting early work called out again, which would bring back a derived reading of
recorded dates, never a guard.
