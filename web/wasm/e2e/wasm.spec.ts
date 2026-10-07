// Brief 4.5, Acceptance, in Chromium against the wasm build (rung 6): the derive worker's
// derive, every projection, previews, local applies, and route files agree with the server's
// answers byte for byte for every fixture; the in-browser root loads each fixture, applies a
// patch, refuses stale ones, and notifies its subscriber; a document from another engine
// version, or a trap, stops the module; the worker keeps a journey's newest document and
// fails loudly; and the derive benchmark at the limits runs in the browser. Set
// CAIRN_PROOF_OUT to a directory to keep the benchmark's figures, for the proof's table.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import type { GroupResult, IndexEntry } from "./cases.ts";

const cases = join(import.meta.dirname, "..", "dist", "cases");
const index = JSON.parse(readFileSync(join(cases, "index.json"), "utf8")) as IndexEntry[];

/** The journeys the server walk read: every fixture's, as the root seeds them. */
function serverJourneys(): string[] {
  const journeys = new Set<string>();
  for (const entry of index.filter((found) => found.label !== "at the limits")) {
    const group = JSON.parse(readFileSync(join(cases, entry.file), "utf8")) as { document?: string };
    if (group.document !== undefined) {
      const document = JSON.parse(group.document) as { journey: { header: { id: string } } };
      journeys.add(document.journey.header.id);
    }
  }
  return [...journeys].sort();
}

/** Keeps the benchmark's figures when the proof asks for them. */
function keep(name: string, found: unknown): void {
  const out = process.env["CAIRN_PROOF_OUT"];
  if (out !== undefined) {
    mkdirSync(out, { recursive: true });
    writeFileSync(join(out, `${name}.json`), JSON.stringify(found, null, 2));
  }
}

async function harness(page: Page): Promise<void> {
  await page.goto("/e2e/index.html");
  await page.waitForFunction(() => "cairnHarness" in window);
}

test.beforeEach(async ({ page }) => {
  page.on("console", (message) => {
    if (message.type() === "error") {
      console.error(`page: ${message.text()}`);
    }
  });
  await harness(page);
});

for (const entry of index) {
  test(`the browser host agrees with the server: ${entry.label}`, async ({ page }) => {
    const found: GroupResult = await page.evaluate((file) => window.cairnHarness.runGroup(file), entry.file);
    const differing = found.results.filter((result) => !result.equal);
    expect(differing, `${entry.label}: calls that differ from the server`).toEqual([]);
    expect(found.results.length).toBe(entry.cases);
  });
}

test("the in-browser root loads each fixture, applies a patch, refuses stale ones, and notifies", async ({ page }) => {
  const found = await page.evaluate(() => window.cairnHarness.rootScenario());
  const loaded = found["loaded"] as { journey: string; derivedBytes: number }[];
  expect(loaded.map((journey) => journey.journey).sort()).toEqual(serverJourneys());
  expect(loaded.every((journey) => journey.derivedBytes > 0)).toBe(true);
  const base = found["base"] as number;
  expect(found["applied"]).toMatchObject({ outcome: "applied", receipt: { revision: base + 1 } });
  expect(found["stale"]).toEqual([
    expect.objectContaining({ reason: expect.objectContaining({ error: "rejected", rejection: expect.objectContaining({ rejection: "stale" }) }), overlaps: true }),
    expect.objectContaining({ reason: expect.objectContaining({ error: "rejected", rejection: expect.objectContaining({ rejection: "stale" }) }), overlaps: false }),
  ]);
  expect(found["resubmitted"]).toMatchObject({ outcome: "applied", receipt: { revision: base + 2 } });
  const heard = found["heard"] as { first: boolean; ticks: { of: unknown; revision: number }[] }[];
  const vendor = { domain: { journey: "j_vendor_eval" } };
  expect(heard[0]).toMatchObject({ first: true, ticks: [{ of: vendor, revision: base }] });
  expect(heard.at(-1)).toMatchObject({ first: false, ticks: [{ of: vendor, revision: base + 2 }] });
  expect(found["after"]).toMatchObject({ journey: "j_vendor_eval", revision: base + 2 });
});

test("a document from another engine version stops deriving, previewing, writing, and retrying", async ({ page }) => {
  const found = await page.evaluate(() => window.cairnHarness.skew());
  const refused = { error: "version_skew", document: "9.0.0", engine: found["engine"] };
  expect(found["before"]).toEqual({ project: true, touched: true });
  expect(found["accepts"]).toBe(false);
  expect(found["worker"]).toEqual({ load: refused, project: refused, reload: refused });
  expect(found["page"]).toEqual({ touched: refused, apply: refused, preview: refused, patch: refused });
});

test("a trap stops the page's module for every caller", async ({ page }) => {
  const found = await page.evaluate(() => window.cairnHarness.abort());
  expect(found["before"]).toBe(true);
  const aborted = { error: "aborted", message: "unreachable" };
  expect(found).toMatchObject({ trap: aborted, capabilities: aborted, journeys: aborted, touched: aborted });
});

test("the worker keeps a journey's newest document and refuses an older one", async ({ page }) => {
  const found = await page.evaluate(() => window.cairnHarness.ordering());
  const base = found["base"] as number;
  expect(found["later"]).toBe(base + 1);
  expect(found["refused"]).toMatchObject({ error: "superseded", held: { revision: base + 1 }, posted: { revision: base } });
  expect(found["held"]).toBe(base + 1);
});

test("a derive worker that fails rejects what was asked of it", async ({ page }) => {
  const found = await page.evaluate(() => window.cairnHarness.deadWorker());
  expect(found).toMatchObject({ error: "aborted" });
});

test("the derive benchmark at the limits runs in the worker", async ({ page }) => {
  const limits = index.at(-1);
  expect(limits?.label).toBe("at the limits");
  const found = await page.evaluate((file) => window.cairnHarness.bench(file, 5), limits?.file ?? "");
  keep("bench-browser", found);
  // Reported, not gated (ARCHITECTURE, Date network): only that it ran.
  expect(found["readAndDeriveMs"]).toBeGreaterThan(0);
  expect(found["memoryBytes"]).toBeGreaterThan(0);
});
