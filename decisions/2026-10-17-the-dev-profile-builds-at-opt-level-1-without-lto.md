# The dev profile builds at opt-level 1 without LTO, and the test crate at 0

Question (build speed): the library crates' integration tests (`cairn-tests`, 118 s), `cairn`'s and `cairn-schema`'s took minutes to build. A probe found linking is about 1% of that (lld, 0.3-0.8 s; mold is no faster); the time is LLVM re-instantiating generics (serde for every schema type, the server stack generic over `Store`) in each final binary. rustc stops sharing generics between crates at opt-level 2, and Cargo's default local thin LTO adds about a third of the CPU. Which dev profile is fastest end to end?

Measured (plain cargo, non-incremental like mbx, fresh target directory each, the 12-core box shared with other agents at load 10-22; wall seconds. "tests edit" is one line in cairn-tests, "engine edit" one line in cairn-engine, both rebuilding all test binaries; "suite" is the cairn-tests binary run alone, 696 tests):

| profile | fresh build | tests edit | engine edit | suite (CPU s) | edit + suite |
| --- | --- | --- | --- | --- | --- |
| now (o2) | 221-266 | 66 | 150 | 12 s wall (58) | 77 / 162 |
| (a) o2, cairn-tests o0, no LTO | 194 | 20 | 64 | 12-14 s (76) | 32 / 76 |
| (b) o1, no LTO | 115 | 46 | 71 | 21-22 s (83) | 67 / 92 |
| (hybrid) o1, no LTO, cairn-tests o0 | 103 | 15 | 44 | 16-20 s (83) | 32 / 61 |
| (c) o0 everywhere | 90 | 17 | 38 | 58-79 s (333) | 90 / 110 |
| (d) o0 workspace, dependencies o2 | 161 | 15 | 34 | 48-58 s (260) | 66 / 85 |

Call: the hybrid. `[profile.dev]` is opt-level 1 with `lto = "off"`, and `cairn-tests` alone is opt-level 0. o0 builds fastest but runs the 696 tests 5 times slower (58-79 s wall against 12 s; the engine property and cost tests are the ones the 2026-10-09 call already measured at 16 s against 3 s), which loses the moment a check runs them; (d) is no better, and its optimized dependencies make a fresh build 70 s slower. The hybrid is fastest for an edit plus its tests and for a fresh build, and it keeps the suite near 17 s. Debug assertions and overflow checks stay on. `wasm-dev` already set opt-level 1 and the release profiles are untouched, so the browser module and the shipped binary do not change.

Alternatives: the five rows above; splitting cairn-tests (doubles CPU: each piece re-instantiates the same generics); a faster linker (the link is 1%); `-Zshare-generics` through RUSTC_BOOTSTRAP (unstable flag on stable); making the server stack non-generic at the crate boundary (an ARCHITECTURE-level change to `Store`, the durable fix for the `cairn` binary and its tests, not done here).

What would change it: a test that passes at opt-level 1 and fails at 0 or 2 (an overflow or ordering bug; debug assertions stay on), or a non-generic server stack that makes opt-level 2 cheap again.
