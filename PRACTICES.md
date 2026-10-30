# Cairn: Engineering practices

How code is written and verified. Companion to `ARCHITECTURE.md`.

## Summary

1. **Assert everything you believe, bound everything you allocate.** Assertions stay on in release; every count, size, depth, and duration has a named limit.
2. **Programmer error panics, user error returns.** An invalid patch is a rejection listing every violation; a broken code invariant is a bug.
3. **A check either fails loudly or proves it ran.** A suite that ran nothing, a missing tool, or a simulation whose faults never fired is a failure.

## Rules

### Bounds and assertions

- **Explicit limits.** See Explicit limits below. Cost and test at the limits, not at typical size.
- **Assertions on in release**, on arguments, returns, and invariants, in positive and negative space. After every apply, assert the revision advanced by one, events equal mutations, and the whole graph still holds its invariants. Relationships between constants are `const` assertions.
- **Bounded load.** Bound concurrency and queues; background work runs on fixed intervals, never per event.
- **No recursion.** Every traversal is iterative with an explicit stack bounded by the depth limit, since wasm stacks are small.
- **Fixed-width integers.** Counts, offsets, weights, and revisions are `u32` or `i32`; `usize` only at slice boundaries, never serialized.

### Code shape

- **Functions of at most 70 lines**, split by phase.
- **Big-endian names with units, no abbreviations**: `node_count_max`, `offset_days`, `commit_duration_ms`.
- **Narrow signatures.** Return the fewest true outcomes and take the narrowest input. Closed sets are enums, matched exhaustively with no wildcard in the engine.
- **Logical interfaces.** Clock, time zone, and viewer are inputs, never read by the engine. I/O, retries, and time live in the shell.

### Scope and dependencies

- **No configuration by default.** A setting exists only when two real deployments need different values: where the data lives, where to listen, the public URL, sign-in providers and their security settings, the assistant's provider, and the time zone. Limits and behaviour are never settings.
- **Do it right while it is hot.** No `TODO` without an owner and reason on the same line; deferred work lives only in the PRD's Later.
- **Back-of-the-envelope first.** A new pass over the graph states its cost at the node limit before code, and comes with a property test.
- **Dependencies are deliberate.** The engine takes only pure dependencies, nothing touching the OS or a runtime, so it compiles to wasm and simulates. Shell crates take a dependency when maintaining our own would cost more than auditing its upgrades; the adding commit says why. Tools are pinned in `mise.toml` and earn their place the same way.

### Simplicity

Do as much as necessary and as little as possible; every layer, script, flag, branch, and rule pays for itself or goes.

- **Question a layer before optimizing it.** Claims about time or cost cite a measurement.
- **Fail closed, recover from durable state.** When in-memory state is in doubt, refuse or crash, and reload from what was committed.
- **Migrate old data once.** One migration, then one code path; never a loader accepting both shapes.
- **No compatibility cruft.** Rename and cut over in one change; no alias, fallback, or shim without a named consumer that cannot move.
- **Cull what no longer earns its place.** Work in an area deletes the code and tests there that no longer pay. Keeping something takes the case adding it would; existing, effort spent, or a check running it are not reasons.
- **Build the end state, removing first.**
- **Fix the class, not the instance**, in the same change.
- **Fix it where it lives.** Report a tool's bug upstream; a workaround that stays names what it waits on.

### Frontend state

- **Machines for lifecycles.** Any owned async workflow (a write, a read, the subscription, an open form, a toast, a conversation) is an XState machine, however small.
- **Pure functions for derived values**, never stored beside their source.
- **Plain data for payloads and anything stored**, never snapshots. A busy machine refuses a new start, and whatever spawns an actor stops it.
- **One owner per fact.** Components read shared actors through selectors and never copy a snapshot into React state. Shared actors are the session root's children; managers keep explicit keyed maps and never use XState's registry.
- **Effects belong to the machine.** Timers are delayed transitions and requests are invoked actors. Cancelling stops waiting, never undoes a commit.
- No hand-written stores with listeners besides preferences and browser sources (media, network, theme), and no flag or counter standing in for a state.

### Not adopted

From Tiger Style and the Power of Ten: static allocation, zero-copy, line length 100. **No dynamic dispatch** is not adopted either: the store, notifier, and auth provider are trait objects, because the seams are the point; async traits use `async-trait`.

## Errors, panics, and rejections

