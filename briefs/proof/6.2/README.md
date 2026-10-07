# Proof for brief 6.2: Durability testbed

`mise run sim` now runs the service on the real Turso store in a simulated filesystem, with one
client submitting a seeded 24-step plan through failed fsyncs, torn writes, and crash-restarts.
After every restart and at the end it checks that no acknowledged commit is missing, each commit
is whole or absent, and the state equals the engine's replay of the log.

## A fault that used to lose a commit

Seed 12: its first commit's log fsync fails. Before the fix, that commit was answered from a
receipt that never reached the disk, and a crash before the next log sync lost it. The store now
syncs the log with a barrier commit before it answers.

| Run | Outcome |
|---|---|
| Fault-free | pass |
| A failed fsync | pass, answered once the barrier synced the log |
| A crash-restart | pass |
| Both | pass (lost step 0 before the fix) |

## Crashing at each commit point

Seed 1, crashing after each sync of one step. The restarted run audits the store, resubmits the
step in flight if it did not land, and finishes with every invariant holding.

| Crash after sync | Step in flight | Last commit point recorded | Landed |
|---|---|---|---|
| 38 | 2 | after the revision row | no |
| 39 | 2 | between state and events | no |
| 40 | 2 | before commit | no |
| 41 | 2 | before commit | yes |
| 42 | none | - | - |
| 43 | 3 | none | no |
| 44 | 3 | before begin | no |

A sweep crashing after every 11th write and sync, with byte-granular tearing, recovered and
completed 28 of 28 runs.

## Known limits

- The smoke campaign draws no crashes, so the crash oracles are exercised by the sweep only.
