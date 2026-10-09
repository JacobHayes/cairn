# Cost tests run in the full check only

Question (from making the inner loop quiet and fast): the engine's cost tests (`cost_*.rs`, `mod cost`) pass over graphs at the schema's limits, which is structurally slow, and rung 3 ran them with the property tests in every `check:fast`. Should they stay there?

Call: they are their own rung, 7, which only `mise run check` and CI run. Rung 3 is the property tests alone, so `check:fast` after an engine change lost the slowest few seconds of its Rust run (the cost tests were about 4 s of the engine binary's 10 s in rung 3). A property test still catches a regression in what the engine derives; a cost test catches a pass that grew past its operation budget, which a change to one pass rarely does and the full check before landing still catches. The suites keep their names and counts, one per file (`scripts/ladder-lib.sh`, `ladder_file_suites`), and `mise run check:fast cost` runs them on demand, as any filter does.

Alternatives: keeping them in rung 3 and skipping them under `--changed` (a second way to be incomplete, with a flag to remember), a rung 4 suite (rung 4 is store and API tests; the cost tests would hide behind its count), and leaving them (the cost the user paid on every change).

What would change it: a regression the cost tests caught only at the full check and that took a day to find, or cost tests that become fast enough (a smaller limit graph) to cost less than a second.
