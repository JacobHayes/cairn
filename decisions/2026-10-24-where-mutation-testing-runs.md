# Mutation testing is not run; the dependency sensors run in lint

Question: where do cargo-mutants, StrykerJS, cargo-machete and knip run?

Call: cargo-machete and knip run in lint on every run: they take seconds and their findings hold up. Mutation testing runs nowhere: it finds missing tests but not bugs, at about ten CI hours a week. A change that deletes tests is judged by the cull's questions instead.

Alternatives: running mutation tools weekly, on a slice of the code, or on every test-removing change (high cost, and the yield is tests, not bugs).

What would change it: a bug reaching main in code covered by line but not by assertion, or a mutation tool that runs in minutes a week.
