# [patina] an `always!` violation leaves no trace to replay

- Question: can a failing seed found through `always!` be replayed and minimized, as PRACTICES' honesty rules require?
- Call: no, at the pinned revision. `always!` reports its violation verdict and aborts the process before the native recorder finalizes, leaving `trace=incomplete ... empty trace file; record finalization did not complete`, and `cargo patina minimize --generation N` then fails even with `--no-trace-phase` (`failed to load the trace recorded from the reduced knobs`). Testbeds report invariants through `verdict` and a nonzero exit, as the spike did; with that, the planted bug's failing generation minimized to one knob. Reproducer: `testbeds/multiplayer/gaps` (`always_abort`); `sim.sh` fails once it stops reproducing.
- Alternatives: keeping `always!` (failures found but not replayable); a verdict before each `always!` (the abort still loses the trace).
- What would change it: a patina revision that finalizes the trace on an `always!` abort.