- **User and agent errors are values**, serialized unchanged through the API and MCP. Never a panic.
- **Programmer errors panic.** An engine panic inside a request fails that request and commits nothing; a panic anywhere else in the server aborts the process, since shared state may be inconsistent.
- **No `unwrap` or `expect` outside tests**; assert with a message.
- **Rejections are complete**: validation collects every violation.
- `assert!` is the correctness check. A simulation oracle (`always!`) never replaces one.

## Explicit limits

Limits are named constants, each with the reason for its value in its doc comment, in each Rust crate's `limits.rs` or beside their use in the web app, and used by validation, loops, and tests alike. Every list a person or agent writes has a count limit and every string a byte limit; a value with no limit of its own takes the nearest that fits. A limit hit in practice is a design signal, not a knob: changing one needs the owner's sign-off.

## Code conventions

- **Rust**: stable, pinned; default `rustfmt`; `clippy` pedantic, with `unwrap` and `expect` denied, `panic` denied in handlers, `indexing_slicing` denied in the engine; `unsafe` forbidden; warnings are errors.
- **Deterministic by construction.** The engine iterates only ordered collections; `HashMap` is denied there.
- **Errors carry context**: the path or key.
- **Comments say why.**
- **Requirement ids in code**: in the doc comment of the function implementing a requirement, and in scenario tests.
- **TypeScript**: strict, type-aware `eslint`, no `any` outside generated code.

## Testing

**One structural check beats a test per case.** A property that must hold everywhere (no id in served text, nothing outside its box, a tool list within budget) is one check every case runs through.

- **Scenario tests** cover every product rule once, at a fixed clock over the shared fixtures, citing the requirement ids they cover.
- **Replay**: engine tests apply through a helper that replays the patch's events and compares with stored state.
- **Property tests** over generated graphs and operation sequences: apply then replay equals apply; a rejected patch changes nothing; derive is pure and deterministic. They use `patina-dst-proptest`; a failing case becomes a scenario test, then a fix.
- **Persisted types are mapped exhaustively in every backend**, with no `..`, `Default`, or catch-all, so a new field or variant does not compile until every backend maps it.
- **API and MCP tests** run in process: every endpoint and tool once, every rejection shape once.
- **Browser tests** are only for what a browser catches: real layout, scroll, focus, gestures, wiring across modules, live updates, sign-in, the shipped binary. One test per write path, never per rule. Every test and screenshot runs the shared structural checks.
- **Browser tests wait on events, never on time**: web-first assertions or a settled page. Fixed waits fail the lint. Every picture goes through `capture` in `web/app/e2e/invariants.ts`, which runs the structural checks on that state.
- **Actor tests drive time with a `SimulatedClock`** and async work with controlled promises, never real or faked timers.

### Simulation with patina

[`patina`](https://github.com/JacobHayes/patina) runs programs under a seeded virtual OS with injected faults and byte-for-byte replay. Campaigns (`mise run sim`) are exploratory, never part of the check. Testbeds in `testbeds/` simulate concurrent clients against stale-patch handling, and SQLite under crashes. A finding becomes an ordinary deterministic test, then a fix.

- Shell crates carry inert `sometimes!` oracles and `buggify!` fault sites, added with the code; they stay even before a campaign uses them.
- Timing-sensitive code takes time as an argument and is tested deterministically, not under simulation.
- **No check depends on a seed's outcome**: it names the fault it injects, or states a property over every run. A declared `sometimes!` that never fired fails the campaign.

## The validation ladder

- **`mise run check:fast`** on the rebased result gates landing. It runs only the tests your change touches, and CI runs everything. It never regenerates generated files, builds the embedded binary, or checks advisories: run `mise run gen` yourself when a generator's source changes. With a filter it runs only the matching tests.
- **`mise run check`** runs every step and skips nothing; CI runs it on every push, and CI's run on main is the gate. A release needs main's run to have passed.
- **`mise run review`** runs the review jobs; **`mise run land`** rebases onto main, runs `check:fast`, and moves main only if it passes. It never pushes, and while main's newest CI run is red it takes only a fix.
- **No step calls a model**; review jobs may, checks may not.
- **A step that runs zero tests fails**; a missing tool fails. Outside CI the lint applies its own fixes first.
- Steps are files in `scripts/steps/`, listed in `scripts/ladder` with their CI part. A new step lands with the first tests of its kind. Integration tests are one test binary, a module per crate.
