# Every read is one keyed-read actor

Question: each read in the tab has to decide whether an answer is still wanted, whether a failure retries, and when a held derivation is freed. Reads that decide this each for themselves disagree. Where do these rules live?

Call: one machine, `keyedRead`, behind one hook. A key is an identity (what is shown), a freshness and arguments; a source says how a read starts, which ticks keep it current, and what releases a value.

- Failures are classed: a missing subject, version skew, a denied caller and an engine refusal are never retried; only a transient failure retries, on the shared backoff.
- A value that owns a resource (a derivation in the worker) is held by an owner that releases it when the value is replaced or the read stops, so no path forgets a release.
- A derivation belongs to the load that made it: the worker numbers its loads, and a release frees only the load it names, so a late release from a replaced owner frees nothing.
- A failed refetch keeps the shown value and reports lag; it never replaces a good graph with an error.

Alternatives: a query-cache library (its staleness model is time, ours is the stream's revisions, and it does not model releasing a derivation); releasing by journey id or a refcount (a late release frees the wrong owner's derivation).

What would change it: a read that must run concurrently with itself for one identity.
