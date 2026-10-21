# The store is SQLite with one writer, and Turso is gone

- Question: from the user's ask to prototype a plain SQLite backend against the conformance suite and see what dropping Turso cleans up. Cairn used one feature only Turso has: MVCC writes (`BEGIN CONCURRENT`), so commits to different domains could run at once. That feature cost the most code in the store: Turso checks only write-write conflicts, so foreign keys did not hold between concurrent transactions, and commits that read what another wrote could both land. The store added dependency revision-row writes, a log sequencer with a read horizon, in-process turns at every revision row, and a retry for a second conflict. Meanwhile the turns already made commits queue: every commit to one domain, and every commit touching an identity, across all journeys. Is one writer enough, and what does it remove?
- Call: the store is `crates/store-sqlite`, rusqlite 0.40.2 over a bundled SQLite in WAL mode with `synchronous = FULL`.
  - One writer connection, behind an async mutex waited for at most `store_connection_acquire`, runs every write: commits and the records outside every domain, each `BEGIN IMMEDIATE`.
  - Reads run on a pool of readers (the same count limit) in deferred transactions, so they never wait on the writer.
  - A call waits for its connection on the runtime, then runs its whole transaction, every statement of it, in one closure on tokio's blocking pool. The inner modules are plain synchronous functions over a connection; only `lib.rs` and the pool know about the runtime. A test's pause inside a commit blocks that blocking thread on the pause's future. Measured, the hand-off costs 5-6 microseconds a call, about 3% of a 150-microsecond journey load. Statements on the caller's task instead would let a slow disk (a cloud volume's `fsync` can take 100 ms or more) hold a runtime thread, and a 2-vCPU host has two: two stalled calls would stop every request, health check and timer.
  - Readers open with `query_only`, so a read path that writes fails instead of writing beside the writer.
  - Since no two transactions ever overlap, these go: the turns (`queue.rs`), the conflict retry and its twice-conflicted answer, the dependency revision-row writes, the sequencer and the readers' horizon (SQLite numbers a log row at insert, and with one writer insert order is commit order), the foreign-key workaround (deferred foreign keys hold), Turso's I/O test seam, the handling of Turso's open panics in the durability testbed, and `SOURCE_DATE_EPOCH` (only Turso's build script needed it).
  - These stay: a `COMMIT` that fails other than by a constraint fails the store closed; the open evicts cached pages (now of `-wal`); the commit fault points.
  - Every connection opens on SQLite's `unix-excl` VFS. The process takes one exclusive lock on the database file at its first access, so one process has the database, as Turso's file lock gave, and `cairn migrate` against a live server is refused. The write-ahead log's index is kept in the process's memory instead of a memory-mapped `-shm` file.
- Migrations: the first migration is 13, the schema the Turso store's migrations 1 to 13 left, as one file (checked equal, column for column and constraint for constraint, to applying those 13 files). Migration 14 (a finished milestone is done) keeps its code step and test. The releases on the Turso store leave a database at 13 (v0.4.2) or 14 (v0.5.0). A database short of 13 is refused: only the Turso store could migrate it. This drops migrations 1 to 12 with their code steps (`to_identities.rs`, the condition backfill) and their tests.
- Existing databases: SQLite cannot open a Turso MVCC file (its header marks a journal mode SQLite refuses, and its writes sit in `-log`). Turso's own shell converts it in one command, `tursodb cairn.db "PRAGMA journal_mode = wal"`.
  - Checked on a database the Turso store wrote with every fixture (4 journeys, 142 events): `sqlite3` then reports `integrity_check` ok and no foreign-key violation, and the new binary migrates and serves it.
  - An unconverted file is refused with a message pointing at the guide (`docs/running.md`, Upgrades and backups).
