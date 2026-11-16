# Testbed invariants are judged together, not one `always!` each

Question: should the testbeds turn their invariants into `always!` sites?

Call: no. `always!` stops at the first broken invariant, while the testbeds' judges check every invariant of a run or audit and report the broken ones together as one `violation` (`patina_dst::violation`), under the first one's label with each named in the detail. A sweep groups and replays on that one violation, so per-invariant sites add code and no detection. A failure is confirmed by rewriting it as a deterministic test, as for any finding.

Alternatives: one `always!` per invariant (per-invariant counts, but first-failure-only reports).

What would change it: a grouped failure that misleads, or a need for per-invariant evaluation counts.
