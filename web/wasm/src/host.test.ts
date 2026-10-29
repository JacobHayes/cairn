// The in-browser host, in Node over the wasm32 module: the in-browser root loads
// each fixture, applies a patch, refuses stale ones, and notifies its subscriber; a trap
// stops the module; the worker keeps a journey's newest document and fails loudly.
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, it, vi } from "vitest";

import type { Schema } from "@cairn/client";

import type { Group, IndexEntry } from "./cases.ts";
import { NOW } from "./cases.ts";
import { brokenWorker, inProcess, wasmModule } from "./in-process.ts";
import type { HostError, PatchRequest } from "./index.ts";

const cases = join(import.meta.dirname, "..", "dist", "cases");

/** A page and a worker of their own: the modules loaded again, with nothing latched. */
function fresh() {
  vi.resetModules();
  return inProcess();
}

/** The journeys the server walk read: every fixture's, as the root seeds them. */
function serverJourneys(): string[] {
  const index = JSON.parse(readFileSync(join(cases, "index.json"), "utf8")) as IndexEntry[];
  const generated = (label: string): boolean => label === "at the limits" || label.endsWith(" generated nodes");
  const journeys = new Set<string>();
  for (const entry of index.filter((found) => !generated(found.label))) {
    const group = JSON.parse(readFileSync(join(cases, entry.file), "utf8")) as Group;
    if (group.document !== undefined) {
      journeys.add((JSON.parse(group.document) as { journey: { header: { id: string } } }).journey.header.id);
    }
  }
  return [...journeys].sort();
}

/** A patch to the vendor evaluation from `base`: a note with `key` on its kickoff. */
function note(id: string, base: number, key: string): PatchRequest {
  return {
    patch: {
      id,
      target: { journey: "j_vendor_eval" },
      base_revision: base,
      mutations: [{ op: "add_annotation", annotation: { key, node: "n_kickoff", note: "From the browser." } }],
    },
  };
}

/** Why `call` failed, as the host reports it. */
async function reasonOf(started: Awaited<ReturnType<typeof fresh>>, call: () => unknown): Promise<HostError | undefined> {
  try {
    await call();
    return undefined;
  } catch (thrown) {
    return thrown instanceof started.host.HostFailure ? thrown.reason : { error: "aborted", message: String(thrown) };
  }
}

const settle = (milliseconds: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, milliseconds));

it("the in-browser root loads each fixture, applies a patch, refuses stale ones, and notifies", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const loaded: { journey: string; derivedBytes: number }[] = [];
  for (const summary of host.journeys().items) {
    const { key } = await worker.load(host.documentText(summary.id));
    loaded.push({ journey: key.journey, derivedBytes: (await worker.derivedText(summary.id)).length });
  }
  expect(loaded.map((journey) => journey.journey).sort()).toEqual(serverJourneys());
  expect(loaded.every((journey) => journey.derivedBytes > 0)).toBe(true);

  const heard: { first: boolean; ticks: Schema<"Tick">[] }[] = [];
  const unsubscribe = host.subscribe(["journey:j_vendor_eval"], (ticks, first) => heard.push({ ticks, first }));
  await settle(0);
  const base = (await worker.load(host.documentText("j_vendor_eval"))).key.revision;
  expect(host.patch(note("p_browser_first", base, "a_browser_first"))).toMatchObject({ outcome: "applied", receipt: { revision: base + 1 } });
  const overlapping = note("p_browser_second", base, "a_browser_first");
  const separate = note("p_browser_third", base, "a_browser_third");
  // H5: the overlapping one is answered stale; the separate one is applied at the current revision, and says what it moved past.
  const reason = await reasonOf(started, () => host.patch(overlapping));
  expect(reason).toMatchObject({ error: "rejected", rejection: { rejection: "stale" } });
  expect(host.patch(separate)).toMatchObject({
    outcome: "applied",
    receipt: { revision: base + 2 },
    over: { conflicts: [{ expected: base, current: base + 1 }] },
  });
  await settle(400);
  const vendor = { domain: { journey: "j_vendor_eval" } };
  expect(heard[0]).toMatchObject({ first: true, ticks: [{ of: vendor, revision: base }] });
  expect(heard.at(-1)).toMatchObject({ first: false, ticks: [{ of: vendor, revision: base + 2 }] });
  expect((await worker.load(host.documentText("j_vendor_eval"))).key).toMatchObject({ journey: "j_vendor_eval", revision: base + 2 });
  unsubscribe();
});

