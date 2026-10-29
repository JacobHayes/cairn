# Cairn: Engineering practices

How code is written and verified. Companion to `ARCHITECTURE.md`.

## Summary

1. **Assert everything you believe, bound everything you allocate.** Assertions stay on in release; every count, size, depth, and duration has a named limit.
2. **Programmer error panics, user error returns.** An invalid patch is a `Rejection` listing every violation (A15); a broken code invariant is a bug.
3. **A check either fails loudly or proves it ran.** A simulation with no fired fault sites, a test suite that ran nothing, or a missing tool is a failure.

## Terms

| Term | Definition |
|---|---|
| **Invariant** | A property that holds between every pair of operations. Graph invariants are in the PRD; code invariants are assertions. |
| **Limit** | A named, hard-coded maximum. Not configuration. |
| **Rejection** | The engine's value-typed answer to an invalid patch: every violation, by path. |
| **Assertion** | A runtime check of a code invariant (`assert!`), on in every build. |
| **Oracle** | What decides whether a simulated run passed: `always!`, `sometimes!`, a `verdict`, or a replay comparison. |
| **Fault site** | A `buggify!` point where simulation may take a rare path. Inert outside simulation. |
| **Seed** | The single input a deterministic run is a function of. |
| **Testbed** | A small program assembling Cairn crates into a simulated world with actors and invariants. |
| **Step** | One named check of the validation ladder. |

## Rules

### Bounds and assertions

- **Explicit limits.** Named constants in one `limits.rs` per crate, used by validation, loops, and tests. A limit hit in practice is a design signal, not a knob. Cost and test at the limits, not typical size.
- **Assertions, positive and negative space, on in release.** Assert arguments, returns, and invariants; engine functions average at least two. After `apply`, assert the revision advanced by exactly one and events equal mutations (J2), and re-verify the whole graph's invariants (on in release). Relationships between constants are `const` assertions.
- **Bounded load.** Bound concurrency and queues; background work runs on fixed intervals, not per event (SSE ticks coalesce per domain).
- **No recursion.** Every traversal is iterative with an explicit stack bounded by `containment_depth_max` (wasm stacks are small). Engine tests run at the depth limit in the wasm build.
- **Fixed-width integers.** Counts, offsets, weights, revisions are `u32`/`i32` (`i64` where day arithmetic multiplies). `usize` only at slice boundaries, never serialized.

### Code shape

- **Functions of at most 70 lines**, split by phase.
- **Big-endian names, no abbreviations, units in the name**: `node_count_max`, `offset_days`, `commit_duration_ms`. Only the PRD's abbreviations (`id`, `key`, `api`, `mcp`).
- **Declare near use.**
- **Dimensionality.** Narrow signatures so callers branch less. Return the fewest true outcomes (`()` over `bool` over a value over `Option` over `Result`); take the narrowest input (a `Key`, not `Option<Key>`). The PRD's closed sets are enums; engine `match`es are exhaustive with no wildcard.
- **Logical interfaces.** Clock, timezone, and viewer are derive inputs, never read by the engine. I/O, retries, and time live in the shell; each seam declares its fault model.

### Scope and dependencies

