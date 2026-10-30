# Recorded dates never warn, and a reopen is the one signal

Question: what does out-of-order work mean, and what still warns?

Call:

- Order is never a guard. A start, a completion or an answer never stops for an unfinished dependency or a date not yet reached. Blocking and scheduling decide what the next list offers, never what a move may do. Evidence stays a guard, bypassable with an optional reason.
- Recorded dates never warn. Shortfalls come only from the plan ahead: a finished node answers to none of its own constraints, and what falls short because of a recorded date shows on the remaining work it holds. A rule of another node that ends at a finished node still binds.
- Early work reads like any other work: no "started early" or "done early".
- Reopening is the one signal. Finished work is stale while a dependency it relied on (done when the work finished) is open again. Finishes and reopens are ordered by the journey's revision, not by day: a reopen records its own revision and that of the finish it undid. Writes in one patch share a revision and do not rely on each other. Editing an actual finish date never changes what is stale. A node whose state is derived (a group, a self-marking milestone) counts as finished when everything it waits on was.

Alternatives: a per-completion record of what it relied on (a stored per-node record the owner does not want); deriving early-work labels (early work needs none); ordering by day or timestamp (same-day ties misorder, and clocks step back); exempting every constraint into a finished node (drops a deadline before a milestone once the milestone happens).

What would change it: people missing a warning when work goes ahead of something that then changes, which would call for a derived reading of recorded dates, never a guard; or ordering across journeys.