- Also gone, from an audit after the move:
  - A commit no longer writes its domain's revision row first. That first write was where Turso surfaced a conflict; behind one writer it protects nothing, so the commit checks its receipt and revisions, applies its writes, then sets the revision once. The `AfterRevisionRow` fault point goes with it.
  - The commit answers the deployment revision it left (`Committed::Applied` carries it), so the service announces that instead of reading the revisions back after an identity create rides in, and its fallback for a failed read-back, with the oracle that marked it, goes.
  - The store's checks that it had not failed closed after a call's own successful `COMMIT`: it fails closed only while a call holds the writer.
  - Of the SQLite store's eleven race cases, the eight that showed one commit's outcome beside another in flight become sequential cases, or already were, in the shared conformance suite, which the memory store runs too. Three stay: a long commit beside reads and another domain's commit, a resubmission beside a commit in flight, and a crash mid-commit.
- Review: a hard delete removed a journey's events and the deletion's own event could take a removed position, since `seq INTEGER PRIMARY KEY` hands out one past the largest left; a history cursor past it skipped the deletion (the Turso store's sequencer had avoided this within a process). Migration 15 rebuilds `events` with `AUTOINCREMENT`, and a conformance case, run on both stores, fails without it.
- Simulation: the durability testbed's legs ran in an Ubuntu 24.04 container. On this Mac patina's shim did not build against the selected Xcode's macOS 27 SDK (a struct layout check; the macOS 26 SDK builds it), and its audit then refuses the `fstatfs` call SQLite makes on every open on Apple platforms. Under fs errors, with the default VFS, two campaign generations died of `SIGBUS` in SQLite's `walIndexAppend`. The cause is a patina defect, not one a disk can cause: the shim loads a newly mapped file with one fault-injected read and sizes the mapping's backing memory to what that read returned, so a short read leaves pages past it unbacked, where Linux sizes a mapping by the file. `unix-excl` removes the mapping, so the simulation no longer meets it. Every write error in a commit now reaches `COMMIT` (SQLite writes a commit's log frames there), so it fails the store closed; that is safe, at the cost the earlier fail-closed record accepted. The testbed now retries a failed open as a supervisor's restart would, since SQLite opens again cleanly in-process (Turso panicked). With these, all six legs pass with no invariant broken: 112 crash-restarts recover, and the torn-tail oracle fires for the first time (SQLite leaves part of a frame where Turso never did). Under fs errors (leg 6) 14 of 112 runs complete, where Turso completed 77: the rest fail closed. (2 completed before calls moved to the blocking pool; the thread schedule decides which writes the seeded faults land on, so the count moves with it, and no run breaks an invariant either way.) A read fault can answer "database disk image is malformed", SQLite's name for `EIO` on a read (`SQLITE_IOERR_CORRUPTFS`); the retry succeeds.
- Measured on an Apple M4 (10 cores):

  | | Turso | SQLite |
  |---|---|---|
  | Cold `cargo test --workspace --no-run` (wall / CPU) | 167 s / 1,194 s | 123 s / 998 s |
  | Crates compiled cold | 419 | 334 |
  | Target directory after it | 5.4 GB | 4.8 GB |
  | Rebuild after touching `cairn-store` / the backend | 10.2 s / 5.3 s | 8.8 s / 3.9 s |
  | `cargo test --workspace` (wall / CPU) | 30.4 s / 117 s | 23.5 s / 98 s |
  | Packages in `Cargo.lock` (durability testbed's) | 502 (303) | 406 (89) |
  | Backend source, Rust + SQL lines | 6,887 + 898 | 5,265 + 561 |

- Under load (a release build on the same machine, 32 journeys of 100 nodes, 4 s a workload; each commit rewrites one node; two runs agree within a few percent):

  | Workload | Turso | SQLite |
  |---|---|---|
  | Journey loads, 1 / 8 readers | 2,250 / 8,800 a second | 6,900 / 21,700 a second |
  | Commits, 1 writer | 800 a second, p99 1.6 ms | 2,420 a second, p99 1.0 ms |
  | Commits, 8 writers on their own journeys | 2,400-2,500 a second, p99 31-35 ms | 2,460-2,500 a second, p99 6.6-7.2 ms |
  | Commits, 8 writers on one journey (stale ones retried) | 168 a second | 288 a second |
  | 8 readers beside 1 writer | 7,400 loads, 475 commits a second | 16,300 loads, 855 commits a second |
  | 8 readers beside 8 writers | fails: loads and commits answer "database is locked" | 14,400 loads (p99 0.9 ms), 830 commits (p99 14 ms) a second |
  | History pages beside 4 writers | 19,000 pages, 995 commits a second | 54,000 pages, 1,020 commits a second |

  - The bundled SQLite is built without allocation statistics (`SQLITE_DEFAULT_MEMSTATUS=0`, set for every build in `.cargo/config.toml`). They take one process-wide mutex on every allocation, so 8 readers loaded 2,200 journeys a second against 6,900 for one; without them, 21,700. A unit test reads the compile options, so a build that loses the flag fails. rusqlite offers no safe call to turn them off at run time, and the workspace forbids unsafe code.
  - Concurrent writers no longer gain throughput, but no longer queue behind Turso's turns and retries either: the tail is shorter at the same rate.
  - A commit that adds one node appends about 68 KB to SQLite's log (17 page frames), against about 670 bytes to Turso's logical log. That is why far more of leg 6's commits meet a write fault.
- Costs:
  - A commit now waits for every commit in flight, not only for those sharing a revision row. A slow commit near the acquire limit makes the others answer 503, where before only commits to its domain or touching identities did. Commits take milliseconds, and identity-touching commits already queued deployment-wide.
  - A failed fsync cannot be injected deterministically: Turso's I/O seam is gone, and SQLite's (a custom VFS) needs unsafe code, which the workspace forbids. The store's test injects the failure in place of the `COMMIT` instead (`fail_closed.rs`), so it pins what the store does with an unknown outcome but not SQLite's own handling of the sync; the durability testbed's fs-error legs exercise that.
- Alternatives:
  - Keep Turso.
  - sqlx's SQLite driver: a worker thread per connection and more dependencies, for no gain on a local file.
  - Statements on the caller's task: no hand-off, at the cost above.
  - A thread per connection with closures sent over a channel (tokio-rusqlite, async-sqlite): an actor per connection, a dependency, and closures that must be `'static` all the same; or deadpool-sqlite's `interact`, which is `spawn_blocking` behind a general pool we have no use for.
  - One `spawn_blocking` per statement: a hand-off for every statement, and transactions spread over several threads.
  - Several writers with `BEGIN IMMEDIATE` and a busy timeout: sleeps on runtime threads, and a `SQLITE_BUSY` path to handle.
  - Keep migrations 1 to 12: they matter only to Turso files, which need converting anyway.
  - The default VFS, with a mapped `-shm` and a separate lock file for one process: an online `sqlite3 .backup` beside the server, at the price of a second lock to keep right, and a simulation that reaches patina's mapped-file defect until it is fixed upstream.
  - Accept any migration and convert in the new binary: it would need Turso linked in.
- What would change it: transient write errors restarting servers in practice. With the index in memory, a `COMMIT` whose outcome is truly unknown is one whose log sync failed (`SQLITE_IOERR_FSYNC`); a failed frame write leaves no whole commit frame for recovery to find. Failing closed on those codes alone would recover leg 6's availability, a narrowing that needs its own record and tests. Also: commits measurably queuing behind each other (then SQLite's `BEGIN CONCURRENT` branch, or Postgres); several processes on one database (Postgres, already `Later`).
- Supersedes:
  - decisions/2026-10-06-a-turso-commit-writes-the-revision-row-of-every-domain.md
  - decisions/2026-10-06-turso-checks-no-foreign-key-against-a-concurrent-transaction.md
  - decisions/2026-10-07-turso-an-open-under-injected-i-o-faults-panics-or-fails.md
  - decisions/2026-10-08-turso-is-embedded-through-the-turso-crate.md
  - and the mechanism, not the behaviour, of decisions/2026-10-06-a-resubmission-beside-its-own-original-in-flight-is-answered.md: a commit beside one in flight still waits and then meets its receipt or what really intervened, by waiting for the writer instead of taking turns.
