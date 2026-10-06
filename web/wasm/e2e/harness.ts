// The page the browser tests drive (wasm.spec.ts): each check runs here, in Chromium, against
// the wasm build (the derive worker's instance, and this page's for the root and the engine's
// worker-free calls), and answers a small summary the test asserts on and the proof reports.
import type { Schema } from "@cairn/client";

import {
  DeriveWorker,
  HostFailure,
  InBrowserHost,
  hosted,
  loadEngine,
  type Engine,
  type HostError,
  type PatchRequest,
  type ProjectionRequest,
} from "../src/index.ts";
import { NOW, type Case, type CaseResult, type Group, type GroupResult } from "./cases.ts";

const wasm = new URL("../dist/bindgen/cairn_wasm_bg.wasm", import.meta.url);

async function fetched<T>(path: string): Promise<T> {
  const response = await fetch(path);
  if (!response.ok) {
    throw new Error(`${path}: ${String(response.status)}`);
  }
  return (await response.json()) as T;
}

/** Runs one case: the worker for what reads its held document, this page for the rest. */
function answer(worker: DeriveWorker, engine: Engine, journey: string, call: Case["call"]): Promise<string> | string {
  switch (call.kind) {
    case "derive":
      return worker.derivedText(journey);
    case "project":
      return worker.projectText(journey, call.request);
    case "preview":
      return worker.previewText(journey, call.request);
    case "apply":
      return worker.applyText(journey, call.request);
    case "export":
      return engine.exportRouteText(call.request);
    case "import":
      return engine.importRouteText(call.request);
    case "touched":
      return engine.touchedText(call.patch);
  }
}

function compared(found: Case, answered: string): CaseResult {
  const result: CaseResult = { name: found.name, kind: found.call.kind, bytes: answered.length, equal: answered === found.expected };
  if (!result.equal) {
    let at = 0;
    while (at < answered.length && answered[at] === found.expected[at]) {
      at += 1;
    }
    result.firstDifference = at;
  }
  return result;
}

