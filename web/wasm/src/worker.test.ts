// The engine in Node over the wasm32 module: a trap stops the module; the derive worker keeps a
// journey's newest document, frees a derivation only by the load that made it, holds a
// proposal's journey after beside the journey, holds a route's graph under its slot, and fails
// loudly. Node's V8 runs the same wasm32 binary a browser does, which is what catches drift only
// that target has (usize, floats, the stack).
import { expect, it, vi } from "vitest";

import type { Schema } from "@cairn/client";

import { brokenWorker, inProcess, wasmModule } from "./in-process.ts";
import type { HostError } from "./index.ts";

const NOW = "2026-10-06T12:00:00Z";
const JOURNEY = "j_sample";

/** A page and a worker of their own: the modules loaded again, with nothing latched. */
function fresh() {
  vi.resetModules();
  return inProcess();
}

/** A journey's domain document with one node, `n_kickoff`, at revision 1. */
function sample(): string {
  return JSON.stringify({
    journey: {
      header: { id: JOURNEY, name: "Sample", status: "active", created_at: "2026-10-01T00:00:00Z", created_on: "2026-10-01" },
      revision: 1,
      graph: { nodes: [{ key: "n_kickoff", id: "kickoff", kind: "action", title: "Kickoff" }], state: { nodes: { n_kickoff: { state: "todo", provenance: "local" } } } },
    },
    inputs: {
      today: "2026-10-06",
      timezone: "UTC",
      rank: { urgency: 0.4, late: 0.15, gravity: 0.25, unlocks: 0.2, horizon_days: 14, undecided_discount: 0.5, other_owner_factor: 2.0 },
      deployment: { revision: 1, identities: [] },
    },
  });
}

/** The deployment of `sample`. */
const deployment: Schema<"Deployment"> = { revision: 1, identities: [] };

/** A patch to the sample from `base`: a note with `key` on its kickoff. */
function note(id: string, base: number, key: string): Schema<"Patch"> {
  return {
    id,
    target: { journey: JOURNEY },
    base_revision: base,
    mutations: [{ op: "add_annotation", annotation: { key, node: "n_kickoff", note: "From the browser." } }],
  };
}

/** Why `call` failed, as the engine reports it. */
async function reasonOf(started: Awaited<ReturnType<typeof fresh>>, call: () => unknown): Promise<HostError | undefined> {
  try {
    await call();
    return undefined;
  } catch (thrown) {
    return thrown instanceof started.host.HostFailure ? thrown.reason : { error: "aborted", message: String(thrown) };
  }
}

/** The sample document after a patch that adds the note `key`, which leaves it one revision on. */
function after(started: Awaited<ReturnType<typeof fresh>>, text: string, key: string): Promise<string> {
  return started.host.loadEngine(wasmModule).then((engine) => {
    const revision = (JSON.parse(text) as { journey: { revision: number } }).journey.revision;
    return JSON.stringify(engine.apply(text, { patch: note(`p_${key}`, revision, key), at: NOW, actor: { user: "u_local" } }).document);
  });
}

it("a trap stops the page's module for every caller", async () => {
  const started = await fresh();
  const engine = await started.host.loadEngine(wasmModule);
  expect(engine.derive(sample()).key.journey).toBe(JOURNEY);
  // A trap reaches JavaScript as a RuntimeError thrown out of a call into the module.
  const trap = await reasonOf(started, () =>
    started.host.hosted(() => {
      throw new WebAssembly.RuntimeError("unreachable");
    }),
  );
  const aborted = { error: "aborted", message: "unreachable" };
  expect(trap).toEqual(aborted);
  expect(await reasonOf(started, () => engine.derive(sample()))).toEqual(aborted);
  expect(await reasonOf(started, () => engine.readRouteFileText(""))).toEqual(aborted);
});

it("the worker keeps a journey's newest document and refuses an older one", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const earlier = sample();
  const base = (await worker.load(earlier)).key.revision;
  const later = await after(started, earlier, "a_ordering");
  expect((await worker.load(later)).key.revision).toBe(base + 1);
  expect(await reasonOf(started, () => worker.load(earlier))).toMatchObject({ error: "superseded", held: { revision: base + 1 }, posted: { revision: base } });
  expect((await worker.load(later)).key.revision).toBe(base + 1);
});

it("a derivation is freed only by the load that made it", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const document = sample();
  const first = await worker.load(document);
  // The same document loaded again replaces the first load, as a remount refetching an unchanged journey does.
  const second = await worker.load(document);
  expect(second.handle.journey).toBe(first.handle.journey);
  expect(second.handle.load).not.toBe(first.handle.load);
  await worker.release(first.handle);
  expect((await worker.derivedText(JOURNEY)).length).toBeGreaterThan(0);
  await worker.release(second.handle);
  expect(await reasonOf(started, () => worker.derivedText(JOURNEY))).toMatchObject({ error: "missing" });
  worker.terminate();
});

/** A proposal that adds `key` to the sample, drafted against `revision`. */
function adding(key: string, revision: number): Schema<"Proposal"> {
  return {
    id: "pr_slot",
    destination: { journey: JOURNEY },
    revision: 1,
    status: "open",
    created_by: "u_local",
    created_at: NOW,
    draft: { title: "Adds one", destination_revision: revision, mutations: [{ op: "add_node", node: { key, id: key, kind: "action", title: "Added by the proposal" } }] },
  };
}

