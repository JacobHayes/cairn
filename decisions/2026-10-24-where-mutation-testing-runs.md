# Where mutation testing and the dependency sensors run

Question: from the work of measuring four tools (cargo-mutants, StrykerJS, cargo-machete, knip), where does each run: in lint, weekly, on a change that removes tests, or nowhere? The owner asked how long they take, what they catch, and whether any is worth running in-band for every change.

## Call

| Tool | Runs |
|---|---|
| cargo-machete | lint, every run |
| knip | lint, every run |
| StrykerJS | `mise run mutants <files>`, on a change that deletes or merges web unit tests |
| cargo-mutants | nowhere (not added) |

A mutation result is evidence that a cut lost nothing, not proof of it. Nothing runs weekly, so no weekly workflow is needed for these tools.

## Numbers

All runs were on the shared workspace box with other builders running, so wall times are upper bounds; parallelism is noted per row. Scratch logs are under the measuring builder's scratch directory.

| Tool, scope | Wall time | Parallelism | Result |
|---|---|---|---|
| cargo-machete, whole repository (testbeds included) | 0.12 s | one process | 5 unused dependencies in 3 crates |
| knip, defaults | 2.0 s | one process | 7 files, 124 exports, 65 types, 1 package: mostly noise |
| knip, with a 10-line config | 0.8 s | one process | 2 dead files, 9 dead exports, 8 dead types, 1 duplicate export |
| cargo-mutants, `derive/skip.rs` (22 mutants), engine tests in `cairn-tests` | 13 min 21 s | `-j2`, under `nice`, 6 GB peak per run | 15 caught, 6 unviable, 1 missed |
| cargo-mutants, calibration at the cull's parent: `skip.rs` (18) and the merge functions (4) | 25 min 54 s and 14 min 39 s | `-j2`, load average near 20 | 13 caught, 4 unviable, 1 missed; 3 caught, 1 unviable |
| cargo-mutants, calibration at the cull: same scope | 27 min 16 s and 18 min 48 s | same | identical outcomes |
| StrykerJS, `timeline/rows.ts` and `model.ts` (929 mutants) | 3 min 23 s | 4 workers | 514 killed, 32 timeouts, 300 survived, 83 uncovered; score 59% |
| StrykerJS, `timeline/rows.ts` alone (710 mutants) | 31 to 36 s | 4 workers | score 55% (385 killed, 5 timeouts, 243 survived, 77 uncovered) |
| StrykerJS, `rows.ts` with its test file removed | 30 s | 4 workers | score 20% (130 killed, 318 survived, 242 uncovered) |

cargo-mutants costs about 36 s a mutant even with the engine tests alone taking 6 s: every mutant rebuilds the engine and relinks the single `cairn-tests` binary (40 to 140 s), and its builds bypass the shared build cache. At that rate the engine (27,000 lines) is a day or more of machine time.

## What each found

**cargo-machete.** Five real unused dependencies, all confirmed by removing them and building the workspace with all targets and features: `cairn-engine` and `metrics` in the binary crate, `patina-dst` in the API crate, `cairn-engine` and `jiff` in the MCP crate. They are removed in this change. Its `--with-metadata` mode added one false finding (a crate's self dev-dependency that turns on a feature), so lint runs the plain mode, which had none. The testbeds are clean. Fast, accurate, no configuration: it earns a place in lint.

**knip.** Out of the box it is noisy: scripts and mise tasks are entry points it cannot see, the `@design` alias is unresolved to it, and it reports every export used only in its own file. With `ignoreExportsUsedInFile`, one extra entry list, and two ignored names, what remains is real: two files nothing imports (`acting/Parts.tsx`, the `authoring/index.ts` barrel), nine exported functions and constants with no use (`destinationIn`, `journeyDocument`, `MayNotApply`, `levelQueryOf`, `unchanged`, `SEGMENTS_PATH`, `routeLayerEntry`, `Chips`, `Gate`), eight exported types nothing used, and `Field`, an alias of `Input` kept "so code written against it still builds". All are removed in this change, with the CSS rule that only `Chips` used. The config is small and the run takes under a second, so it joins lint.

**cargo-mutants.** Over `skip.rs`, the code behind one of the two behaviours the 10-17 cull lost, 21 of 22 mutants were stopped or unviable and the one survivor is `Skips::effective` returning an empty iterator. Its only caller is a debug assertion that every effective skip is in scope, which holds trivially for an empty list: an equivalent mutant, with a small real note that the assertion is not exercised with data by these tests. It found no gap worth a test.

**StrykerJS.** It found real gaps and a lot of noise, in roughly these shares. Noise: string-literal mutants on switch cases and display text, `<` against `<=` on loop bounds, optional-chaining mutants on values never undefined. Real: `timeline/model.ts` `labelOf` and the `MONTHS` table, the text of every tick label (month, quarter, year, first-tick year) is asserted by no unit test and no browser test, so changing it fails nothing; and the calendar step math (`start`, `index`, `monthsPerStep`) survives arithmetic mutations. The tool's use is comparative: removing a module's test file dropped kills from 385 to 130 in half a minute, so a test-removing change sees what it gave up.

## The replay of the 10-17 cull

The cull dissolved the scenario matrix and lost two behaviours: a cascaded skip that must hold dependents on kept work, and that an entity merge leaves journey references alone. Only a property test still caught the first.

cargo-mutants was run over the code of both, at the cull's parent and at the cull (`skip.rs`; the merge functions of `mutate/deployment.rs`), with the same engine tests. The outcomes were identical in both trees: the same mutants caught, the same one equivalent survivor. It would not have caught the cull. For the first behaviour the property test and other tests still killed every mutant of the skip code, so the lost concrete example added nothing a mutant could see. The second is an absence (the merge must not rewrite journey references): a mutant that deletes or alters code cannot add the missing rewrite, so removing that assertion kills no mutant. "No difference" is the result.

## Why these calls

- Machete and knip take under two seconds, find real things, and have findings that hold up: lint.
- cargo-mutants takes tens of minutes for one file, kills at the rate the engine's tests already show, and could not see the cull's loss. Weekly over the engine is days of machine time for a handful of equivalent survivors, and in-band is out of the question. It stays out of the repository.
- StrykerJS is cheap when scoped to the few modules whose tests a change removes (about a minute for a module of 500 lines), and the kills it reports fall when tests are removed. A weekly whole-app run is about 47,000 lines, an hour or more with a survivor list that is mostly noise, which nobody would act on. So it runs on test-removing web changes only, not on every change and not weekly. Stryker mutates in place because the wasm module the tests read is untracked build output, so it must run in the change's own workspace.

## Alternatives

Stryker weekly across the app, cargo-mutants weekly on a rotating engine area, both rejected for cost against yield. Running Stryker in a copy needs the untracked build output copied and was not worth the cost.

## What would change it

A Rust mutation run under the shared build cache that costs a few seconds a mutant; a test-removing Rust change whose lost behaviour a mutant exposes; or the weekly cull wanting a survivor list it can act on.
