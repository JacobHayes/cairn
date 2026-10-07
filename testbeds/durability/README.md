# Durability testbed

Brief 6.2 (PRACTICES, Simulation with patina: Testbeds, `durability`). The service runs on
the real Turso store in a directory of patina's simulated filesystem, under filesystem
faults (`EIO`, `ENOSPC`, `EINTR`, short reads and writes) and crash-restart with torn
writes. The question it answers: after every restart, does the state equal the replayed
log, and is every acknowledged commit still there? Exploratory: `mise run sim` runs it,
locally and nightly in CI; `mise run check` never builds it (its own Cargo workspace).

## The program

- `plan.rs`: a seeded plan of 24 domain patches over three journeys and the deployment
  (journey creates, nodes, renames, transitions, edges, entity creates, some riding in a
  journey patch). A pure function of its seed; its tests apply every patch of 64 plans
  through the engine, so a rejection in a run is a finding.
- `ledger.rs`: the client's memory on the same disk: the plan's seed, an intent before
  each submission, each commit point the store's fault hook reports for the step in
  flight (`AfterRevisionRow` and the rest, so a crash selector can land right after one),
  and the acknowledgement with its revision. Each entry is checksummed and synced before
  the client acts on it; a torn last entry is cut off on reopen.
- `world.rs`: the service over `TursoStore` as a composition root builds it. One client
  submits each step until it is answered, retrying a failed write with the same patch id
  (H5) and reopening the store after three failures in a row. Buggify sites lose an
  acknowledgement (the resend must be answered from the receipt) and reopen the store
  between steps.
- `audit.rs`: the invariants, read from the store: every acknowledged step has its events
  and its receipt at the acknowledged revision; a commit is whole or absent; the log holds
  only planned, submitted steps, once each, as a prefix of the plan; every journey and the
  deployment equal the engine's replay of the log from empty (J3). Each broken invariant
  is a `violation` verdict and the run exits 1.
- `restart.rs`: in an incarnation that starts on an existing ledger, before the store
  opens it notes the step in flight, its last commit point, and whether Turso's logical
  log ends inside a frame; after the restart audit it fires the crash oracles and checks
  that the step in flight, resubmitted, is answered from its receipt exactly when it
  landed.

Arguments: `--dir`, `--steps`, `--plan-seed`, `--open-attempts` (only to reproduce a Turso
gap), `--bug ack-before-commit` (the planted client bug). A run ends with a `pass` verdict
whose detail is echoed as `DURABILITY_RESULT outcome=complete ...`, or `outcome=unavailable`
when the store did not open on an erroring disk.

```
cargo patina build . --output target/patina/cairn-durability
cargo patina run target/patina/cairn-durability --allow-unsupported-symbols <list> \
  --seed 1 --fs-crash-at sync:40 --fs-torn-granularity byte
mise run sim     # every leg of sim.sh, from the repository root
```

## sim.sh

1. rustfmt, clippy, unit tests.
2. Build and audit. The only unsupported symbols are `dlopen`, `dlclose`, and simsimd's
   AVX-512 FP16 kernels, which Turso links and Cairn never reaches (6.1's patina entry),
   allowed by name; rustix's raw syscall runs through syscall-user dispatch.
3. A crash-restart run recorded twice is byte-identical and replays its verdict.
4. Smoke campaign: 32 generations of buggify, fs errors and short I/O (bands at 30 per
   mille of their default), and swarm. Every generation passes, and every oracle fires but
   a named out-of-reach list (two writers in flight, proposals, imports, derived reads, and
   a failed write retried, which leg 6 gates).
5. Crash sweep: crash-restart after every 11th write and sync, whole-block and byte
   tearing, seeds 1 and 2 (112 runs). Every run restarts and completes with the invariants
   holding, and the crash oracles fire: a crash between a commit's state rows and its
   events, an interrupted commit lost whole, one that had landed, a restart recovering
   acknowledged commits.
6. The same sweep with fs errors at 5 per mille: no invariant breaks, and some commit whose
   log sync failed is answered applied once a barrier synced it
   (`turso-failed-commit-applied-once-synced`), and some failed write is retried. The store
   finding below is pinned as a regression run (seed 12, a crash at `sync:28` after its
   first commit's log fsync failed), which must complete; with the fix removed it loses
   that acknowledged commit.
7. Known gaps still reproduce.

## Findings

All in `DECISIONS.md` (brief 6.2, and the log sync fix).

- **Store: an acknowledged commit is lost after a failed log fsync (fixed).** Turso keeps a
  commit visible after its log `fsync` fails and its `COMMIT` errors; the resubmission was
  answered from that receipt and acknowledged; a crash before the next log sync lost it.
  Found at seed 31, `--fs-error-permille 5 --fs-crash-at write:40`. The store now answers
  nothing after such a failure until a barrier commit has synced the log past it, and
  fails closed until reopened when it cannot. Its deterministic tests are
  `crates/store-turso/tests/conformance/log_sync.rs`; leg 6 pins seed 12 at `sync:28`.
- **Patina: a removed directory stops crash points.** Once the guest removes a directory
  (Turso's temporary directory, when a connection that wrote is dropped), no later
  `--fs-crash-at` point fires and the run ends uncrashed. So the store's open runs its
  barrier on a pooled connection, which keeps its temporary directory. An in-process
  reopen still drops the pool, so a run that reopens is not crashed after it.
- **Turso: opens under injected faults.** A log size error at open panics; a short read of
  the log header fails the open; an open retried in-process after a failed one panics in
  the page cache; `EINTR` is reported, not retried.
- **Patina: no crash in campaigns, no partial Turso frame.** A campaign never draws a
  crash, so the crash oracles carry constant labels (out of the link-time table) and the
  sweep gates them. Byte tearing tears the ledger's appends but never leaves part of a
  Turso log frame, so the torn-tail oracle has not fired.
- Reused from 6.1: whitespace-free site labels, verdicts rather than `always!`, the
  symbol allowance.
