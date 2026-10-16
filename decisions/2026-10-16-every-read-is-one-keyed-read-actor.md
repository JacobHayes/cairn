# Every read is one keyed-read actor

Question (from the keyed-reads work): the tab had six read paths (the journey store, the live read, the deployment and viewer stores, the route canvas's effects, and a dozen effects with a `live` flag for projections, layouts and previews), each with its own answer to "is this still the read I asked for, did it fail, and when does it try again". They disagreed: a stale error showed over a newer load, a failed refetch replaced a route's good graph with an error, a retry fired after its subject was gone or its read had succeeded. What replaces them?

Call: one XState machine, `keyedRead` (`web/app/src/data/keyed-read.ts`), behind one hook (`useKeyedRead`). A key is an identity (what is shown), a freshness (what the answer is read from) and arguments; a source says how a read starts, which ticks keep it current and what releases a value. The journey manager, the deployment and viewer reads, the index and route reads, projections, layouts and previews are sources of it; the subscription is a session child machine.

- Failure classes: a thrown failure is `gone` (the subject is missing, A19, never retried), `skew` (latches the tab, never retried), `denied` (HTTP 401 or 403, never retried), `refused` (the engine's refusal: the same input fails the same way, never retried) or `transient` (retried after the shared backoff, 250 ms to 15 s, as the stream's reopen delay). A derive that the worker superseded is transient: reading again finds the newer document.
- Version skew stops new work only for sources that derive or preview; reads of the host and projections of a derivation already held keep answering in skew.
- Ticks are listened to per identity, only while the read is keyed, and credit the revisions an answer holds; a tick that asks for a revision the answer did not reach is tried again with no further tick.
- A value that owns a resource (a derivation) is held by an owner child that releases it when the value is replaced, gone, or the read stops, so a release cannot be forgotten on any path.
- A route canvas whose refetch fails keeps its graph and is counted by the sync dot as behind (it reports lag against the route's revision); it does not show an error in place of the graph. A route that no longer exists shows that it does not exist.
- Previews wait for the key to hold still (the existing settle times, no new limit), run only while the tab is compatible, and check that the read is still wanted right before the call that posts work to the worker.

Alternatives: keep the separate stores and fix each defect where it was found (the defects share a cause, so they would come back one by one); a data library with a query cache (the cache's staleness model is time, ours is the stream's revisions, and a derivation must be released by its owner, which a cache does not model); fold previews and layouts into plain effects with an abort flag (the shape that produced the defects).

What would change it: a read that needs to run concurrently with itself for one identity (the machine runs one at a time and coalesces), or a limit on parked reads other than the journey manager's eight.
