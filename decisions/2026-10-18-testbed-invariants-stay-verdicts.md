# Testbed invariants stay `violation` verdicts, not `always!`

Question: `always!` leaves a trace to replay. Should the testbeds turn their invariants into `always!` sites?

Call: no. `always!` stops at the first broken invariant, while the testbeds' judges report every broken invariant of a run or audit. The campaign classifies, deduplicates and minimizes on the first violation verdict, so converting adds code and no detection. A minimized failure is confirmed by rewriting it as a deterministic test, as for any finding.

Alternatives: one `always!` per invariant (per-invariant counts, but first-failure-only reports).

What would change it: a minimized verdict failure that misleads, or a need for per-invariant evaluation counts.
