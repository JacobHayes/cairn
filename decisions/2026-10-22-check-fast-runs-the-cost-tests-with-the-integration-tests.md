# check:fast runs the cost tests with the integration tests

Question (from making `check:fast` catch a broken cost bound): the cost tests ran only in the full check, so a generator or engine change that broke a cost bound was found by CI on main (it happened once). Should the inner loop run them?

Call: yes, whenever the integration step runs. The cost tests are modules of the integration test binary (`cairn-tests`), already built for the unit step, and they read the engine, the schema and the generators in `cairn-engine`: all within the crates the integration flag of `scripts/changed` already derives from `cargo metadata` (with the fixtures and tooling). So `scripts/ladder` adds the cost step beside integration, with no new input list and no new flag. A web-only or document-only change runs neither. The integration flag also fires for a change only to the binary crate's own dependencies, which the cost tests do not read; that adds about 3 s to such a run, accepted over keeping a second list that could drift. The cost step stays its own, with its own count.

Alternatives: a fifth flag for the crates the cost tests alone build (a second derivation to keep in step); running cost on every fast check (adds about 3 s to changes that touch no crate); leaving it to CI (what turned main red).

What would change it: the cost step growing past about 30 s, when it would be limited to the engine's reverse dependencies.
