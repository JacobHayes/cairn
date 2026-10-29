# Testbed invariants stay `violation` verdicts, not `always!`

- Question: `always!` now leaves a trace to replay (`2d1c8d3f`). Should the testbeds convert their invariants to `always!` sites (patina-bump)?
- Call: no. `always!` aborts at the first broken invariant, while the multiplayer judge reports every broken invariant of a run (`outcome.rs`) and durability reports every broken invariant of the audit that failed (it stops at that audit). The campaign already classifies, dedupes and minimizes on the first violation verdict, and a probe minimized one. Converting would add code for no new detection. The cost of staying on the exit path is the open Patina ticket "minimize accepts a replay that diverges on an ordinary exit"; a minimized failure is confirmed anyway by rewriting it as a deterministic test, as PRACTICES requires.
- Alternatives: literal `always!` sites for each invariant (per-invariant evaluation counts, but first-failure-only reports and new code for it).
- What would change it: a minimized verdict failure that misleads while that ticket is open, or a need for per-invariant evaluation counts.