/** Every case of one group: the server's document loaded into a fresh derive worker. */
async function runGroup(file: string): Promise<GroupResult> {
  const group = await fetched<Group>(`/dist/cases/${file}`);
  const engine = await loadEngine(wasm);
  const worker = await DeriveWorker.start(wasm);
  try {
    const journey = group.document === undefined ? "" : (await worker.load(group.document)).journey;
    const results: CaseResult[] = [];
    for (const found of group.cases) {
      try {
        results.push(compared(found, await answer(worker, engine, journey, found.call)));
      } catch (thrown) {
        const error = thrown instanceof HostFailure ? JSON.stringify(thrown.reason) : String(thrown);
        results.push({ name: found.name, kind: found.call.kind, bytes: 0, equal: false, error });
      }
    }
    return { label: group.label, documentBytes: group.document?.length ?? 0, results };
  } finally {
    worker.terminate();
  }
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

function reasonOf(call: () => unknown): HostError | undefined {
  try {
    call();
    return undefined;
  } catch (thrown) {
    return thrown instanceof HostFailure ? thrown.reason : { error: "aborted", message: String(thrown) };
  }
}

function settle(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

/** What a subscriber heard. */
interface Heard {
  first: boolean;
  ticks: Schema<"Tick">[];
}

/** Each fixture's journey loaded from the root and derived in the worker. */
async function loadEach(host: InBrowserHost, worker: DeriveWorker): Promise<{ journey: string; revision: number; derivedBytes: number; frontier: number }[]> {
  const loaded = [];
  for (const summary of host.journeys().items) {
    const key = await worker.load(host.documentText(summary.id));
    const derived = await worker.derivedText(summary.id);
    const frontier = (await worker.derived(summary.id)).acting_frontier.length;
    loaded.push({ journey: key.journey, revision: key.revision, derivedBytes: derived.length, frontier });
  }
  return loaded;
}

/** The in-browser root: each fixture loads, a patch applies and is heard, stale ones are refused. */
async function rootScenario(): Promise<Record<string, unknown>> {
  const host = await InBrowserHost.start(wasm, () => NOW);
  const worker = await DeriveWorker.start(wasm);
  const loaded = await loadEach(host, worker);
  const heard: Heard[] = [];
  const unsubscribe = host.subscribe(["journey:j_vendor_eval"], (ticks, first) => heard.push({ ticks, first }));
  await settle(0);
  const base = (await worker.load(host.documentText("j_vendor_eval"))).revision;
  const applied = host.patch(note("p_browser_first", base, "a_browser_first"));
  const overlapping = note("p_browser_second", base, "a_browser_first");
  const separate = note("p_browser_third", base, "a_browser_third");
  const stale = [overlapping, separate].map((request) => {
    const reason = reasonOf(() => host.patch(request));
    const intervening = reason?.error === "rejected" && reason.rejection.rejection === "stale" ? reason.rejection.intervening : undefined;
    return { patch: request.patch.id, reason, overlaps: intervening === undefined ? undefined : host.engine.touchedOverlaps(request.patch, intervening) };
  });
  // H5: the separate one is safe to resubmit as is, at the revision the rejection named.
  const resubmitted = host.patch({ patch: { ...separate.patch, base_revision: base + 1 } });
  await settle(400);
  const after = await worker.load(host.documentText("j_vendor_eval"));
  unsubscribe();
  worker.terminate();
  return { capabilities: host.capabilities(), loaded, base, applied, stale, resubmitted, heard, after };
}

async function reasonOfAsync(call: () => Promise<unknown>): Promise<HostError | undefined> {
  try {
    await call();
    return undefined;
  } catch (thrown) {
    return thrown instanceof HostFailure ? thrown.reason : { error: "aborted", message: String(thrown) };
  }
}

/**
 * Version skew latches (ARCHITECTURE, Web UI): once the worker is posted a document from
 * another engine it refuses everything, the journey it held before included; once the page's
 * engine sees one, it refuses touched sets, local applies, previews, and the root's writes.
 */
async function skew(): Promise<Record<string, unknown>> {
  const host = await InBrowserHost.start(wasm, () => NOW);
  const worker = await DeriveWorker.start(wasm);
  const text = host.documentText("j_vendor_eval");
  const document = JSON.parse(text) as Schema<"DomainDocument">;
  const newer = JSON.stringify({ ...document, engine_version: "9.0.0" });
  const actor = { user: "u_local" } as Schema<"Actor">;
  const request = note("p_skewed", document.journey.revision, "a_skewed");
  const summary: ProjectionRequest = { projection: "status_summary" };
  await worker.load(text);
  const before = {
    project: (await worker.projectText("j_vendor_eval", summary)).length > 0,
    touched: host.engine.touchedText(request.patch).length > 0,
  };
  const workerRefusals = {
    load: await reasonOfAsync(() => worker.load(newer)),
    project: await reasonOfAsync(() => worker.projectText("j_vendor_eval", summary)),
    reload: await reasonOfAsync(() => worker.load(text)),
  };
  worker.terminate();
  const accepts = host.engine.accepts({ engine_version: "9.0.0" });
  const pageRefusals = {
    touched: reasonOf(() => host.engine.touchedText(request.patch)),
    apply: reasonOf(() => host.engine.apply(text, { patch: request.patch, at: NOW, actor })),
    preview: reasonOf(() => host.engine.preview(text, { proposal: {} as Schema<"Proposal">, at: NOW, actor })),
    patch: reasonOf(() => host.patch(request)),
  };
  return { engine: host.engine.version, before, accepts, worker: workerRefusals, page: pageRefusals };
}

/** A trap stops the page's module for every caller: the root and the engine alike. */
async function abort(): Promise<Record<string, unknown>> {
  const host = await InBrowserHost.start(wasm, () => NOW);
  const before = host.capabilities().auth.length > 0;
  // A trap reaches JavaScript as a RuntimeError thrown out of a call into the module.
  const trap = reasonOf(() => hosted(() => {
    throw new WebAssembly.RuntimeError("unreachable");
  }));
  return {
    before,
    trap,
    capabilities: reasonOf(() => host.capabilities()),
    journeys: reasonOf(() => host.journeys()),
    touched: reasonOf(() => host.engine.touchedText(note("p_after", 1, "a_after").patch)),
  };
}

/** The worker keeps the newest document of a journey: an older one posted late is refused. */
async function ordering(): Promise<Record<string, unknown>> {
  const host = await InBrowserHost.start(wasm, () => NOW);
  const worker = await DeriveWorker.start(wasm);
  const earlier = host.documentText("j_vendor_eval");
  const base = (await worker.load(earlier)).revision;
  host.patch(note("p_ordering", base, "a_ordering"));
  const later = await worker.load(host.documentText("j_vendor_eval"));
  const refused = await reasonOfAsync(() => worker.load(earlier));
  const derived = await worker.derivedText("j_vendor_eval");
  const held = (await worker.load(host.documentText("j_vendor_eval"))).revision;
  worker.terminate();
  return { base, later: later.revision, refused, held, derivedBytes: derived.length };
}

/** A worker that fails rejects what was asked of it instead of leaving it pending. */
async function deadWorker(): Promise<HostError | undefined> {
  const script = URL.createObjectURL(new Blob(['throw new Error("broken");'], { type: "text/javascript" }));
  return reasonOfAsync(() => DeriveWorker.start(wasm, new Worker(script, { type: "module" })));
}

function median(times: number[]): number {
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

/** The derive benchmark at the limits, in the worker, timed from this page: medians of `runs`. */
async function bench(file: string, runs: number): Promise<Record<string, number>> {
  const group = await fetched<Group>(`/dist/cases/${file}`);
  const document = group.document ?? "";
  const worker = await DeriveWorker.start(wasm);
  const timed = async (work: () => Promise<unknown>): Promise<number> => {
    const times: number[] = [];
    for (let run = 0; run < runs; run += 1) {
      const started = performance.now();
      await work();
      times.push(performance.now() - started);
    }
    return median(times);
  };
  const journey = (await worker.load(document)).journey;
  const level: ProjectionRequest = { projection: "level", shown: ["group", "action", "deliverable", "decision", "milestone"] };
  const result = {
    runs,
    documentBytes: document.length,
    derivedBytes: (await worker.derivedText(journey)).length,
    readAndDeriveMs: await timed(() => worker.load(document)),
    derivedToJsonMs: await timed(() => worker.derivedText(journey)),
    levelMs: await timed(() => worker.projectText(journey, level)),
    memoryBytes: await worker.memoryBytes(),
  };
  worker.terminate();
  return result;
}

const harness = { runGroup, rootScenario, skew, abort, ordering, deadWorker, bench };

declare global {
  interface Window {
    cairnHarness: typeof harness;
  }
}

window.cairnHarness = harness;
