# The store is SQLite with one writer

**Question.** Commits to different domains could run concurrently, but concurrent writers cost the store its most complex code (conflict detection, retries, sequencing, foreign keys that do not hold between transactions). Is one writer enough?

**Call.** Yes. The store is SQLite (rusqlite, bundled) in WAL mode with `synchronous = FULL`:
- One writer connection runs every write, one `BEGIN IMMEDIATE` transaction at a time, so transactions never conflict and log order is commit order. Readers are a small pool of `query_only` connections that never wait on the writer.
- Each call runs its whole transaction on tokio's blocking pool, so a slow `fsync` never holds a runtime thread.
- The `unix-excl` VFS gives one process the database and keeps the WAL index in memory, with no mapped `-shm` file.
- SQLite is built without allocation statistics, whose global mutex serializes readers.
- Migrations start at 13; a database that records an earlier schema is refused.

**Alternatives.** Concurrent writers (`BEGIN CONCURRENT`, or `BEGIN IMMEDIATE` with a busy timeout: conflict handling or sleeps on runtime threads, for throughput no workload needs); sqlx (a thread per connection and more dependencies); statements on the caller's task (a slow disk stalls the runtime); the default VFS (a second lock to keep right).

**What would change it.** Commits measurably queuing behind each other, or several processes on one database (Postgres).
