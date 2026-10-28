// `mise run sweep`: the structural rules (e2e/invariants.ts, `checkpoint`) over every state of the
// screenshot table at the widths where the layout can change: the viewport thresholds the CSS's
// media queries name (read from the stylesheets, so a new one is swept with no edit here), each
// one and a pixel either side, and a spread of widths between them where the container queries
// switch and the content wraps. Light only (light and dark are held to the same layout by the
// screenshot walk). A phone's states are visited below the desktop threshold and the desktop's
// from it. The browser step runs it on every push (decisions/).
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "./test.ts";

import { checkpoint } from "./invariants.ts";
import { walkStates } from "./screenshots.ts";

/** The first width a desktop's layout applies at, and the height every width is swept at. */
const DESKTOP_MIN = 720;
const HEIGHT = 900;

/** Widths between the thresholds: a phone, where the container queries of the list and Summary switch, a desktop, and a wide screen. */
const BETWEEN = [390, 560, 810, 960, 1440, 2560];

/** Every `min-width` and `max-width` a media query in the app's stylesheets names. */
function thresholds(): number[] {
  const files = ["../../../design", "../src"].flatMap((root) => {
    const base = fileURLToPath(new URL(root, import.meta.url));
    return readdirSync(base, { recursive: true, encoding: "utf8" }).filter((each) => each.endsWith(".css")).map((each) => join(base, each));
  });
  const found = new Set<number>();
  for (const file of files) {
    for (const match of readFileSync(file, "utf8").matchAll(/@media[^{]*\((?:min|max)-width:\s*(\d+)px\)/g)) {
      found.add(Number(match[1]));
    }
  }
  return [...found];
}

const WIDTHS = [...new Set([...thresholds().flatMap((width) => [width - 1, width, width + 1]), ...BETWEEN])].sort((a, b) => a - b);

for (const width of WIDTHS) {
  const phone = width < DESKTOP_MIN;
  test.describe(`sweep ${String(width)}px`, () => {
    // A narrow screen is a touch screen: the coarse-pointer rules apply.
    test.use({ viewport: { width, height: HEIGHT }, hasTouch: phone });
    test(`every state at ${String(width)}px`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
      const failed = await walkStates(page, { width, height: HEIGHT }, phone, async (address, state, reach) => {
        await reach();
        await checkpoint(page, `${address} ${state.name} at ${String(width)}px`);
      });
      expect(failed).toEqual([]);
    });
  });
}
