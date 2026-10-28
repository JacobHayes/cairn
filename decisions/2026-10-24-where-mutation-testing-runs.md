# Where mutation testing and the dependency sensors run

Question: from the work of measuring four tools (cargo-mutants, StrykerJS, cargo-machete, knip), where does each run: in lint, weekly, on a change that removes tests, or nowhere? The owner asked how long they take, what they catch, and whether any is worth running in-band for every change.

## Call

| Tool | Runs |
|---|---|
| cargo-machete | lint, every run |
| knip | lint, every run |
| StrykerJS | weekly in CI, over the whole web app in twelve parallel shards, with the noisy mutators off and static mutants skipped |
| cargo-mutants | weekly in CI, over one sixteenth of the engine (a stretch of consecutive mutants, the week's chosen by ISO week), under a time cap |

Mutation testing runs weekly and feeds the weekly cleanup: the job (`.github/workflows/mutants.yml`) uploads each tool's survivors as a short list, and the cleanup reads them next to the test evidence when it decides what to cull or strengthen. Builders do not run it: a change that deletes tests is judged by the cull's own questions, not by a before-and-after score. A mutation result is evidence that a cut lost nothing, not proof of it.

Both tools are tuned to be read, not gated. Stryker leaves out string-literal and optional-chaining mutants, the two kinds that were mostly noise in the measurements below, and static mutants (top-level code that runs once; 9% of the app's mutants, but 92% of the run time because each runs every test). Without that the first weekly run did not finish in 90 minutes; with it the whole app takes about 20 minutes on the shared box.

One job over the whole app did not fit GitHub's runners: with 46,588 mutants over 313 files it was cancelled at its 45-minute limit, and Stryker writes its report only at the end, so a cut-off run left nothing. The web half is now twelve parallel jobs (`mise run mutants:web --shard I/12`), each mutating every twelfth file of the sorted list and uploading its own survivors file (`web-survivors-1` to `web-survivors-12`; the cleanup reads them together). Locally, under `nice -n 19`, eight shards of the app took 1.7 to 4.0 minutes each (about 21 minutes in all, the slowest 4.0, with 4,500 to 6,800 mutants each); twelve shards leave the slowest near 3 minutes, so even if the runners are five times slower, a shard stays far inside the limit with more than twice the headroom. CI also turns on Stryker's append-only progress reporter, so a log shows how far a run got. cargo-mutants runs the engine's tests under nextest, which stops at the first failing test, so a mutant the tests catch costs one test instead of the whole suite (15.7 s a mutant against 29.1 s in the same sample), and its build runs without the shared build cache's wrapper, which switched incremental compilation off. Survivors are listed with a file and line, what was mutated, and the tool's own id (`scripts/mutation-survivors`).

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

cargo-mutants cost about 36 to 59 s a mutant in those runs, with the engine tests alone taking 6 s: every mutant rebuilt the engine and relinked the single `cairn-tests` binary, with incremental compilation off (the shared build cache's wrapper forces it) and in a copied tree. At that rate the engine (27,000 lines, about 3,500 mutants) is a day or more of machine time.

Speeding it up, measured on 15 mutants of `derive/blocking.rs` (identical outcomes in each: 11 caught, 4 missed) on the shared box, one mutant at a time:

| Setup | Seconds a mutant |
|---|---|
| copied tree, plain `cargo test`, incremental on | 29.1 |
| the same with nextest (stops at the first failing test) | 15.7 |
| nextest, with opt-level 0 and no debug info | 22.6 (the tests run slower than the build saves) |

Running in place gave the same time as the copy, and lld is already Rust's default linker here, so neither is used beyond placing the run where the build cache can restore dependencies. Nextest is kept; the lighter profile is not.

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
- cargo-mutants at about 16 s a mutant cannot cover the engine in one run, but a slice a week covers it in 16 weeks, and a survivor list is cheap to read. In-band is out of the question. A cargo-mutants run could not have seen the 10-17 cull's loss, so the list is a lead for the cleanup, not a check on a change.
- StrykerJS is cheap per module (about a minute for 500 lines), and the kills it reports fall when tests are removed. Whole-app weekly is about 48,000 lines; with string-literal and optional-chaining mutants off the list is shorter, and the cleanup, not a builder, wades through it. Stryker mutates a sandbox copy of the app, with the untracked wasm module and the design tokens linked in where the tests look, so no mutated file reaches the checkout.

## Alternatives

Stryker on every test-removing web change (the earlier call: it costs a builder minutes and only compares scores) was dropped for the weekly list. A copy of the tree for cargo-mutants, a lighter build profile, and a different linker were measured and left out above.

## What would change it

A survivor list the cleanup cannot act on (mostly equivalent mutants), or a job that outgrows its time cap.
