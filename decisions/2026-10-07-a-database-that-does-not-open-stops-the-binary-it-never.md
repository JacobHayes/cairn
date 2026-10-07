# A database that does not open stops the binary; it never retries the open

- Question: 6.2's durability testbed found that Turso 0.8.2 panics when an open is retried in the process a failed open ran in, and that some open failures are unrecoverable (DECISIONS.md, 6.2: [turso] an open under injected I/O faults panics or fails where it could recover). What does the binary do when its store does not open?
- Call: `cairn serve` and `cairn migrate` open the store exactly once. A failed open is a startup error: the process prints it, naming the database, and exits nonzero; a panic inside the open aborts the process (the panic policy). Recovery is a restart by whatever supervises the process, in a fresh process. A test runs both commands over a database path that cannot be opened and checks the nonzero exit and the named path.
- Alternatives: retrying the open with a backoff in process (reaches Turso's panic); serving without a store until it opens (every request would fail, and a half-started server hides the fault from the supervisor).
- What would change it: a Turso release that recovers from a failed open in process, which 6.2's gap legs would show.
