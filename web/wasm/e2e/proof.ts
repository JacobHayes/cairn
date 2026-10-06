// Writes the tables of briefs/proof/4.5/README.md from what the browser tests kept
// (CAIRN_PROOF_OUT) and the server walk's cases: run by briefs/proof/4.5/prove.sh with Node,
// which runs TypeScript as is. Prints Markdown on stdout; exits non-zero when anything the
// browser answered differs from the server.
//
// usage: node web/wasm/e2e/proof.ts FOUND_DIR CASES_DIR
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { parsed } from "../src/types.ts";
import type { CaseResult, Group, GroupResult, IndexEntry } from "./cases.ts";

const [foundArgument, casesArgument, section] = process.argv.slice(2);
if (foundArgument === undefined || casesArgument === undefined || section === undefined) {
  throw new Error("usage: node web/wasm/e2e/proof.ts FOUND_DIR CASES_DIR SECTION");
}
const found: string = foundArgument;
const cases: string = casesArgument;

function text(path: string): string {
  return readFileSync(path, "utf8");
}

function sha(text: string): string {
  return createHash("sha256").update(text).digest("hex").slice(0, 16);
}

function kinds(results: CaseResult[]): string {
  const counted = new Map<string, number>();
  for (const result of results) {
    counted.set(result.kind, (counted.get(result.kind) ?? 0) + 1);
  }
  return [...counted].map(([kind, count]) => `${kind} ${String(count)}`).join(", ");
}

let differing = 0;

function agreement(): string[] {
  const index = parsed<IndexEntry[]>(text(join(cases, "index.json")));
  const lines = [
    "| Document | Document bytes | Calls (by kind) | Bytes compared | Equal to the server | Derive's SHA-256 (server = browser) |",
    "|---|---|---|---|---|---|",
  ];
  for (const [position, entry] of index.entries()) {
    const result = parsed<GroupResult>(text(join(found, `group-${String(position).padStart(2, "0")}.json`)));
    const group = parsed<Group>(text(join(cases, entry.file)));
    const equal = result.results.filter((one) => one.equal).length;
    differing += result.results.length - equal;
    const compared = result.results.reduce((sum, one) => sum + one.bytes, 0);
    const derive = group.cases.find((one) => one.call.kind === "derive");
    const digest = derive === undefined ? "-" : `\`${sha(derive.expected)}\``;
    lines.push(
      `| ${entry.label} | ${String(result.documentBytes)} | ${kinds(result.results)} | ${String(compared)} | ${String(equal)} of ${String(result.results.length)} | ${digest} |`,
    );
  }
  return lines;
}

function fenced(value: unknown): string[] {
  return ["```json", JSON.stringify(value, null, 2), "```"];
}

interface Loaded {
  journey: string;
  revision: number;
  derivedBytes: number;
  frontier: number;
}

function root(): string[] {
  const scenario = parsed<Record<string, unknown>>(text(join(found, "root.json")));
  const loaded = scenario["loaded"] as Loaded[];
  const lines = [
    "| Journey | Revision | Derived bytes (in the worker) | Acting frontier |",
    "|---|---|---|---|",
    ...loaded.map((one) => `| \`${one.journey}\` | ${String(one.revision)} | ${String(one.derivedBytes)} | ${String(one.frontier)} |`),
    "",
    "The capabilities document the root answers:",
    "",
    ...fenced(scenario["capabilities"]),
    "",
    `A note on the vendor evaluation's kickoff from revision ${String(scenario["base"])}, applied:`,
    "",
    ...fenced(scenario["applied"]),
    "",
    "Two more patches from the same base, both stale. The first writes the same note, so what intervened overlaps it (`overlaps: true`) and it surfaces; the second writes its own note, so it is safe to resubmit as is (H5):",
    "",
    ...fenced(scenario["stale"]),
    "",
    "The second resubmitted at the revision its rejection named:",
    "",
    ...fenced(scenario["resubmitted"]),
    "",
    "What the subscriber to `journey:j_vendor_eval` heard: the current revision first, then one tick with the latest, since both commits landed within the 250 ms coalescing interval after the first take (H6):",
    "",
    ...fenced(scenario["heard"]),
  ];
  return lines;
}

function bench(): string[] {
  const native = parsed<Record<string, number>>(text(join(cases, "bench-native.json")));
  const browser = parsed<Record<string, number>>(text(join(found, "bench-browser.json")));
  const ms = (value: number | undefined): string => `${(value ?? 0).toFixed(1)} ms`;
  const mib = (bytes: number | undefined): string => `${((bytes ?? 0) / 1024 / 1024).toFixed(0)} MiB`;
  return [
    `| Measure, median of ${String(browser["runs"])} | Native (release, through crates/wasm) | Browser (Chromium, the derive worker's wasm, timed from the page) |`,
    "|---|---|---|",
    `| domain document | ${String(native["document_bytes"])} bytes | ${String(browser["documentBytes"])} bytes |`,
    `| read the document and derive it | ${ms(native["read_and_derive_ms"])} | ${ms(browser["readAndDeriveMs"])} |`,
    `| every derived value to JSON (${String(browser["derivedBytes"])} bytes) | ${ms(native["derived_to_json_ms"])} | ${ms(browser["derivedToJsonMs"])} |`,
    `| the top canvas level | ${ms(native["level_ms"])} | ${ms(browser["levelMs"])} |`,
    `| memory | peak resident ${mib((native["peak_resident_kib"] ?? 0) * 1024)} (the whole process, cases included) | wasm linear memory ${mib(browser["memoryBytes"])} (its peak: it never shrinks) |`,
  ];
}

const sections: Record<string, string[]> = {
  agreement: agreement(),
  root: root(),
  skew: fenced(parsed<unknown>(text(join(found, "skew.json")))),
  bench: bench(),
};
const lines = sections[section];
if (lines === undefined) {
  throw new Error(`no section ${section}`);
}
console.log(lines.join("\n"));
if (differing > 0) {
  console.error(`proof: ${String(differing)} calls differ from the server`);
  process.exit(1);
}
