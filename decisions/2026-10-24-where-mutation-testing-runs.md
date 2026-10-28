# Mutation testing is not run; the dependency sensors run in lint

Question: from the work of measuring four tools (cargo-mutants, StrykerJS, cargo-machete, knip), where does each run? The owner later asked whether the two mutation tools earned their weekly CI time.

## Call

| Tool | Runs |
|---|---|
| cargo-machete | lint, every run |
| knip | lint, every run |
| StrykerJS | nowhere |
| cargo-mutants | nowhere |

Mutation testing is not run. Two weekly runs found missing tests but no bugs, at about ten CI hours a week. The gaps they exposed were closed with tests; the tools, their tasks, their workflow and their configuration were deleted. A change that deletes tests is judged by the cull's own questions.

Machete and knip take under two seconds, find real things, and have findings that hold up, so they stay in lint.

## Alternatives

Running the tools weekly (the earlier call), on a slice of the code, or on every test-removing change was dropped: the cost was high and the yield was tests, not bugs.

## What would change it

A bug that reached main in code the tests covered by line but not by assertion, or a much cheaper way to run a mutation tool (minutes a week rather than hours), would reopen it.
