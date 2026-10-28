// The invariants every page a browser test opened, and every picture taken of one, must hold:
// nothing spills out of its box (e2e/containment.ts), no key is shown where a name belongs
// (e2e/internal-ids.ts), the only tooltip is the app's (e2e/tips.ts), on a touch screen every
// control has the touch-sized hit area (e2e/touch.ts), no text is under the type floor or the
// contrast floor (e2e/contrast.ts), the document does not scroll down at a desktop width and nothing
// is drawn past the window's edge (e2e/edges.ts), all read from Chrome's layout snapshot (e2e/snapshot.ts, e2e/structure.ts). `test.ts` runs them
// after every test; a screenshot (a proof spec's, or the `mise run screenshots` walk's) goes through
// `capture`, which writes the picture only when they pass and no tip is open.
import { writeFileSync } from "node:fs";

import type { Page } from "@playwright/test";

import { ariaOf } from "./aria.ts";

import { expectStructure, type Observed } from "./structure.ts";
import { expectNoClosedTipDrawn, expectNoTipOpen } from "./tips.ts";

/**
 * The invariants (the structural rules over Chrome's layout snapshot: containment, no internal key shown, no native tooltip, touch-sized controls on a touch screen, the type and contrast floors, the window's edges; and no closed tip drawn) on `page` as it is now. They run once at the
 * end of a test body, so a test that walks through several states, each one a test of its own
 * before they were merged, calls this at the ones that are not its end: a state that spills
 * out of its box, or shows a key where a name belongs, fails where it was reached. Returns what it read, for `capture`.
 */
export async function checkpoint(page: Page, where = "the page"): Promise<Observed | undefined> {
  const seen = await expectStructure(page, where);
  await expectNoClosedTipDrawn(page);
  return seen;
}

/**
 * A screenshot of `page` for a proof or a picture, taken only when it passes the invariants. With
 * `structure`, the rules' results are saved beside it: `<path minus .png>.checks.json` (what
 * passed, and what came near a rule) and `.aria.yml` (the trimmed accessibility snapshot). They
 * are build output, never committed.
 */
export async function capture(page: Page, path: string, options: { fullPage?: boolean; structure?: boolean; dry?: boolean } = {}): Promise<void> {
  const seen = await checkpoint(page, path);
  await expectNoTipOpen(page);
  const { structure, dry, ...shot } = options;
  // `dry`: the checks have run, and the picture and its files are not wanted.
  if (dry === true) {
    return;
  }
  await page.screenshot({ path, ...shot });
  if (structure === true && seen !== undefined) {
    const base = path.replace(/\.png$/, "");
    writeFileSync(`${base}.checks.json`, `${JSON.stringify({ passed: Object.keys(seen.problems), near: seen.near }, null, 2)}\n`);
    writeFileSync(`${base}.aria.yml`, `${await ariaOf(page)}\n`);
  }
}
