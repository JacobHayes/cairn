# Proof for brief 1.1: Workspace, lints, tasks, CI

`mise run check` now runs every rung of the validation ladder that exists, in order, stops at
the first failure, and ends with a summary. A clean tree passes; each kind of rule break stops
the ladder at rung 1, before any test runs.

## What the ladder refuses

Each row is one change planted in a copy of the tree, and what `mise run check` did with it.

| Planted change | Outcome |
|---|---|
| None (the clean tree) | passes every rung |
| A function ending in `text.parse().unwrap()` | rung 1 fails: `unwrap()` on a `Result` |
| A function body of 70 lines | passes (70 is the limit) |
| The same body at 71 lines | rung 1 fails: too many lines (71/70) |
| `pub const   SPACED : u32=1;` | rung 1 fails: rustfmt shows the diff |
| A `std::collections::HashMap` in the engine crate | rung 1 fails: disallowed type |
| A crate without `[lints] workspace = true` | rung 1 fails, naming the crate and its manifest |
| `node` missing from the machine | rung 1 fails: tool missing |
| A rung whose suite ran zero tests | that rung fails: a suite checked 0 tests |

## Known limits

- These outcomes come from the lint configuration and `scripts/ladder-lib.sh`; nothing in the
  ladder re-runs these planted changes, so a regression in the ladder itself is caught only by
  review.
