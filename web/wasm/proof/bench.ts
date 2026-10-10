// Prints the derive benchmark table of briefs/proof/4.5/README.md: the document at the limits
// read and derived natively (the cairn-wasm-cases binary, optimized, which wrote
// bench-native.json) and by the derive worker's own script over the wasm32 module in Node,
// and the graph's size figures over a generated 2,000-node journey. Run by
// briefs/proof/4.5/prove.sh with Node, which runs TypeScript as is. Reported, never gated
// (ARCHITECTURE, Date network). Prints Markdown on stdout.
//
// usage: node web/wasm/proof/bench.ts CASES_DIR
import { readFileSync } from "node:fs";
import { join } from "node:path";

import type { Group, IndexEntry } from "../src/cases.ts";
import { inProcess } from "../src/in-process.ts";
import type { ProjectionRequest } from "../src/index.ts";
import { parsed } from "../src/types.ts";

const [cases] = process.argv.slice(2);
if (cases === undefined) {
  throw new Error("usage: node web/wasm/proof/bench.ts CASES_DIR");
}
const RUNS = 5;

const index = parsed<IndexEntry[]>(readFileSync(join(cases, "index.json"), "utf8"));
const group = (label: string): Group => {
  const found = index.find((entry) => entry.label === label);
  return parsed<Group>(readFileSync(join(cases, found?.file ?? label), "utf8"));
};

/** The median of `runs` timings of `work`, after one warm-up. */
async function median(work: () => Promise<unknown>, runs: number): Promise<number> {
  await work();
  const times: number[] = [];
  for (let run = 0; run < runs; run += 1) {
    const started = performance.now();
    await work();
    times.push(performance.now() - started);
  }
  return times.sort((a, b) => a - b)[Math.floor(times.length / 2)] ?? 0;
}

const { worker: start } = await inProcess();
const worker = await start();
const limits = group("at the limits").document ?? "";
const journey = (await worker.load(limits)).journey;
const level: ProjectionRequest = { projection: "level", shown: ["group", "action", "deliverable", "decision", "milestone"] };
const found = {
  derivedBytes: (await worker.derivedText(journey)).length,
  readAndDeriveMs: await median(() => worker.load(limits), RUNS),
  derivedToJsonMs: await median(() => worker.derivedText(journey), RUNS),
  levelMs: await median(() => worker.projectText(journey, level), RUNS),
  memoryBytes: await worker.memoryBytes(),
};

// The size figures (design 5.11): the level with containers collapsed and a relevance class
// left out, and the trace on selection, over the generated 2,000-node journey's own cases.
const big = group("2000 generated nodes");
const bigJourney = (await worker.load(big.document ?? "")).journey;
const timedCalls = async (select: (request: ProjectionRequest) => boolean): Promise<number[]> => {
  const times: number[] = [];
  for (const { call } of big.cases) {
    if (call.kind === "project" && select(call.request)) {
      times.push(await median(() => worker.projectText(bigJourney, call.request), 9));
    }
  }
  return times;
};
const levelTimes = await timedCalls((request) => request.projection === "level");
const traceTimes = await timedCalls((request) => request.projection === "trace");
worker.terminate();

const native = parsed<Record<string, number>>(readFileSync(join(cases, "bench-native.json"), "utf8"));
const ms = (value: number | undefined): string => `${(value ?? 0).toFixed(1)} ms`;
const mib = (bytes: number | undefined): string => `${((bytes ?? 0) / 1024 / 1024).toFixed(0)} MiB`;
console.log(
  [
    `| Measure, median of ${String(RUNS)} | Native (release) | Node (V8, derive worker, wasm32) |`,
    "|---|---|---|",
    `| read the document (${String(native["document_bytes"])} bytes) and derive it | ${ms(native["read_and_derive_ms"])} | ${ms(found.readAndDeriveMs)} |`,
    `| every derived value to JSON (${String(found.derivedBytes)} bytes) | ${ms(native["derived_to_json_ms"])} | ${ms(found.derivedToJsonMs)} |`,
    `| the top canvas level | ${ms(native["level_ms"])} | ${ms(found.levelMs)} |`,
    `| memory | ${mib((native["peak_resident_kib"] ?? 0) * 1024)} peak resident | ${mib(found.memoryBytes)} wasm linear memory |`,
    "",
    `At 2,000 generated nodes the slowest level takes ${ms(Math.max(...levelTimes))} and the trace ${ms(Math.max(...traceTimes))} (median of 9).`,
  ].join("\n"),
);
