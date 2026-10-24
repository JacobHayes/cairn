# Step lines time building apart from testing

Question: a step's line gave one number, so web-unit's 128 s and lint's 206 s said nothing about how much of it was compiling and how much checking or testing, and a time limit set on that number would compare a cold build with a slow test. How does each line separate the two? (From the step-timers unit.)

## The call

- **Each span is timed where it runs, never by subtraction.** `scripts/timed build|tests COMMAND...` wraps the command, takes its own wall clock and appends one span to the step's times file (`LADDER_TIMES`, one per step, set by `scripts/ladder-lib.sh` and inherited by every script and mise task a step starts). `scripts/ladder-tests` records its build and its parallel test run itself. With no step collecting, `scripts/timed` only runs the command, so the wrapped scripts work alone.
- **Every compile site is wrapped:** the web and wasm builds (inside `build:web` and `build:wasm`, so each caller gets them), the generators' builds, the agreement cases' build, the testbeds' test builds (`cargo test --no-run`, then the run), embed, clippy and the wasm32 check in lint, and the builds inside `scripts/e2e-servers`. The prepare phase keeps its own `build:` line.
- **A kind counts the wall time something of that kind was running.** Builds that overlap (the generators side by side) count once; a build beside a test run (the demo site's build inside Playwright's run, the agreement cases beside Vitest) is reported on its own and also lies inside the test run, as the span it was.
- **The line is** `web-unit: passed, 1430 tests, 38s tests, 90s build, 140s in all`. The whole time stays at the end because lint and advisories do work that is neither (eslint, the typecheck, the network), and a budget needs the sum.
- **Doc tests count as tests.** rustdoc compiles each one as it runs it, so there is no build to separate.
- **The cache signal** is rust-cache's `cache-hit` output, passed by `check.yml` as `LADDER_CACHE_HIT`. The `build:` line words it as what it is, "exact dependency-cache hit" or "no exact dependency-cache hit"; lint, which has no `build:` line, carries it on the ladder's summary line.

## Alternatives

- Subtracting the test time from the step's time to get the build: rejected, the build overlaps the tests in web-unit and browser, so the difference is neither.
- Summing overlapping spans: rejected, ten builds side by side would read as ten times the wait.
- Parsing cargo's own timings: rejected, it covers cargo only, not the web build, wasm-bindgen or the generators.
- A third kind for lint's checks: rejected for now, the whole time beside the build time shows them.

## What would change it

A step budget that needs more than two kinds, or a compile site whose time cannot be wrapped where it runs.
