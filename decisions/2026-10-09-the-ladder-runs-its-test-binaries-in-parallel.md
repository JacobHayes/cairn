# The ladder builds every Rust test binary once and runs them in parallel

Question: run one after another, with every step paying cargo's start-up, the test binaries leave the check mostly waiting. How can it be faster without changing what it checks?

Call: the ladder builds every test binary once and runs them in parallel, and each Rust step picks its tests out of that one run by name, so suite names and counts are the same as running them step by step. eslint keeps a per-file result cache locally; CI has none.

Alternatives: `cargo-nextest` (several times slower here, from one process per test); a dedicated ladder profile and target directory (a second full build of the tree).

What would change it: a stale eslint result after a change to another file's types (delete the cache), or nextest's process spawns becoming cheap.
