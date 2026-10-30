# The multiplayer testbed runs a multi-thread runtime with four workers

Question: which tokio runtime does the multiplayer testbed serve on?

Call: a multi-thread runtime with four workers (`WORKER_THREADS`). The binary uses one worker per CPU, but Patina presents one CPU, so tokio's default would start a single worker. Four let requests run on several threads and PCT reorder them, reaching interleavings of the store's pool, the notifier and the event stream that one thread never makes. The count describes the testbed's world (a four-core host), not configuration.

Alternatives: a current-thread runtime (not what ships, and it never reaches multi-thread interleavings); tokio's default count (one worker under Patina); a leg per runtime (a second configuration nothing ships).

What would change it: Patina presenting the host's CPU count, or multi-thread runs that stop being reproducible.
