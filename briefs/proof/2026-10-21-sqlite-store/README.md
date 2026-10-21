# Proof: the store is SQLite

Cairn keeps its data in a plain SQLite file in WAL mode, written by one connection at a time.
An operator converts a database an earlier release wrote with one command, and with the
server stopped can open the file with any SQLite tool. Developers get a smaller, faster build.

## A database from an earlier release comes over

Observed on a database the Turso store wrote with every fixture:

| Step | What happened |
|---|---|
| `sqlite3 cairn.db` before converting | `file is not a database`: Turso's header and its `-log` file |
| new `cairn migrate` before converting | refused: "a database an earlier release wrote opens only once converted; convert it as the guide to running a deployment says, under Upgrades" |
| `tursodb cairn.db "PRAGMA journal_mode = wal"` | answers `wal`; the log is folded into `cairn.db` |
| new `cairn migrate` | applies migration 15 after the 14 recorded; `integrity_check` ok, no foreign-key violation, 4 journeys, 142 events |
| `cairn serve` | `/healthz` 200; `/api/journeys` lists all four; a derived read answers 200; no `-shm` file beside the database |
| `cairn migrate` while that server runs | refused: "another process has it open" |

## Under simulated faults

The durability testbed's six legs pass on SQLite (Ubuntu 24.04, patina at its pin): 128
campaign generations, 112 crash-restarts that all recover, and no invariant broken under
crashes with disk errors. Byte tearing now leaves part of a log frame, and recovery
discards it, which on Turso never happened.

## Under load (Apple M4, release build, the same workloads on both)

| | Turso | SQLite |
|---|---|---|
| Journey loads a second, 1 / 8 readers | 2,250 / 8,800 | 6,900 / 21,700 |
| Commits a second, 1 writer / 8 writers on their own journeys | 800 / 2,450 | 2,420 / 2,480 |
| p99 commit, 8 writers on their own journeys | 31-35 ms | 6.6-7.2 ms |
| 8 readers beside 8 writers | "database is locked" | 14,400 loads and 830 commits a second |

## The build, before and after (Apple M4, 10 cores)

| | Turso | SQLite |
|---|---|---|
| Cold `cargo test --workspace --no-run` (wall / CPU) | 167 s / 1,194 s | 123 s / 998 s |
| Crates compiled cold | 419 | 334 |
| Target directory after it | 5.4 GB | 4.8 GB |
| Rebuild after touching the store trait crate / the backend | 10.2 s / 5.3 s | 8.8 s / 3.9 s |
| `cargo test --workspace` (wall / CPU) | 30.4 s / 117 s | 23.5 s / 98 s |
| Packages in `Cargo.lock` (durability testbed's) | 502 (303) | 406 (89) |
| Backend source, Rust + SQL lines | 6,887 + 898 | 5,265 + 561 |

## Known limits

- A commit waits for every commit in flight, not only those to its domain: more writers
  add no throughput, and a wait past the connection acquire limit answers 503.
- Any write fault during a commit fails the store closed (SQLite writes the log at
  `COMMIT`): under simulated disk errors 14 of 112 runs finish, against Turso's 77.
- No online backup: the server's lock keeps other SQLite tools out while it runs.
