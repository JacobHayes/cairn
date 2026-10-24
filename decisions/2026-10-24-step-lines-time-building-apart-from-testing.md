# Step lines time building apart from testing

Question: a step's line gave one number, so web-unit's 128 s and lint's 206 s said nothing about how much of it was compiling and how much checking or testing, and a time limit set on that number would compare a cold build with a slow test. How does each line separate the two? (From the step-timers unit.)

## The call

- **Each span is timed where it runs, never by subtraction.** `scripts/timed build|tests COMMAND...` wraps the command, takes its own wall clock and appends one span to the step's times file (`LADDER_TIMES`, one per step, set by `scripts/ladder-lib.sh` and inherited by every script and mise task a step starts). `scripts/ladder-tests` records its build and its parallel test run itself. With no step collecting, `scripts/timed` only runs the command, so the wrapped scripts work alone.
- **Every compile site is wrapped:** the web and wasm builds (inside `build:web` and `build:wasm`, so each caller gets them), the generators' builds, the agreement cases' build, the testbeds' test builds (`cargo test --no-run`, then the run), embed, clippy and the wasm32 check in lint, and the builds inside `scripts/e2e-servers`. The prepare phase keeps its own `build:` line.
- **A kind counts the wall time something of that kind was running.** Builds that overlap (the generators side by side) count once; a build beside a test run (the demo site's build inside Playwright's run, the agreement cases beside Vitest) is reported on its own and also lies inside the test run, as the span it was.
- **The line is** `web-unit: passed, 1430 tests, 38s tests, 90s build, 140s in all`. The whole time stays at the end because lint and advisories do work that is neither (eslint, the typecheck, the network), and a budget needs the sum.
- **Doc tests count as tests.** rustdoc compiles each one as it runs it, so there is no build to separate.
- **The cache signal** says exact, partial or none, from data the workflow has. rust-cache (v2.9.2, read at the pinned commit) keys on the lock files, manifests and toolchain and caches dependencies only, so our own crates compile in every case. Its `cache-hit` output is true only for an exact key match; a restore from the fallback key sets it false and leaves the dependencies' artifacts in `target/`, and nothing restored leaves `target/` empty. A `check.yml` step reads both and passes `exact`, `partial` or `none` as `LADDER_CACHE_STATE`. The `build:` line words it as "exact dependency cache", "partial dependency cache, restored from an older key" or "no dependency cache"; lint, which has no `build:` line, carries it on the ladder's summary line. The web build has no cache to report.

## Alternatives

- Subtracting the test time from the step's time to get the build: rejected, the build overlaps the tests in web-unit and browser, so the difference is neither.
- Summing overlapping spans: rejected, ten builds side by side would read as ten times the wait.
- Parsing cargo's own timings: rejected, it covers cargo only, not the web build, wasm-bindgen or the generators.
- A third kind for lint's checks: rejected for now, the whole time beside the build time shows them.

## What would change it

A step budget that needs more than two kinds, or a compile site whose time cannot be wrapped where it runs.
