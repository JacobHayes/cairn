// Light and dark lay out the same (DESIGN, Dark theme): a theme changes colours, never a box. The
// page is read as it stands, put in the other theme, and read again; every drawn element must be
// where it was, at the size it was. Layout that differs by theme (a width set in one, a border
// that exists in one) shows as a moved or resized box.
import type { Page } from "@playwright/test";

import { nameOf } from "./containment.ts";
import { settled } from "./shell.ts";
import { snapshotOf, type Snapshot } from "./snapshot.ts";

/** How far a box may differ between themes (px): rounding of the same layout. */
const TOLERANCE_PX = 0.5;

/** The most differences one report lists. */
const LISTED_MAX = 10;

/** Where `now` differs from `before`: one line for each element that is drawn in one theme and not the other, or has a different box. */
export function themeDifferences(before: Snapshot, now: Snapshot): string[] {
  if (before.nodes.length !== now.nodes.length) {
    return [`the dark theme has ${String(now.nodes.length)} nodes where the light has ${String(before.nodes.length)}`];
  }
  const found = before.nodes.flatMap((part, index) => {
    const [a, b] = [part.box, now.nodes[index]?.box];
    if (a === undefined && b === undefined) {
      return [];
    }
    if (a === undefined || b === undefined) {
      return [`${nameOf(part)} is drawn in the ${a === undefined ? "dark" : "light"} theme only`];
    }
    const moved = ([a.x - b.x, a.y - b.y, a.width - b.width, a.height - b.height] as const).some((by) => Math.abs(by) > TOLERANCE_PX);
    return moved ? [`${nameOf(part)} is [${[b.x, b.y, b.width, b.height].map((n) => String(Math.round(n * 10) / 10)).join(", ")}] in the dark theme`] : [];
  });
  return found.length > LISTED_MAX ? [...found.slice(0, LISTED_MAX), `and ${String(found.length - LISTED_MAX)} more`] : found;
}

/** The page, in the light theme, against itself in the dark; the page is in the light theme again afterwards. */
export async function themeProblems(page: Page): Promise<string[]> {
  const light = await snapshotOf(page);
  await page.emulateMedia({ colorScheme: "dark" });
  try {
    await settled(page);
    return themeDifferences(light, await snapshotOf(page));
  } finally {
    await page.emulateMedia({ colorScheme: "light" });
  }
}
