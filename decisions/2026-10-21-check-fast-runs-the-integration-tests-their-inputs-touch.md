# check:fast runs the integration tests their inputs touch

Question (from the rule-tooling unit; it refines `2026-10-21-ci-on-main-is-the-gate.md`): with CI on main as the gate and no local full check, `check:fast` never ran the integration step, so a change to the service, API or MCP crates could land and turn main red on tests it never ran. Which changes should run the step?

Call: `scripts/changed` prints a fourth flag, integration, and `check:fast` adds the integration step beside the property step when it is 1. The flag is 1 when a changed file is under the directory of a crate the integration tests build or run (`cairn-tests` and `cairn`, with every crate they depend on, read from `cargo metadata`, so a crate that gains or loses a dependency moves in or out with no list), under `fixtures/` or `instructions/` (which those tests read), a workspace manifest or lock file, or the tooling. The advisories and cost steps stay out of the fast check.

Measured on this machine, warm cache, a change touching Rust, web and tooling files: the integration step took 5 s (175 tests) and ran beside the property step, so the landing's wall time did not move (the browser step's 36 s dominates when the web side changed; a Rust-only change adds the 5 s to a run of about 20 s beyond the build). The cold first run is paid by the shared build of the test binaries, which the unit step needs anyway.

Alternatives: run integration on every fast check (adds 5 s to changes that touch no crate, such as documents); a hand list of crates (drifts); leave it to CI (what just turned main red).

What would change it: the step growing past about 30 s, when it would be limited to the crates whose reverse dependencies the change reaches.
