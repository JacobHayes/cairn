# [patina] a campaign draws no crash

- Question: can `cargo patina campaign` cover the crash-restart oracles?
- Call: no, at the pinned revision. A campaign never draws `--fs-crash-at` (`campaign.rs`, "not drawn: crash restart is native-only"), and it gates every literal `sometimes!` it links, so the testbed's crash oracles use constant labels that stay out of the link-time table and `sim.sh` leg 5 gates them over an explicit crash sweep instead (every 11th write and sync, both tear granularities, two seeds); the campaign runs `--allow-unmet-sometimes` and leg 4 requires every oracle but a named out-of-reach list.
- Alternatives: a second build without the crash oracles for the campaign; waiving the whole coverage gate.
- What would change it: a patina campaign band for crash placement; then the campaign covers the crash oracles and leg 5 keeps only the sweep.
