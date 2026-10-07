# Proof for brief 6.1: Multiplayer testbed

`mise run sim` now drives Cairn's real HTTP server, Turso store, and Rust client with four HTTP
clients and one in-process agent patching one journey at once, under seeded network faults and
fault sites, and checks nine invariants at the end of every run.

## One seed under each fault

Seed 1, every run passing its invariants. Surfaced is patches shown to the user as conflicts;
resubmitted is automatic safe retries (H5); receipts is answers given from a patch's receipt.

| Run | Acknowledged | Surfaced | Resubmitted | Receipts | Writes take (us) |
|---|---|---|---|---|---|
| Fault-free | 38 | 2 | 3 | 0 | 87000 |
| Connections reset (10%) | 38 | 2 | 12 | 10 | 267000 |
| Connections refused (20%) | 38 | 2 | 7 | 0 | 147000 |
| Segments delayed and 10% dropped | 36 | 4 | 40 | 0 | 341000 |
| Answers lost and commits held (fault sites) | 34 | 6 | 24 | 10 | 517000 |

Clients conflict even without faults, so some patches are retried and some overlapping renames
are surfaced. A lost answer after a patch landed comes back from its receipt; a refused connect
never reaches the server, so it never does.

## What it found

- A product bug, now fixed: a patch resubmitted while its original was still committing was
  answered stale with nothing intervening, so its caller was told it conflicted although it
  landed. A Turso commit now takes its turn at the rows it writes and at its patch id, and the
  client never rebases backward.
- With Turso's commit-time revision check broken on purpose, every generation of the 16-generation
  smoke campaign fails `acknowledged-visible`; generation 0 minimizes to seed
  5133223892554006150 with only `--buggify=737`.

## Known limits

- Ten service paths (proposal and import resubmissions, memo reads, deployment-revision reads)
  are out of this testbed's reach and never fire in its campaign.
- Turso links symbols the shim does not support; runs allow exactly those by name
  (`testbeds/multiplayer/README.md`, gap 4).
