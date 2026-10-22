# The check covers the testbeds' build, lint and unit tests; the campaigns stay nightly

Question (from the nightly `sim` job going red on most nights, and again on 2026-10-22): the job runs only on schedule, so a change on main can break a testbed unseen. Which of its legs belong in the ladder?

Call: the legs that are cheap and deterministic are already steps of the ladder and stay there: the testbeds' formatting and clippy (lint, which compiles every target, so a service API change that breaks a testbed's build fails it; decisions/2026-10-18-the-ladder-lints-the-testbeds.md) and their unit tests (unit; decisions/2026-10-21-the-ladder-runs-the-testbeds-unit-tests.md). Both run in `check:fast` whenever `scripts/changed` sees a Rust input, and testbeds/ and every crate they build on are Rust inputs, and in the full check always. Measured warm, the two add about 3 s of tests and the testbed clippy runs. What stays nightly is what needs `cargo patina`: the shim-linked build and audit, the campaigns, and the coverage gate (a declared `sometimes!` or `reachable!` that no run fired). The 2026-10-22 failure was of that last kind: the service gained a `reachable!` site only a second writer can reach, and the durability testbed, which has one client, did not list it as out of reach. It is listed now, with the other two-writer sites.

Alternatives:
- Run a short campaign in the check: needs cargo-patina in every runner and minutes per push, which is the cost PRACTICES keeps campaigns out for.
- Cross-check every production oracle label against each testbed's out-of-reach list statically: it cannot tell a label a testbed reaches from one it does not, so it would only restate the list.

What would change it: a campaign leg that runs in seconds, or a second kind of break the nightly finds that a cheap step could.
