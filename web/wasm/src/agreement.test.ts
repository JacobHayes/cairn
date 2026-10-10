// Brief 4.5, Acceptance, in Node over the wasm32 module: the derive worker's derive,
// every projection, previews, local applies, and route files agree with the server's answers
// byte for byte for every fixture. Node's V8 runs the same wasm32 binary a browser does, which
// is what catches drift only that target has (usize, floats, the stack); the worker in a real
// browser is the app's smoke test. The server's answers are the cases the server walk wrote
// (the cairn-wasm-cases binary).
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, it, vi } from "vitest";

import { type Case, type Group, type IndexEntry } from "./cases.ts";
import type { DeriveWorker, Engine } from "./index.ts";
import { inProcess, wasmBytes } from "./in-process.ts";

const cases = join(import.meta.dirname, "..", "dist", "cases");

/** A page and a worker of their own: the modules loaded again, with nothing latched. */
function fresh() {
  vi.resetModules();
  return inProcess();
}
const index = JSON.parse(readFileSync(join(cases, "index.json"), "utf8")) as IndexEntry[];

/** The answer a case's call gets here: the worker for what reads its held document, the page's engine for the rest. */
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

it.each(index)("the browser host agrees with the server: $label", async (entry) => {
  const group = JSON.parse(readFileSync(join(cases, entry.file), "utf8")) as Group;
  const started = await fresh();
  const worker = await started.worker();
  const engine = await started.host.loadEngine(wasmBytes);
  const journey = group.document === undefined ? "" : (await worker.load(group.document)).journey;
  const differing: string[] = [];
  for (const found of group.cases) {
    const answered = await Promise.resolve(answer(worker, engine, journey, found.call)).catch((thrown: unknown) => `error: ${String(thrown)}`);
    if (answered !== found.expected) {
      let at = 0;
      while (at < answered.length && answered[at] === found.expected[at]) {
        at += 1;
      }
      differing.push(`${found.name}: differs from the server at byte ${String(at)}`);
    }
  }
  expect(differing, `${entry.label}: calls that differ from the server`).toEqual([]);
  expect(group.cases.length).toBe(entry.cases);
  worker.terminate();
});
