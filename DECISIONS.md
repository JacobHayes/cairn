# Decisions awaiting review

Judgment calls made while implementing the briefs, for the user to review (`AGENTS.md`, Decide, record, keep going). Newest first. Each entry: date, brief, the question, the call, the alternatives, and what would change it.

## 2026-10-06, brief 1.2: entity creates in another domain's patch advance the deployment revision

- Question: E6 lets an entity create ride in any patch with no deployment revision check, and asks a journey patch that writes an entity reference to name the deployment revision it was validated against. Whether a create riding in a journey patch advances the deployment revision is left open, and the fixtures need an answer.
- Call: yes, once per patch that creates entities (it writes deployment records, and A17's revision counts a domain's changes). A later patch referring to those entities names the resulting revision; a patch referring only to entities it creates itself names none. Each fixture scenario runs in a fresh deployment, and a fixture test checks the rule.
- Alternatives: only deployment-domain patches advance the deployment revision (then a reference check could miss a create that landed in between).
- What would change it: the engine or store (2.1, 3.1) committing riding creates without a deployment revision bump.

## 2026-10-06, brief 1.2: an event's delta is the after-state of the records it wrote

- Question: J1 asks each event to carry "the after-state delta needed to replay it", and J3's replay must be exact; ARCHITECTURE's change set is "entities put and removed by key" for a store that does not depend on the engine. The readiness scout left the delta's representation open.
- Call: one record vocabulary (`Record`, addressed by `RecordKey`, mirroring the Schema outline's tables, with a node field addressable on its own) serves both. An event's delta is the ordered list of `Write`s its mutation made: `Put(record)` as after-state, `Remove(address)` (an address may cover a whole domain, graph, or node), or `CopyGraph` (publishing). Replay applies the writes and re-derives nothing; the change set is the receipt plus the events, and its writes are what a store persists. Side effects (a local-edit marker set by an edit, a snooze cleared by a transition, tombstones written by a removal) appear in the delta as writes, so replay never needs the rules that produced them.
- Alternatives: a typed delta per event type (readable history, but replay re-implements each mutation's effects and must match apply exactly); deltas as full graph snapshots (exact but large).
- What would change it: a store that needs coarser rows than per-field node writes, or history display (J4) needing the mutation as submitted beside its effects, which would add the mutation to the event.

## 2026-10-06, brief 1.2: touched sets by record address, coarse when in doubt

- Question: H5 retries a stale patch automatically when its touched set does not overlap the intervening events', "by key and field"; the scout left the granularity open, and some mutations' effects (create, publish, upgrade, apply a proposal) depend on content they do not name.
- Call: a touched set is a set of record addresses. Node fields, edges, participations, resources, local-edit markers, and each kind of journey state are addressed separately, so an edit and a transition on one node, or edits to two fields, never overlap; a whole node overlaps everything on it (an edge belongs to both ends), so a removal overlaps any change to what it removes. A mutation whose effect depends on unnamed content, and any mutation the dispatch does not list, touches its whole domain: too coarse only costs an automatic retry, never correctness. A transition or answer also touches the node's snooze and answer; proposal edits touch only the proposal.
- Alternatives: per-node granularity (simpler, fewer automatic retries); computing touched sets from the graph at apply (exact but not available to a client deciding whether to resubmit).
- What would change it: retries in the multiplayer testbed (6.1) failing too often, which would refine the coarse cases.

## 2026-10-06, brief 1.2: the file format names a node's parent rather than its full path

- Question: PRD Identity and references says paths are how files refer to nodes. A file could give each node its full path, or its id and its parent's path.
- Call: a node in a file has `id` and `parent` (the parent's path, absent for a root), and every reference (`requires`, conditions, rules, stage bounds, `feeds_milestone`) is a full path. The graph form has the same three fields with `parent` a key, so the two forms share one node type and one wire shape (ARCHITECTURE, File format: one schema). Nodes are a flat list, so parsing never recurses through containment (PRACTICES, No recursion); a canonical file lists them as written, and export (2.7) sorts.
- Alternatives: `path: setup/access` on each node (the path is visible at a glance, but the graph form needs a different identity shape, and a node's id is repeated in its children's paths); nested `children:` (reads as a tree, but recursion in parsing and deep indentation at depth 16).
- What would change it: authors finding parent-plus-id harder to read than full paths in practice.

## 2026-10-06, brief 1.2: mutation semantics the PRD leaves implicit

- Question: the PRD defines what can change but not every mutation's shape; three readings shape the engine.
- Call: (1) start and reach take no date: apply records today (a derive input) and `set_recorded_date` edits it, so a mutation never carries a default the server must trust (F1, F2). (2) A guard bypass is an override mutation applied in the same patch as the transition it covers, so it is its own event as J1 lists ("guard bypassed"), and the engine records the specific failures present on it (D4). (3) In a journey, editing a node's weight or participations is state (B5, glossary), every other field edit is structural, and every change to a route is structural; applying a proposal is structural, since its content is not in the patch.
- Alternatives: dates on the transitions; a bypass flag on the transition mutation (one event, which J1's separate event type argues against).
- What would change it: the engine (2.1) finding a guard bypass in a separate mutation awkward to tie to its transition.

## 2026-10-06, brief 1.2: values the limits table does not name take the nearest named limit

- Question: PRACTICES (Explicit limits) puts a limit on everything, but its table names no limit for keys, client-generated ids, names (entity, route, journey, role and kind titles), emails, choice labels, condition values, reasons, URLs, or for collections such as emails per entity, attachments per node, or entities per deployment. Adding a limit needs the user's sign-off.
- Call: no new limits. Keys and prefixed ids take the id slug limit (64 bytes, prefix included); single-line labels (names, emails, choice titles, condition values) the title limit (256 bytes); free text (reasons, prompts, help) and URLs the body limit (64 KiB). A collection the table does not name is bounded by the serialized graph cap (16 MiB, checked on the graph a patch produces) and, in any document, by the request-body cap (24 MiB), which every parse checks first.
- Alternatives: new named limits (URL 2 KiB, emails per entity, attachments per node), which need sign-off; leaving those values unbounded, which PRACTICES forbids.
- What would change it: a value that is legitimately longer than its borrowed limit (a long URL is the likeliest), or a collection that grows large inside the 16 MiB cap; either becomes a named limit after sign-off.

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
