# CI runs the ladder in parts on separate runners

Question (from the CI speed work): the check ran as one job on one 4-vCPU runner, every step in turn: about 37 minutes from empty caches and 31 to 32 with a warm cache. How should CI run the same steps faster without a step dropping out of it, and with `mise run check` still the one entry point?

Call:
- `scripts/ladder` owns a fixed split of the full check's steps into parts, each the steps that share one build: `lint`; `rust` (unit, property, integration, cost: one test build); `web` (web-unit, generated, embed: the web build and what reads it); `browser` (browser: the web build and the test build, for the binary Playwright drives). `scripts/ladder --part NAME` runs the full check restricted to that part, in the ladder's order, building only what its steps read; `mise run check --part NAME` passes through to it. Without `--part` nothing changes: `mise run check` runs every step on one machine.
- The workflow's matrix comes from `scripts/ladder --parts` (a JSON array, printed by bash alone), read by a small `parts` job, so a part added to the ladder runs in CI with no workflow edit. Every invocation of the ladder asserts that each step of the full check is in exactly one part, that no entry names a step or part that does not exist, and that every part has a step; a violation exits 2 naming it. A step cannot drop out of CI by being added to the ladder and forgotten in the YAML.
- An aggregate job named `check` (`if: always()`, red unless every part passed) is the one status, and what the release jobs wait for. `if: always()` because a job skipped by a failed need reads as success to branch protection; a failed `parts` job skips the matrix and the aggregate still goes red (exercised on a scratch branch).
- `fail-fast: false`: a failing part must not cancel the others mid-build, so they finish their tests and save their caches.
- The nightly job was a copy of the check job that differed only in `PROPTEST_CASES`; the scheduled run now runs the same matrix with 4096 cases, and the copy is deleted.
- `scripts/ladder` prepares (the web build, then the test binaries) before the first step that is not lint, no longer only after lint, so a part without lint builds; and `embed` now asks for the web build itself, since in its own part no browser step sits beside it.

Measured (scratch branches, same source, runners vary by about 25% between runs of one tree; full table in the commit body):
- Before, one job: 37 minutes from empty caches, 31 to 32 warm. After, four parts side by side: about 18 minutes cold, 13 to 18 warm. The longest part is browser in most warm runs (ladder 675 to 1031 s against rust's 696 to 780 s): both do the same test build, and browser first does the web build with the optimized wasm module (100 to 170 s).
- Warm, the rust test build is still 540 to 740 s of the part: the dependencies come from the cache, but rust-cache drops the workspace's own crates (Cargo compares mtimes, which a fresh checkout resets), so every run recompiles all of them at opt-level 2 and links the test binaries. The slowest units are the `cairn-tests` and `cairn` integration test binaries and the `cairn` binary (230 to 290 s each, link time not isolated by `--timings`) behind the `cairn-schema` and `cairn-engine` library builds.

Alternatives:
- One job, as before: four cores for about three builds and four side-by-side steps.
- Build once and hand the artifacts to the other parts: the browser part needs only the `cairn` binary and the `cairn` crate's test binary, so it could skip its own 540 to 740 s test build, but it would wait for the rust part's whole build plus a transfer, and `scripts/e2e-servers` and `scripts/ladder-tests` would need a no-cargo mode that local runs never use.
- A job per step: unit, property, integration and cost would each build every test binary.
- Shard the Playwright tests: the browser step itself is 65 to 95 s warm; the build before it is the cost.
- A static `matrix.part` list in the YAML: it needs the list twice, and nothing could compare it with the ladder's.

What would change it: the next saving is in the build, not in the split. The browser part is the longest by about four minutes in most warm runs, past the three-minute mark this work set for the next step, and it was not built here: hand the rust part's test binaries to the browser part by artifact (the second alternative; the browser part would still run its web build beside the wait), or cache the workspace crates' artifacts with their source mtimes restored, which would shorten every part. A step that needs a build of its own gets a part of its own in `scripts/ladder`.
