// Light and dark lay out the same (DESIGN, Dark theme): a theme changes colours, never a box. The
// page is read as it stands, put in the other theme, and read again; every drawn element must be
// where it was, at the size it was. Layout that differs by theme (a width set in one, a border
// that exists in one) shows as a moved or resized box.
import type { Page } from "@playwright/test";

import { nameOf } from "./containment.ts";
import { settled } from "./shell.ts";
import { snapshotOf, upFrom, type Snapshot } from "./snapshot.ts";

/** How far a box may differ between themes (px): rounding of the same layout. */
const TOLERANCE_PX = 0.5;

/** The most differences one report lists. */
const LISTED_MAX = 10;

/** Where `now` differs from `before`: one line for each element that is drawn in one theme and not the other, or has a different box. */
export function themeDifferences(light: Snapshot, dark: Snapshot): string[] {
  // The sync chip's words come and go with the clock (SAVED shows for a moment after a save), which is not the theme's doing; the chip's popover is compared.
  const kept = (snapshot: Snapshot): Snapshot => ({ ...snapshot, nodes: snapshot.nodes.filter((part) => ![...upFrom(part)].some((up) => up.attr["data-testid"] === "sync")) });
  const [before, now] = [kept(light), kept(dark)];
  if (before.nodes.length !== now.nodes.length) {
    // Which elements are in one theme and not the other (by name, with how many more).
    const count = new Map<string, number>();
    for (const part of before.nodes) {
      count.set(nameOf(part), (count.get(nameOf(part)) ?? 0) + 1);
    }
    for (const part of now.nodes) {
      count.set(nameOf(part), (count.get(nameOf(part)) ?? 0) - 1);
    }
    const unmatched = [...count].filter(([, by]) => by !== 0).map(([name, by]) => `${name} x${String(-by)}`);
    return [`the dark theme has ${String(now.nodes.length)} nodes where the light has ${String(before.nodes.length)} (${unmatched.slice(0, LISTED_MAX).join("; ")})`];
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