it("a trap stops the page's module for every caller", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  expect(host.capabilities().auth.length).toBeGreaterThan(0);
  // A trap reaches JavaScript as a RuntimeError thrown out of a call into the module.
  const trap = await reasonOf(started, () =>
    started.host.hosted(() => {
      throw new WebAssembly.RuntimeError("unreachable");
    }),
  );
  const aborted = { error: "aborted", message: "unreachable" };
  expect(trap).toEqual(aborted);
  expect(await reasonOf(started, () => host.capabilities())).toEqual(aborted);
  expect(await reasonOf(started, () => host.journeys())).toEqual(aborted);
  expect(await reasonOf(started, () => host.engine.readRouteFileText(""))).toEqual(aborted);
});

it("the worker keeps a journey's newest document and refuses an older one", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const earlier = host.documentText("j_vendor_eval");
  const base = (await worker.load(earlier)).key.revision;
  host.patch(note("p_ordering", base, "a_ordering"));
  expect((await worker.load(host.documentText("j_vendor_eval"))).key.revision).toBe(base + 1);
  expect(await reasonOf(started, () => worker.load(earlier))).toMatchObject({ error: "superseded", held: { revision: base + 1 }, posted: { revision: base } });
  expect((await worker.load(host.documentText("j_vendor_eval"))).key.revision).toBe(base + 1);
});

it("a derivation is freed only by the load that made it", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const document = host.documentText("j_vendor_eval");
  const first = await worker.load(document);
  // The same document loaded again replaces the first load, as a remount refetching an unchanged journey does.
  const second = await worker.load(document);
  expect(second.handle.journey).toBe(first.handle.journey);
  expect(second.handle.load).not.toBe(first.handle.load);
  await worker.release(first.handle);
  expect((await worker.derivedText("j_vendor_eval")).length).toBeGreaterThan(0);
  await worker.release(second.handle);
  expect(await reasonOf(started, () => worker.derivedText("j_vendor_eval"))).toMatchObject({ error: "missing" });
  worker.terminate();
});

/** A proposal that adds `key` to the vendor evaluation, drafted against `revision`. */
function adding(key: string, revision: number): Schema<"Proposal"> {
  return {
    id: "pr_slot",
    destination: { journey: "j_vendor_eval" },
    revision: 1,
    status: "open",
    created_by: "u_local",
    created_at: NOW,
    draft: { title: "Adds one", destination_revision: revision, mutations: [{ op: "add_node", node: { key, id: key, kind: "action", title: "Added by the proposal" } }] },
  };
}

it("a proposal's journey after is held under its slot beside the journey, and released with it", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const source = await worker.load(host.documentText("j_vendor_eval"));
  const slot = "j_vendor_eval/proposals/pr_slot";
  const request = { proposal: adding("n_added_by_slot", source.key.revision), at: NOW, actor: { user: "u_local" } };
  const answered = await worker.preview(source.key, request, slot);
  expect(answered.after?.destination).toBe("journey");
  expect(answered.loaded?.key.journey).toBe(slot);
  // The slot's derivation has the node the journey does not.
  const derived = JSON.parse(await worker.derivedText(slot)) as { nodes: Record<string, unknown> };
  const real = JSON.parse(await worker.derivedText("j_vendor_eval")) as { nodes: Record<string, unknown> };
  expect([derived.nodes["n_added_by_slot"] !== undefined, real.nodes["n_added_by_slot"] !== undefined]).toEqual([true, false]);
  // A second preview replaces the slot's load under a new generation, so nothing read from the first is read from the next.
  const again = await worker.preview(source.key, request, slot);
  expect(again.loaded?.key.generation).not.toBe(answered.loaded?.key.generation);
  expect(again.loaded).toBeDefined();
  await worker.release((again.loaded ?? { handle: { journey: slot, load: 0 } }).handle);
  expect(await reasonOf(started, () => worker.derivedText(slot))).toMatchObject({ error: "missing" });
  expect((await worker.derivedText("j_vendor_eval")).length).toBeGreaterThan(0);
  worker.terminate();
});

