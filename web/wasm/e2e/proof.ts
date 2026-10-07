// Prints the derive benchmark table of briefs/proof/4.5/README.md from what the browser
// benchmark kept (CAIRN_PROOF_OUT) and the native figures the server walk wrote: run by
// briefs/proof/4.5/prove.sh with Node, which runs TypeScript as is. Prints Markdown on stdout.
//
// usage: node web/wasm/e2e/proof.ts FOUND_DIR CASES_DIR
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { parsed } from "../src/types.ts";

const [found, cases] = process.argv.slice(2);
if (found === undefined || cases === undefined) {
  throw new Error("usage: node web/wasm/e2e/proof.ts FOUND_DIR CASES_DIR");
}

function figures(path: string): Record<string, number> {
  return parsed<Record<string, number>>(readFileSync(path, "utf8"));
}

const native = figures(join(cases, "bench-native.json"));
const browser = figures(join(found, "bench-browser.json"));
const ms = (value: number | undefined): string => `${(value ?? 0).toFixed(1)} ms`;
const mib = (bytes: number | undefined): string => `${((bytes ?? 0) / 1024 / 1024).toFixed(0)} MiB`;
console.log(
  [
    `| Measure, median of ${String(browser["runs"])} | Native (release) | Browser (Chromium, derive worker) |`,
    "|---|---|---|",
    `| read the document (${String(native["document_bytes"])} bytes) and derive it | ${ms(native["read_and_derive_ms"])} | ${ms(browser["readAndDeriveMs"])} |`,
    `| every derived value to JSON (${String(browser["derivedBytes"])} bytes) | ${ms(native["derived_to_json_ms"])} | ${ms(browser["derivedToJsonMs"])} |`,
    `| the top canvas level | ${ms(native["level_ms"])} | ${ms(browser["levelMs"])} |`,
    `| memory | ${mib((native["peak_resident_kib"] ?? 0) * 1024)} peak resident | ${mib(browser["memoryBytes"])} wasm linear memory |`,
  ].join("\n"),
);