- **No configuration by default.** A knob exists only when two real deployments need different values. The surface: database path, listen address, public base URL, auth providers and their security settings (the Tailscale trusted proxies, the dev provider's off-loopback override and verified emails), assistant provider and credential, deployment timezone, rank constants. Limits and behavior switches are not configuration.
- **Do it right while it is hot.** No `TODO` without an owner and reason on the same line; deferred work lives only in the PRD's Later.
- **Back-of-the-envelope first.** A new pass over the graph states its cost at `node_count_max` (compute, memory, storage or network) before code, and comes with a property test.
- **Dependencies are deliberate, not zero.** The engine takes only `serde` and a date type, nothing touching the OS or a runtime, so it compiles to wasm and simulates. Shell crates take what they need when maintaining our own would cost more than auditing upgrades; the adding commit says why. Tools in `mise.toml` are pinned and earn their place the same way.

### Simplicity

Do as much as necessary and as little as possible; every layer, script, flag, branch, and rule pays for itself or goes.

- **A fast loop is a feature; question a layer before optimizing it.** Claims about time or cost cite a measurement. A slow step first asks whether it should exist.
- **Fail closed, recover from durable state.** When in-memory state is in doubt, refuse or crash, and reload from what was committed; never track suspect rows.
- **Migrate old data once.** A one-off migration, then one code path; no loader accepting both shapes.
- **No compatibility cruft before anyone depends on it.** Rename and cut over in one change; no alias, fallback field, or shim without a named consumer that cannot move.
- **Judge what exists by what it is for, and cull what no longer earns its place.** Work in an area deletes the code and tests there that no longer pay. Keeping something takes the case adding it would: what it is for, what it catches or does that nothing else does, what it costs. Existing, effort spent, or a check running it are not reasons; for later work built on it, weigh the cost of moving that work. Before crediting a test with catching a break, check the break was not intended.
- **Build the end state, removing first.** Delete what the change makes redundant, then add what is missing.
- **Fix the class, not the instance**, in the same change.
- **Fix it where it lives.** Report a tool's bug upstream with a reproduction; a workaround that stays names what it waits on.
- **Plain and portable over clever**, until a measurement says otherwise.

### Frontend state

How `web/app` and `web/client` hold state.

- **Machines for lifecycles.** An owned, mutually exclusive workflow mode (deciding which async work exists, when it expires, or which events proceed meanwhile) is an XState v5 machine, however small: a write, a read, the subscription, an open form, a toast, a receipt, the sync chip's popover, an assistant conversation.
- **Pure functions for derived values**, never stored beside the source (the sync chip's state is `summarize`, not a machine).
- **Plain data for payloads and anything stored**, never snapshots.
- **One owner per fact.** Components read through tagged selectors (`writeView`), never copy a snapshot into React state.
- **Effects belong to the machine.** Timers are delayed transitions, requests invoked actors; a busy state refuses a new start. Cancelling stops waiting, never undoes a commit.
- **Owners.** Shared actors are the session root's children; components own transient actors (`useActorRef`) and read shared ones with `useSelector`. Managers keep explicit keyed maps, stop what they spawn, and never use XState's system registry. Besides preferences (`useAxis`, `useFrame`) and browser sources (`useMedia`, network, theme): no hand-written stores with listeners, no flag or counter standing in for a state (one that stays says why where it is).
- **Testing actors**: controlled promises and a `SimulatedClock`; bounded workflows also walk every state with `@xstate/graph`; Playwright covers focus, navigation, reload recovery, multiple tabs, worker wiring.

### Not adopted

From Tiger Style and the Power of Ten: static memory allocation, zero-copy data plane, line length 100 (rustfmt's default instead), `else if`/compound-condition rules. **No dynamic dispatch** is not adopted either: the store, notifier, and auth provider are trait objects, because the seams are the point; async traits use `async-trait` as the one object-safe idiom.

## Errors, panics, and rejections

- **User and agent errors are values**: `Rejection` or `thiserror` enums, serialized unchanged through the API and MCP. Never panics.
- **Programmer errors panic** (`assert!`, `unreachable!`). The server's service layer catches an engine panic inside a request: nothing commits, the request fails 500 with its request id, the panic is logged with domain and request id, and the engine panic metric increments. A panic anywhere else in the server aborts the process, since shared state may be inconsistent. In the browser any panic aborts the wasm module.
- **No `unwrap` or `expect` outside tests**; use an assertion with a message.
- **`#[must_use]`** on every result-bearing engine function.
- **Rejections are complete**: validation collects every violation (A15).

### `assert!` versus `always!`

`assert!` is the correctness check, on everywhere. `patina_dst::always!` is a simulation oracle that compiles to `debug_assert!` outside `patina`, so it never replaces an `assert!`; add it beside one only where a labeled verdict helps triage. Testbeds report broken invariants with `verdict` (one per invariant, not aborting at the first). Every site label is a kebab-case string literal (lint checks; campaigns and `sim.sh` grep them).

## Explicit limits

Validation rejects a patch exceeding a limit; loops use the same constant. Every list a person or agent writes has a count limit and every string a byte limit; a value without its own row takes the nearest row that fits. Changing one needs the user's sign-off, then this table and `limits.rs` change together.

| Limit | Value | Why this value |
|---|---|---|
| nodes per graph | 2,000 | 10x a large journey. |
| containment depth | 16 | Far past readable. |
| explicit edges per node, in plus out | 64 | Past this, restructure. |
| mutations per patch | 8,000 | A full import, about four per node; `const`-asserted as 4 x the node limit. |
| roles, participation kinds per graph | 32 each | One participation per kind per node, so this bounds those too. |
| choices per decision | 32 | More is a text or identity answer. |
| checklist items per node | 50 | More are child actions. |
| identities per role fill, identity-list answer, explicit participation, or journey's owners; values per `in` clause | 100 | A stakeholder list, not a team. |
| emails per identity | 16 | Room for a merge. |
| aliases per identity | 32 | One per merged duplicate. |
| identities per deployment | 5,000 | Not a staff directory. |
| journeys named by one identity merge | 1,000 | Every journey referencing either identity. |
| resources per node | 16 | More belongs in one linked document. |
| notes per node, or on the journey | 100 | More belongs in a linked conversation. |
| links per node, or on the journey | 32 | More belongs in a folder behind one link. |
| condition tree depth, clauses | 8, 16 | Fits a form editor. |
| id slug or key, title or label | 64, 256 bytes | Short references; card labels. |
| description, prompt, help, note, tip, message draft, text answer | 64 KiB | More lives in a linked document. |
| link (URL) | 4 KiB | Long signed links, under the 8 KiB request line. |
| email address | 254 bytes | RFC 5321. |
| reason for a skip, override, or bypass | 4 KiB | Longer belongs in a note. |
| date offset, estimate | 365 days | Longer is a modeling error. |
| weight | 1,000 | Sums over every node fit a `u32`. |
| history page | 200 per request | Callers page. |
| tool list, one tool's definition apart from the mutation sketch, the mutation sketch | 48 KiB, 4 KiB, 14 KiB | Compact JSON an agent loads on connecting. A tool that outgrows its share is split or described less. |
| served guide | 4,096 bytes | UTF-8 bytes of `instructions::guide()`; clients show about 4 KB. It keeps what spans tools; a one-tool rule goes in that tool's description. |
| bytes per listed node row, on each fixture journey | 320 | A page of 200 stays near 16,000 tokens. |
| graph, serialized with its state | 16 MiB | One journey, route version or draft, or the deployment record. Events and history are excluded and paged. |
| request body | 24 MiB | The graph cap plus envelope. |
| contradictory chains per rejection | 16 | The rejection says when there were more. |
| explanation entries per value in a server response | 50 | Largest entries plus a total; the rest pages. |
| SSE subscribers per process | 256 | Tens of users, a few tabs each. |
| assistant tool-loop iterations per turn | 32 | A runaway stop, not a budget. |
| request duration, API and MCP (not SSE or the assistant) | 5 s | Past this it is stuck. |
| assistant provider call | 120 s | Past this the provider is down; the turn reports it. |
| assistant turn | 10 min | The iteration limit at typical latency. |
| assistant tool-result bytes kept per turn | 256 KiB | Past this the oldest results are replaced by a note that they were dropped. |
| identical failing tool calls per turn | 3 | The third failure ends the turn. |
| store connection acquire | 5 s | Longer is contention, a bug. |
| outbound sign-in provider call (OIDC, IAP key set, Google token exchange), and a whole request to the IAP relay's endpoints | 10 s | The login fails. |
| requests in flight at the IAP relay's endpoints, per process | 16 | Unauthenticated relay floods leave the rest of the 128 for signed-in requests. |
| IAP key set fetch tries, failed or not | once per 60 s | Forged key ids or an outage never fetch per request. |
| IAP key set age | 1 h | Then refetched, and refused as unavailable if that fails. |
| IAP assertion clock skew | 30 s | |
| IAP assertion lifetime, issue to expiry | 10 min, plus the skew at either end | Longer was not issued by IAP. |
| SSE coalescing interval | 250 ms | At most four ticks a second per domain. |
| SSE write stall | 15 s | A subscriber holds the latest revision per domain, not a queue; a stalled one is disconnected and reconnects with current revisions (H6). |
| requests in flight per process | 128 | Overflow is a 503 with `Retry-After`. |
| store connections per process | 16 readers, plus the one writer | Reads borrow a reader; writes take the writer in turn. |
| assistant turns in flight per process | 32 | |
| automatic resends of a write with no answer | 5, waiting 250 ms, doubling, at most 15 s | Each waits for the connection; then the person retries by hand. |
| journeys a tab keeps derived after their view closes | 8 | Least recently shown beyond this are released. |
| canvas layouts a tab keeps | 32 views, 4 per view | Least recently laid out beyond this are forgotten. |
| saves the sync chip lists as Recent | 10 | Newest first. |
| CI check part duration | 45 min | Main never cancels a run in flight, so a hung part would hold the next (`timeout-minutes` in `check.yml`). |

## Code conventions

- **Rust**: stable toolchain pinned by mise; default `rustfmt`; `clippy` `pedantic` as warnings plus denied `unwrap_used`, `expect_used`, `panic` in shell handlers, `indexing_slicing` in the engine, `too_many_lines` at 70, `missing_docs` on public engine items; `#![forbid(unsafe_code)]` everywhere; warnings are errors in CI.
- **Deterministic by construction.** The engine iterates only ordered collections (`BTreeMap`, `IndexMap`, sorted `Vec`); `HashMap` is denied there. A property test checks two derives serialize identically, under `patina` and plain.
- **Errors carry context**: `Display` names the path or key; shell errors wrap engine errors without flattening.
- **Comments say why.**
- **PRD ids in code**: in the doc comment of the function implementing a requirement (`/// D1a skip cascade.`) and in scenario tests.
- **TypeScript**: strict, type-aware `eslint`, `max-lines-per-function` at 70, no `any` outside generated code. Generated files live only in the generated paths (ARCHITECTURE, Generated artifacts) and are never hand-edited.

## Testing

**One structural check beats a test per case.** A property that must hold everywhere (no id in served text, nothing outside its box, a tool list within budget) is one check every case runs through, not a test per screen or message.

### Engine: examples, properties, replay

- **Scenario tests** (J3): every PRD scenario, at a fixed clock over the shared fixtures, citing its ids, asserting derived bounds, blocking, shortfalls, and ranking. Each scenario once.
- **Replay** (J3): engine tests apply patches through `support::apply`, which replays the patch's events and compares; every mutation type emits an event (J2).
- **Property tests** over generated graphs and operation sequences within the limits: apply then replay equals apply; a rejected patch changes nothing; derive is pure; gravity never decreases upstream along a dependency between in-scope nodes; no accepted contradictory chain; every implicit edge names its source.
- **Round trips**: import and export, JSON and YAML, proposal edit and apply.

#### Property tests

Use `patina-dst-proptest`, a drop-in for `proptest`'s prelude seeded from `patina_dst::rng()`: a fixed seed under `cargo test`, swept and byte-for-byte replayable under a campaign. Where plain `proptest` is better, use it and record the gap in a `decisions/` file tagged `[patina]`. A failing case becomes an example-based scenario test, then a fix.

### Shell: conformance and integration

- **Store conformance suite** (`crates/store`) on memory and SQLite: revision checks, atomic commit, alias resolution, index queries.
- **Persisted types are mapped exhaustively, in every backend.** Codecs destructure with no `..`, build loaded values as struct literals with no `..` or `Default`, and match every persisted enum variant with no catch-all, so a new field or variant does not compile until every backend maps it. The every-field and every-variant cases (`cairn_store::build::every`, `cairn_store::conformance::every`) catch what the compiler cannot. A schema change and its mapping land together.
- **API and MCP tests** in process on the memory store: every endpoint and tool once, every rejection shape once, capability gating.
- **Browser tests** (Playwright, each test against a `cairn demo` of its own, seeded with the fixtures) are only for what a browser catches: real layout, scroll, focus, gestures; wiring across modules unit tests mock; live updates, conflicts, and sign-in; the shipped binary. One test per write path, not per feature rule: rules are engine scenarios, API tests, or Vitests of pure functions (extract React-state rules into one).
  - Specs import `test` from `web/app/e2e/test.ts` (checks containment, DESIGN's Nothing spills out of its box, and no sideways scroll after every test). Pictures go through `capture` (`e2e/invariants.ts`), which runs those checks, no internal keys, and on touch screens touch targets.
  - New screen states go in `e2e/screenshots.ts`; the stress walk (`stress.spec.ts`, worst-case content at the limits, every width and theme, plus touch at phone width), `mise run screenshots`, the screenshot walk's checks (light and dark, folds opened, focus ring, axe-core), and `mise run sweep` (every media-query breakpoint, a pixel either side) all reach states through it. None uses a model.
- **Browser tests wait on events, never on time**: web-first assertions or `settled(page)` (`web/app/e2e/shell.ts`); non-retrying reads run on a settled page (`whenSettled`) or in `expect.poll`/`toPass`; clock-only displays use `page.clock`; reduced motion. eslint forbids fixed waits and unsettled reads. Screen-opening and write helpers return settled.

### Simulation with `patina` (exploratory, not a gate)

[`patina`](https://github.com/JacobHayes/patina) runs `std` programs under a seeded virtual OS with injected faults and byte-for-byte replay. Campaigns run as `mise run sim` (locally and nightly), never in `mise run check`; the check covers only each testbed's lint and unit tests. A finding becomes an ordinary deterministic test, then a fix.

- **Shell-crate instrumentation** (`patina_dst`, inert outside simulation), added with the code and allowed before any campaign uses it: `sometimes!`/`reachable!` oracles proving interesting paths ran, and `buggify!` sites for faults the runtime cannot inject (provider timeout, notifier delay, a clock read straddling midnight).
- **Testbeds** (`testbeds/`, each its own Cargo workspace driven by `sim.sh`): `multiplayer` (the real router, N clients on a lossy network, against stale-patch handling, H5; invariants in `testbeds/multiplayer/README.md`) and `durability` (SQLite under filesystem faults and torn-write crashes: after restart state equals the replayed log and no acknowledged commit is lost).
- **No point tests** (`#[patina_dst::test]`): timing-sensitive code takes time as an argument and is tested deterministically.

Keeping it honest: a declared `sometimes!` that never fired fails the campaign; a finding that does not reproduce is `NONDETERMINISM` and fails `sim.sh`; fault knobs are recorded; a failing seed is minimized and rewritten as a deterministic test, never kept as the regression test. No check depends on a seed's outcome: it names the event it injects (`--fs-crash-at sync:40`) or states a property over every run plus a required `sometimes!`.

Tooling: mise installs `cargo-patina` from git at a pinned revision; `patina-dst` and `patina-dst-proptest` are git dependencies at the same revision. `mise run sim` fails, never skips, without it; the core ladder never needs it.

## The validation ladder

`mise run check:fast [filter...]` is the inner loop; `mise run check` runs every step, never skipping. Steps live in `scripts/steps/`. Lint, unit, web-unit, generated and embed run first, one at a time and in order (generated rewrites files the others read); the rest run side by side. Nothing is skipped silently.

| Step | Check |
|---|---|
| lint | `rustfmt`, `clippy` (each testbed too), kebab-case patina labels, TypeScript typecheck, `eslint`, CSS lint (`mise run lint:css`), `cargo-machete`, `knip`, the `wasm32-unknown-unknown` build of the runtime-free crates |
| unit | Rust unit tests (every crate and testbed), engine scenarios, replay |
| web-unit | Vitest |
| generated | regenerate the generated paths; fail on any change |
| embed | `cargo check` of `cairn` with default features (web build embedded) |
| property | property tests, fewer cases locally than in CI |
| integration | store conformance, API and MCP in process, doc tests |
| cost | cost tests at the limits, budgeted in operations, not wall time |
| browser | the binary's tests; end-to-end against `cairn demo`; screenshot walk; breakpoint sweep |
| advisories | `cargo deny check advisories` over every lock file; an open RustSec advisory fails unless `deny.toml` accepts it with why |

- **CI** runs `mise run check` on every push, in parts on separate runners: lint (lint, advisories); rust (unit, property, integration, cost); web (web-unit, generated, embed); browser. `scripts/ladder --parts` feeds the matrix; a step in no part or two fails the ladder. CI's run on main is the gate; on main a run in flight is never cancelled. Nightly adds more property cases and `mise run sim`. A release is a manual dispatch that refuses unless CI passed on main's head.
- **`mise run check:fast`** on the rebased result gates landing. It runs lint, then only the steps whose inputs changed (`scripts/changed`, from `jj diff` of `@`, or `@-` when empty), and says what it skipped. Rust inputs select unit and property; web or Rust inputs select web-unit; web inputs select browser. Integration and cost run when what they build or read changed. Never generated, embed, or advisories.
- **`mise run check:fast <filter>`** runs only matching Rust tests (by name), Vitest files or tests, and Playwright specs or tests (Chromium); no lint; fails when nothing matches.
- **`mise run review`** runs the review jobs (code review, `cull`, `screen-critique` with screenshots, `acceptance-audit` with `--plan`) into `dist/review/`; **`mise run land [--fix]`** squashes, rebases onto `main`, runs `check:fast`, and moves `main` only if it passes, refusing while main's newest CI run is red unless `--fix`. It never pushes.
- Checks take a machine-wide lock (`scripts/locked`), so one runs at a time.
- **A step that runs zero tests fails**, per suite; lint fails on a missing tool. Outside CI lint applies auto-fixes first and says to commit them; in CI it only verifies.
- **Integration tests are one binary**: `crates/tests/tests/integration/main.rs`, a module per crate. A new file goes in its crate's directory there, declared in `mod.rs`. Steps pick tests by module: `mod property` (`property_*.rs`), `mod conformance` and `mod in_process` (integration), `mod binary` (browser), `mod cost` (`cost_*.rs`), the rest unit.
- Output is quiet: each step logs to `dist/ladder/<step>.log`, prints one line on success, and on failure only what a person acts on plus the rerun command. New compile sites are wrapped in `scripts/timed`.

### Growing the ladder

A step is a file in `scripts/steps/`, listed in `scripts/ladder` with its CI part. Add it in the same change as the first tests of its kind.
