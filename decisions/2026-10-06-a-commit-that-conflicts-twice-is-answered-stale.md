# A commit that conflicts twice is answered stale with the revision in flight

- Question: ARCHITECTURE (Concurrency and notification) retries a Turso write-write conflict once, then answers a revision conflict. When the second attempt conflicts too, the commit it lost to is still in flight: no revision has moved yet, so there is no current revision to report.
- Call: the answer is `stale`, listing the target with the revision the in-flight commit is producing (the base plus one), any other revision the patch named that has moved, and the touched set of whatever has committed since. The client's refetch-and-resubmit is the retry; no backoff knob.
- Alternatives: report the revision read (then `expected` equals `current`, which reads as no conflict); retry with a backoff (a knob, and commits take milliseconds).
- What would change it: the multiplayer testbed (6.1) seeing second conflicts often.
- Superseded by the H5 fix ("a resubmission beside its own original in flight is answered stale"): commits queue in process on what they write, and a second conflict with nothing moved fails instead of naming the revision in flight.
