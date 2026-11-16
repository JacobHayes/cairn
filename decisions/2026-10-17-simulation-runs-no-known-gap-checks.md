# Simulation runs check no known dependency gap

- Question: should the sim step keep a sweep that re-proves a known gap in a pinned dependency (patina or the store's database library)?
- Call: no. Patina and the database library are pinned (`mise.toml`, `Cargo.lock`), and such a sweep exercises only the dependency, never Cairn's product code. On the same binaries it gives the same result every time and can change only on a version bump. A gap in a dependency is recorded where its owner reads it (a patina issue, or a `[patina]` file in `decisions/`); whoever bumps the dependency reads those and retests by hand. A test that injects a fault and asserts the dependency still misbehaves tests no Cairn behaviour and does not belong in the store's cases either.
- Alternatives: keep such sweeps as a bump alarm (a cost on every check and a failing sweep that means "good news, update the notes", for a bump that is a deliberate act anyway); move the reproducers to a bump-time task (still code to maintain for something checked a few times a year).
- What would change it: a gap that Cairn's own code reaches, which belongs in a test of that code, not in a sweep about the dependency.