it("a preview of a journey that has moved on is refused as superseded", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const earlier = await worker.load(host.documentText("j_vendor_eval"));
  host.patch(note("p_moves_on", earlier.key.revision, "a_moves_on"));
  await worker.load(host.documentText("j_vendor_eval"));
  const request = { proposal: adding("n_late", earlier.key.revision), at: NOW, actor: { user: "u_local" } };
  expect(await reasonOf(started, () => worker.preview(earlier.key, request, "j_vendor_eval/proposals/pr_slot"))).toMatchObject({ error: "superseded", posted: { revision: earlier.key.revision } });
  worker.terminate();
});

it("a derive worker that fails rejects what was asked of it", async () => {
  const started = await fresh();
  expect(await reasonOf(started, () => started.host.DeriveWorker.start(wasmModule, brokenWorker()))).toMatchObject({ error: "aborted" });
});

it("a route's graph is held under its slot and answers its trace and schedule; a replaced slot answers only its newest graph", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const deployment = host.deployment();
  const vendor = host.routeVersion("vendor-evaluation", 1).graph;
  const launch = host.routeVersion("product-launch", 1).graph;
  const first = await worker.loadRoute({ graph: vendor, deployment }, "route:a#1");
  const second = await worker.loadRoute({ graph: launch, deployment }, "route:b#2");
  const trace = await worker.project("route:a#1", { projection: "trace", key: "n_partner_results" });
  expect(trace.upstream).toContain("n_partner_runs");
  // The other slot holds another graph: the same question there is a node it lacks.
  expect(await reasonOf(started, () => worker.project("route:b#2", { projection: "trace", key: "n_partner_results" }))).toMatchObject({ error: "missing" });
  expect((await worker.routeSchedule("route:b#2")).end).toBe("n_launch");
  // A slot's next load is a new generation, and what the first load's handle owns is gone with it.
  const again = await worker.loadRoute({ graph: launch, deployment }, "route:a#1");
  expect(again.key.generation).not.toBe(first.key.generation);
  await worker.release(first.handle);
  expect((await worker.project("route:a#1", { projection: "trace", key: "n_launch" })).node).toBe("n_launch");
  await worker.release(again.handle);
  expect(await reasonOf(started, () => worker.project("route:a#1", { projection: "trace", key: "n_launch" }))).toMatchObject({ error: "missing" });
  await worker.release(second.handle);
  worker.terminate();
});

it("a route's slot refuses what only a journey's document answers, and a journey's refuses the route's schedule", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmModule, () => NOW);
  const worker = await started.worker();
  const loaded = await worker.loadRoute({ graph: host.routeVersion("vendor-evaluation", 1).graph, deployment: host.deployment() }, "route:a#1");
  const journey = await worker.load(host.documentText("j_vendor_eval"));
  expect(await reasonOf(started, () => worker.derivedText("route:a#1"))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.renderDraft("route:a#1", { key: "n_kickoff", resource: "a_x" }))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.applyText("route:a#1", { patch: note("p_x", 1, "a_x").patch, at: NOW, actor: { user: "u_local" } }))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.preview(loaded.key, { proposal: adding("n_x", 1), at: NOW, actor: { user: "u_local" } }, "slot"))).toMatchObject({ error: "missing" });
  expect(await reasonOf(started, () => worker.routeSchedule("j_vendor_eval"))).toMatchObject({ error: "missing" });
  expect((await worker.derivedText(journey.key.journey)).length).toBeGreaterThan(0);
  worker.terminate();
});