it("a proposal's journey after is held under its slot beside the journey, and released with it", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const source = await worker.load(sample());
  const slot = `${JOURNEY}/proposals/pr_slot`;
  const request = { proposal: adding("n_added_by_slot", source.key.revision), at: NOW, actor: { user: "u_local" } };
  const answered = await worker.preview(source.key, request, slot);
  expect(answered.after?.destination).toBe("journey");
  expect(answered.loaded?.key.journey).toBe(slot);
  // The slot's derivation has the node the journey does not.
  const derived = JSON.parse(await worker.derivedText(slot)) as { nodes: Record<string, unknown> };
  const real = JSON.parse(await worker.derivedText(JOURNEY)) as { nodes: Record<string, unknown> };
  expect([derived.nodes["n_added_by_slot"] !== undefined, real.nodes["n_added_by_slot"] !== undefined]).toEqual([true, false]);
  // A second preview replaces the slot's load under a new generation, so nothing read from the first is read from the next.
  const again = await worker.preview(source.key, request, slot);
  expect(again.loaded?.key.generation).not.toBe(answered.loaded?.key.generation);
  expect(again.loaded).toBeDefined();
  await worker.release((again.loaded ?? { handle: { journey: slot, load: 0 } }).handle);
  expect(await reasonOf(started, () => worker.derivedText(slot))).toMatchObject({ error: "missing" });
  expect((await worker.derivedText(JOURNEY)).length).toBeGreaterThan(0);
  worker.terminate();
});

it("a preview of a journey that has moved on is refused as superseded", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const first = sample();
  const earlier = await worker.load(first);
  await worker.load(await after(started, first, "a_moves_on"));
  const request = { proposal: adding("n_late", earlier.key.revision), at: NOW, actor: { user: "u_local" } };
  expect(await reasonOf(started, () => worker.preview(earlier.key, request, `${JOURNEY}/proposals/pr_slot`))).toMatchObject({ error: "superseded", posted: { revision: earlier.key.revision } });
  worker.terminate();
});

it("a derive worker that fails rejects what was asked of it", async () => {
  const started = await fresh();
  expect(await reasonOf(started, () => started.host.DeriveWorker.start(wasmModule, brokenWorker()))).toMatchObject({ error: "aborted" });
});

/** A route graph: `n_second` requires `n_first`, and a milestone `n_end` closes it. */
const lead: Schema<"Graph"> = {
  nodes: [
    { key: "n_first", id: "first", kind: "action", title: "First", estimate: 2 },
    { key: "n_second", id: "second", kind: "action", title: "Second", estimate: 3, requires: ["n_first"] },
    { key: "n_end", id: "end", kind: "milestone", title: "End", final: true, requires: ["n_second"] },
  ],
};

/** Another graph, which has no `n_second`. */
const other: Schema<"Graph"> = { nodes: [{ key: "n_only", id: "only", kind: "milestone", title: "Only", final: true }] };

it("a route's graph is held under its slot and answers its trace and schedule; a replaced slot answers only its newest graph", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const first = await worker.loadRoute({ graph: lead, deployment }, "route:a#1");
  const second = await worker.loadRoute({ graph: other, deployment }, "route:b#2");
  const trace = await worker.project("route:a#1", { projection: "trace", key: "n_second" });
  expect(trace.upstream).toContain("n_first");
  // The other slot holds another graph: the same question there is a node it lacks.
  expect(await reasonOf(started, () => worker.project("route:b#2", { projection: "trace", key: "n_second" }))).toMatchObject({ error: "missing" });
  expect((await worker.routeSchedule("route:b#2")).end).toBe("n_only");
  // A slot's next load is a new generation, and what the first load's handle owns is gone with it.
  const again = await worker.loadRoute({ graph: other, deployment }, "route:a#1");
  expect(again.key.generation).not.toBe(first.key.generation);
  await worker.release(first.handle);
  expect((await worker.project("route:a#1", { projection: "trace", key: "n_only" })).node).toBe("n_only");
  await worker.release(again.handle);
  expect(await reasonOf(started, () => worker.project("route:a#1", { projection: "trace", key: "n_only" }))).toMatchObject({ error: "missing" });
  await worker.release(second.handle);
  worker.terminate();
});

it("a route's slot refuses what only a journey's document answers, and a journey's refuses the route's schedule", async () => {
  const started = await fresh();
  const worker = await started.worker();
  const loaded = await worker.loadRoute({ graph: lead, deployment }, "route:a#1");
  const journey = await worker.load(sample());
  const patch = note("p_x", 1, "a_x");
  expect(await reasonOf(started, () => worker.derivedText("route:a#1"))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.renderDraft("route:a#1", { key: "n_kickoff", resource: "a_x" }))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.applyText("route:a#1", { patch, at: NOW, actor: { user: "u_local" } }))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.preview(loaded.key, { proposal: adding("n_x", 1), at: NOW, actor: { user: "u_local" } }, "slot"))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.routeSchedule(JOURNEY))).toMatchObject({ error: "missing" });
  expect((await worker.derivedText(journey.key.journey)).length).toBeGreaterThan(0);
  worker.terminate();
});
