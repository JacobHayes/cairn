# The check covers the testbeds' build, lint and unit tests; the campaigns stay nightly

Question: the nightly simulation job runs only on schedule, so a change on main can break a testbed unseen. Which of its legs belong in the check?

Call: the cheap, deterministic legs: the testbeds' formatting and clippy (which compiles every target, so a service change that breaks a testbed's build fails lint) and their unit tests run in the ladder, in `check:fast` whenever a Rust input changes. What needs `cargo patina` stays nightly: the shim-linked build and audit, the campaigns, and the coverage gate that fails on a declared site no run reached. A new site only a second writer can reach must be listed as out of reach in each single-client testbed.

Alternatives: a short campaign in the check (cargo-patina on every runner and minutes per push); statically cross-checking oracle labels against each testbed's out-of-reach list (it cannot tell what a testbed reaches, so it only restates the list).

What would change it: a campaign leg that runs in seconds, or a nightly break a cheap step could have caught.
