# The dev profile builds at opt-level 1 without LTO, and the test crate at 0

Question: almost all of an integration test binary's build is LLVM re-instantiating generics in each final binary, not linking. Which dev profile is fastest end to end?

Call: `[profile.dev]` is opt-level 1 with `lto = "off"`, and `cairn-tests` alone is opt-level 0. Against the alternatives, this is fastest for a fresh build and for an edit plus its tests. Opt-level 0 everywhere builds faster but runs the suite about five times slower, which loses as soon as a check runs it. Debug assertions and overflow checks stay on; the release and wasm profiles are separate.

Alternatives: opt-level 2 (rustc stops sharing generics between crates); opt-level 0 for the workspace with optimized dependencies (slower fresh builds, no faster suite); a faster linker (linking is about 1%); splitting the test crate (each piece re-instantiates the same generics).

What would change it: a bug that shows only at another opt-level, or a measured build where opt-level 2 costs no more.
