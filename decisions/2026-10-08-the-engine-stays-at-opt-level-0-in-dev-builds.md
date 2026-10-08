# The engine stays at opt-level 0 in dev builds

- Question: should the dev profile, which tests and the check build with, compile `cairn-engine` at opt-level 1 or 2, so its tests (the cost tests at the limits, the property tests) run faster?
- Measured (one host, other agents' builds alongside, so a few seconds of noise either way), each setting in its own fresh target directory:

  | engine opt-level | warm `mise run check`, nothing changed | engine edit, then `mise run check` | engine edit, then `cargo test -p cairn-engine` (build) | target size |
  | --- | --- | --- | --- | --- |
  | 0 (now) | 119-145 s | 151 s | 41 s (8 s) | 5.5 GiB |
  | 1 | 95-129 s | 191 s | 39-40 s (20-21 s) | 5.5 GiB |
  | 2 | 91 s | not run | 50-51 s (43 s) | 5.5 GiB |

- Call: keep opt-level 0. Optimizing the engine shortens rung 3 and the engine's own tests, so a check with nothing to rebuild gains up to about 25 s, but every change to the engine then compiles it, and everything built on it, optimized: an engine edit followed by a check took 40 s longer at opt-level 1, and the engine's edit-and-test loop broke even at 1 and lost 10 s at 2. The engine is where most changes land, and disk is unchanged, so neither level clearly wins. Debug assertions and overflow checks would have stayed on at either level.
- Alternatives: optimizing only the dependencies (already cached by the build cache, and the engine's time is its own code); opt-level 1 for the test profile only (the same rebuild cost, since the check's tests and programs share one build).
- What would change it: a check dominated by engine tests that run at the limits, or an incremental compiler that makes optimized engine rebuilds cheap.
