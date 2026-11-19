# Durability testbed

The durability testbed (PRACTICES, Simulation with patina). The service runs on the real
SQLite store in a directory of patina's simulated filesystem, under filesystem faults (`EIO`,
`ENOSPC`, `EINTR`, short reads and writes, failed fsyncs), buggify sites and machine crashes
with restarts. The question it answers: after every restart, does the state equal the replayed
log, and is every acknowledged commit still there?

## The program

- `plan.rs`: a seeded plan of 24 domain patches over three journeys and the deployment
  (journey creates, nodes, renames, transitions, edges, identity creates, some riding in a
  journey patch). A pure function of its seed; its tests apply every patch of 64 plans
  through the engine, so a rejection in a run is a finding.
- `ledger.rs`: the client's memory on the same disk: the plan's seed, an intent before
  each submission, each commit point the store's fault hook reports for the step in
  flight (`BetweenStateAndEvents` and the rest, so a restart knows how far it got),
  and the acknowledgement with its revision. Each entry is checksummed and synced before
  the client acts on it; a torn last entry is cut off on reopen.
- `world.rs`: the service over `SqliteStore` as a composition root builds it, opened again
  after a failed open as a supervisor would start the server again. One client
  submits each step until it is answered, retrying a failed write with the same patch id
  (H5) and reopening the store after three failures in a row, unless the store has failed
  closed, which ends the run unavailable. Buggify sites lose an
  acknowledgement (the resend must be answered from the receipt) and reopen the store
  between steps.
- `audit.rs`: the invariants, read from the store: every acknowledged step has its events
  and its receipt at the acknowledged revision; a commit is whole or absent; the log holds
  only planned, submitted steps, once each, as a prefix of the plan; every journey and the
  deployment equal the engine's replay of the log from empty (J3). An audit's broken
  invariants are reported together as one patina `violation` and the run exits 1.
- `restart.rs`: in an incarnation that starts on an existing ledger, before the store
  opens it notes the step in flight, its last commit point, and whether SQLite's
  write-ahead log ends inside a frame; after the restart audit it fires the crash oracles and checks
  that the step in flight, resubmitted, is answered from its receipt exactly when it
  landed.

Arguments: `--dir`, `--steps`, `--plan-seed`, `--bug ack-before-commit` (the planted
client bug). A run that passes prints `DURABILITY_RESULT outcome=complete ...` and exits 0. A
store that did not open, or failed closed, exits 5 (`DURABILITY_UNAVAILABLE`): after an
injected I/O error or failed fsync that is an honest answer, and `patina.toml` makes it an
ok run then; in any other run it is a finding. The data directory defaults to `/work/cairn-durability`,
on patina's simulated disk, which its faults and crashes act on.

## The sweep

`patina.toml` holds the sweep's settings: buggify, the disk faults and machine crashes, each
class on in about half the seeds, so one sweep has runs with crashes alone, errors alone, and
both. Every run must pass its audits, and every site in reach must fire somewhere in the sweep:
the crash oracles (a crash between a commit's state rows and its events, an interrupted commit
lost whole, one that had landed, a torn log tail discarded, a restart recovering acknowledged
commits) and the client's (a failed write retried, a lost acknowledgement answered from the
receipt). The check's sim step runs it over 256 seeds, CI's nightly run over 4096.

```
cargo build --release
patina sweep --seeds 1..256                                      # the step's sweep, from here
patina run --seed 1 --crash machine -- target/release/cairn-durability
patina sweep --seeds 1..200 --crash machine -- target/release/cairn-durability --bug ack-before-commit
```

A finding's `rerun:` line runs it again from any directory.

## Findings

Each finding is recorded in `decisions/`, linked at the end of its item.

- **Store: a commit it cannot settle fails it closed.** After a failed log `fsync` the
  store fails closed and the server exits for its supervisor to restart; every open drops
  the files' cached pages first. Patina keeps the dirty data of a failed fsync (Linux drops
  it) and accepts `posix_fadvise` without changing anything, so a run cannot show the
  restart reading from disk: the world ends the run when the store fails closed, instead
  of reopening it in-process. The rule is pinned by
  `crates/tests/tests/integration/store_sqlite/conformance/conformance/fail_closed.rs`,
  which injects the failure in place of the `COMMIT`; the testbed does not judge sync
  failures. Recorded in
  [`decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md`](../../decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md).
- **SQLite: a `SIGBUS` in the log index, and fail-closed on every write fault.** With SQLite's default VFS, two simulated runs died of `SIGBUS` writing the
  write-ahead log's index into a memory-mapped `-shm` file. The cause is patina's: it
  loads a newly mapped file with one fault-injected read and sizes the mapping to what that
  read returned, so a short read leaves pages past it unbacked, which Linux never does (a
  mapping's size is the file's). The store now opens on `unix-excl`, which keeps that index
  in memory, so nothing is mapped. SQLite writes a
  commit's log frames only at `COMMIT`, so any write fault in a commit fails the store
  closed: about a third of the sweep's runs end unavailable, with no invariant broken. A
  read fault can answer "database disk image is malformed": SQLite reports `EIO` on a read as
  `SQLITE_IOERR_CORRUPTFS`; the retry succeeds. The world retries a failed open, as a
  supervisor's restart would, since SQLite opens again cleanly in-process. Recorded in
  [`decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md`](../../decisions/2026-10-21-the-store-is-sqlite-with-one-writer.md).
- Shared with the multiplayer testbed: every broken invariant of a run reported together
  rather than one `always!` each
  ([`decisions/2026-10-18-testbed-invariants-stay-verdicts.md`](../../decisions/2026-10-18-testbed-invariants-stay-verdicts.md)).
