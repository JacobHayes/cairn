# Decisions awaiting review

Judgment calls made while implementing the briefs, for the user to review (`AGENTS.md`, Decide, record, keep going). Newest first. Each entry: date, brief, the question, the call, the alternatives, and what would change it.

## 2026-10-06, brief 1.3: 6.1 drives the real HTTP server, on one current-thread runtime

- Question: does 6.1's multiplayer testbed drive the real HTTP server (axum on tokio) under the patina shim, or call the service in-process (PRACTICES, Simulation with patina: Testbeds)?
- Call: the real HTTP server. The spike (`testbeds/spike`, README) runs an axum server and hyper clients under the shim with a clean audit and no allowance, byte-identical repeats, flag-free replay, a passing 16-generation campaign, and 200 seeds of dropped, refused, reset, and delayed connections, all with the faults visibly reaching the retry and deduplication paths. Conditions, from the three patina gaps below: the server and every client run as tasks on one current-thread tokio runtime (not a multi-thread runtime, not a runtime per client thread; blocking `std::net` client threads beside it also work, shown only by a reproducer); a task wakes the runtime every millisecond while delayed-delivery faults are on; network faults run as explicit knobs (`cargo patina explore run` or `run`), not `campaign --faults`. So 6.1's "N client threads" are N client tasks (6.1 brief updated), and 4.2 must let a caller serve the API's `axum::Router` on a runtime the caller builds (a function from the composition root's `Service` to a `Router`), so the testbed and the binary differ only in the runtime around it.
- Alternatives: the in-process fallback (clients call the service directly, with 4.2's transport-independent retry and subscription logic), which needs none of the conditions but leaves the HTTP layer, SSE, and request handling out of every campaign; or waiting for patina to run multi-thread tokio.
- What would change it: a patina revision that fixes gap 1 (then the testbed may use the binary's own runtime) or gap 2 (then the ticker goes); SSE failing under the shim when 6.1 adds it (the spike covers request and response, not long-lived streams), which would move subscriptions in-process.

## 2026-10-06, brief 1.3: [patina] tokio I/O on more than one thread aborts the shim

- Question: can a testbed run tokio's multi-thread runtime, or several current-thread runtimes on several threads, under the native shim?
- Call: no, at the pinned revision. A minimal tokio TCP ping-pong on a two-worker runtime aborts on 14 of 20 seeds with `patina native shim fatal: invalid_state: cannot wake scheduler task N in state Runnable` or `deadlock: no runnable tasks; parked tasks: ... (futex-wait), ... (epoll-wait)`; natively it passes 100 of 100. Task spawning and timers alone pass; the TCP I/O driver is what breaks. The spike's multi-thread and thread-per-client flavors fail 6 of 40 fault-free seeds each. Reproducer and outputs: `testbeds/spike/README.md`, gap 1; `mise run sim` fails when it stops reproducing. Testbeds use one current-thread runtime.
- Alternatives: patching patina (out of scope: patina is read-only to Cairn); running the multi-thread flavor anyway and discarding aborted seeds (hides the abort class and wastes the campaign).
- What would change it: a patina fix; then the multi-thread legs in `testbeds/spike/sim.sh` start failing and this entry and the README section are updated.

## 2026-10-06, brief 1.3: [patina] a delayed TCP segment does not wake epoll_wait

- Question: do patina's network delay knobs (`--net-latency-nanos`, `--net-jitter-nanos`, a drop's retransmit backoff) work for a tokio guest?
- Call: only with a timer keeping the reactor awake. A segment whose delivery time passes while the reader blocks in `epoll_wait` is delivered at the reactor's next wake instead: 1 ns of latency makes a ping-pong under a 1 s timeout take the full 1 s of virtual time, and with no timer armed the shim aborts (`a shim lock was re-entered by the thread that holds it (a signal handler ran over shim code)`). Blocking `std::net` reads are delivered on time. The spike's `--tick-ms 1` task bounds the lag to 1 ms; with it every delayed run passes. Patina's `pubsub` testbed passes its latency leg because its heartbeat timers do the same. Reproducer: `testbeds/spike/README.md`, gap 2.
- Alternatives: no delay faults for tokio guests (loses the timeout and retry paths 6.1 exists to test); a larger attempt timeout (delays then always cost a full timeout, which is a different fault than the one configured).
- What would change it: a patina fix; then the ticker is dropped and the `delayed-delivery` gap leg starts failing.

## 2026-10-06, brief 1.3: [patina] campaign --faults is unusable for a TCP-only guest

- Question: can `mise run sim` sweep network faults with `cargo patina campaign --faults`, as PRACTICES' campaigns assume?
- Call: not at the pinned revision. The `--faults` bands draw `--net-duplicate-permille`, which acts on datagrams only, in every generation; a TCP-only guest applies no duplicates and every such generation is classed `VACUOUS_NET_FAULT` (4 of 4 in the spike), with no spec field to drop one band and no `classify` rule that can override a vacuity class. The vacuity class also masks the guest's own outcome: generations whose guest gave up (exit 4) are still `VACUOUS_NET_FAULT`. Separately, the vacuity diagnostic is a zero-count test, so a live knob that draws no effect by chance is reported inert (seed 158 of the spike's sweep: 0 refusals in 55 connects at 100 per mille). The spike's campaign therefore runs `--buggify --sched-pct`, and network faults run as a seeded explicit-knob sweep (`cargo patina explore run`); 6.1 does the same.
- Alternatives: give the guest a UDP side channel so the duplicate knob fires (a guest change made only to satisfy the classifier: green by construction, rejected); `--allow-unmet-sometimes` or similar waivers (none covers vacuity).
- What would change it: a patina revision that bands datagram-only knobs only for guests with datagram traffic, or lets a spec deselect a band; then the spike's campaign adds `--faults` and the `campaign-faults-vacuous` gap leg starts failing.

## 2026-10-06, brief 1.1: rung 1's TypeScript checks join with the first TypeScript package

- Question: PRACTICES (The validation ladder) lists a TypeScript typecheck and `eslint` in rung 1, and brief 1.1 pins Node and creates the npm workspace root, but no TypeScript exists yet. Growing the ladder says `check` never depends on a tool nothing uses, and rung 1 fails when a tool reports nothing checked.
- Call: rung 1 runs `rustfmt` and `clippy` (plus a check that every crate inherits the workspace lints). The first brief with a TypeScript package (4.2, `web/client`) adds `typescript` and `eslint` as root devDependencies, the strict `tsconfig` and the `eslint` config (type-aware rules, `max-lines-per-function` at 70, no `any` outside generated code), and a suite for each to `mise-tasks/check/1`.
- Alternatives: install `typescript` and `eslint` now with configs that check nothing, which either fails the nothing-checked rule or needs an exemption from it.
- What would change it: TypeScript landing before 4.2.

## 2026-10-06, brief 1.1: where the crate-scoped lints apply

- Question: PRACTICES (Code conventions) denies some lints only in some crates (`indexing_slicing` and `HashMap` in the engine, `missing_docs` on public engine items, `panic` in shell handlers), but Cargo's `[workspace.lints]` is one table for every crate, and a crate that inherits it cannot add to it.
- Call: the workspace lints carry the crate-wide set (`unsafe_code` forbidden, `pedantic` warned, `unwrap_used`, `expect_used`, `too_many_lines` at 70 denied). Rung 1 raises the scoped lints on top, by package name, from the moment the crate joins the workspace: `cairn-engine` gets `clippy::indexing_slicing`, `clippy::disallowed_types` (`HashMap` and `HashSet`, listed in `clippy.toml`), and `missing_docs`; `cairn-api` and `cairn-mcp`, the crates whose functions are handlers, get `clippy::panic`. Test code (`#[test]`, `#[cfg(test)]`) may unwrap, expect, index, and panic, through `clippy.toml`'s `allow-*-in-tests`. Rung 1 denies warnings locally as CI does, so `check` means the same everywhere.
- Alternatives: `#![deny(...)]` attributes in each crate's root, which depend on the brief that creates the crate remembering them; a `clippy.toml` per crate, which cannot set lint levels.
- What would change it: handlers living in another crate; wanting `unwrap` denied in tests too; a rust-analyzer user wanting the scoped lints in the editor (then also add the attributes).
