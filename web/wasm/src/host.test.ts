// Brief 4.5, Acceptance, in Node over the wasm32 module (rung 6): the in-browser root loads
// each fixture, applies a patch, refuses stale ones, and notifies its subscriber; a document
// from another engine version, or a trap, stops the module; the worker keeps a journey's
// newest document and fails loudly.
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, it, vi } from "vitest";

import type { Schema } from "@cairn/client";

import type { Group, IndexEntry } from "./cases.ts";
import { NOW } from "./cases.ts";
import { brokenWorker, inProcess, wasmBytes } from "./in-process.ts";
import type { HostError, PatchRequest, ProjectionRequest } from "./index.ts";

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
  const host = await started.host.InBrowserHost.start(wasmBytes, () => NOW);
  const worker = await started.worker();
  const loaded: { journey: string; derivedBytes: number }[] = [];
  for (const summary of host.journeys().items) {
    const key = await worker.load(host.documentText(summary.id));
    loaded.push({ journey: key.journey, derivedBytes: (await worker.derivedText(summary.id)).length });
  }
  expect(loaded.map((journey) => journey.journey).sort()).toEqual(serverJourneys());
  expect(loaded.every((journey) => journey.derivedBytes > 0)).toBe(true);

  const heard: { first: boolean; ticks: Schema<"Tick">[] }[] = [];
  const unsubscribe = host.subscribe(["journey:j_vendor_eval"], (ticks, first) => heard.push({ ticks, first }));
  await settle(0);
  const base = (await worker.load(host.documentText("j_vendor_eval"))).revision;
  expect(host.patch(note("p_browser_first", base, "a_browser_first"))).toMatchObject({ outcome: "applied", receipt: { revision: base + 1 } });
  const overlapping = note("p_browser_second", base, "a_browser_first");
  const separate = note("p_browser_third", base, "a_browser_third");
  for (const [request, overlaps] of [[overlapping, true], [separate, false]] as const) {
    const reason = await reasonOf(started, () => host.patch(request));
    expect(reason).toMatchObject({ error: "rejected", rejection: { rejection: "stale" } });
    const intervening = reason?.error === "rejected" && reason.rejection.rejection === "stale" ? reason.rejection.intervening : [];
    expect(host.engine.touchedOverlaps(request.patch, intervening)).toBe(overlaps);
  }
  // H5: the separate one is safe to resubmit as is, at the revision the rejection named.
  expect(host.patch({ patch: { ...separate.patch, base_revision: base + 1 } })).toMatchObject({ outcome: "applied", receipt: { revision: base + 2 } });
  await settle(400);
  const vendor = { domain: { journey: "j_vendor_eval" } };
  expect(heard[0]).toMatchObject({ first: true, ticks: [{ of: vendor, revision: base }] });
  expect(heard.at(-1)).toMatchObject({ first: false, ticks: [{ of: vendor, revision: base + 2 }] });
  expect(await worker.load(host.documentText("j_vendor_eval"))).toMatchObject({ journey: "j_vendor_eval", revision: base + 2 });
  unsubscribe();
});

// Version skew latches (ARCHITECTURE, Web UI): once the worker is posted a document from
// another engine it refuses everything, the journey it held before included; once the page's
// engine sees one, it refuses touched sets, local applies, previews, and the root's writes.
// Each side is checked on a module graph of its own, as a page and a worker are realms of
// their own: one latch would otherwise stand for the other.
it("a document from another engine version stops the worker deriving, and the page previewing, writing, and retrying", async () => {
  const worked = await fresh();
  const host = await worked.host.InBrowserHost.start(wasmBytes, () => NOW);
  const worker = await worked.worker();
  const text = host.documentText("j_vendor_eval");
  const document = JSON.parse(text) as Schema<"DomainDocument">;
  const newer = JSON.stringify({ ...document, engine_version: "9.0.0" });
  const request = note("p_skewed", document.journey.revision, "a_skewed");
  const summary: ProjectionRequest = { projection: "status_summary" };
  await worker.load(text);
  expect((await worker.projectText("j_vendor_eval", summary)).length).toBeGreaterThan(0);
  const refused = { error: "version_skew", document: "9.0.0", engine: host.engine.version };
  expect(await reasonOf(worked, () => worker.load(newer))).toEqual(refused);
  expect(await reasonOf(worked, () => worker.projectText("j_vendor_eval", summary))).toEqual(refused);
  expect(await reasonOf(worked, () => worker.load(text))).toEqual(refused);

  const paged = await fresh();
  const page = await paged.host.InBrowserHost.start(wasmBytes, () => NOW);
  expect(page.engine.touchedText(request.patch).length).toBeGreaterThan(0);
  expect(page.engine.accepts({ engine_version: "9.0.0" })).toBe(false);
  const actor = { user: "u_local" } as Schema<"Actor">;
  expect(await reasonOf(paged, () => page.engine.touchedText(request.patch))).toEqual(refused);
  expect(await reasonOf(paged, () => page.engine.apply(text, { patch: request.patch, at: NOW, actor }))).toEqual(refused);
  expect(await reasonOf(paged, () => page.engine.preview(text, { proposal: {} as Schema<"Proposal">, at: NOW, actor }))).toEqual(refused);
  expect(await reasonOf(paged, () => page.patch(request))).toEqual(refused);
});

it("a trap stops the page's module for every caller", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmBytes, () => NOW);
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
  expect(await reasonOf(started, () => host.engine.touchedText(note("p_after", 1, "a_after").patch))).toEqual(aborted);
});

it("the worker keeps a journey's newest document and refuses an older one", async () => {
  const started = await fresh();
  const host = await started.host.InBrowserHost.start(wasmBytes, () => NOW);
  const worker = await started.worker();
  const earlier = host.documentText("j_vendor_eval");
  const base = (await worker.load(earlier)).revision;
  host.patch(note("p_ordering", base, "a_ordering"));
  expect((await worker.load(host.documentText("j_vendor_eval"))).revision).toBe(base + 1);
  expect(await reasonOf(started, () => worker.load(earlier))).toMatchObject({ error: "superseded", held: { revision: base + 1 }, posted: { revision: base } });
  expect((await worker.load(host.documentText("j_vendor_eval"))).revision).toBe(base + 1);
});

it("a derive worker that fails rejects what was asked of it", async () => {
  const started = await fresh();
  expect(await reasonOf(started, () => started.host.DeriveWorker.start("module.wasm", brokenWorker()))).toMatchObject({ error: "aborted" });
});
