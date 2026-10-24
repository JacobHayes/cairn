// The tooltip invariants every browser test ends with (e2e/test.ts), and the helpers its spec
// shares. There is one tooltip, the app's (ui/Tip.tsx): the browser's own, from a `title`
// attribute or an SVG `<title>`, is gone from every page whatever put it there (a component, a
// library), and a tip that is not open is never drawn (an author `display` beats the browser's
// rule that hides a closed popover, and a closed tip taking room in a row is the symptom).
import { expect, type Locator, type Page } from "@playwright/test";

import { upFrom, type Snapshot } from "./snapshot.ts";

/** One line per `title` attribute or SVG `<title>` on `snapshot`: the browser would show its own tooltip, where the app's belongs. */
export function titleProblems(snapshot: Snapshot): string[] {
  return snapshot.nodes
    .filter((part) => ("title" in part.attr || part.tag === "title") && ![...upFrom(part)].some((up) => up.tag === "head"))
    .map((part) => `a native title, where the app's tooltip belongs: <${part.tag}${"title" in part.attr ? ` title="${(part.attr["title"] ?? "").slice(0, 60)}"` : ""}>`);
}

/** Fails when a tip that is not open has a box: it is drawn, or takes room. */
export async function expectNoClosedTipDrawn(page: Page): Promise<void> {
  if (page.isClosed()) {
    return;
  }
  const drawn = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('[role="tooltip"]')]
      .filter((each) => !each.matches(":popover-open") && (each.getClientRects().length > 0 || getComputedStyle(each).display !== "none"))
      .map((each) => each.outerHTML.slice(0, 140)),
  );
  expect(drawn, "a closed tooltip that is drawn").toEqual([]);
}

/** Fails when a tip is open: a picture of a state shows only what the state is, so a tip nobody asked for (left by a focus a script moved) would mislead whoever reads it. */
export async function expectNoTipOpen(page: Page): Promise<void> {
  if (page.isClosed()) {
    return;
  }
  const open = await page.evaluate(() => [...document.querySelectorAll('[role="tooltip"]:popover-open')].map((each) => each.textContent.slice(0, 80)));
  expect(open, "a tooltip open in a picture").toEqual([]);
}

/** The tip that is open now. */
export const openTip = (page: Page): Locator => page.locator('[role="tooltip"]:popover-open');

/** The tip is open, with its box wholly inside the window by at least `margin` px. */
export async function expectTipInWindow(page: Page, margin = 8): Promise<void> {
  const tip = openTip(page);
  await expect(tip).toBeVisible();
  const box = await tip.evaluate((element) => {
    const { left, right, top, bottom } = element.getBoundingClientRect();
    return { left, top, right: document.documentElement.clientWidth - right, bottom: window.innerHeight - bottom };
  });
  for (const [side, room] of Object.entries(box)) {
    expect(room, `the tip's ${side} edge is ${String(room)}px from the window's`).toBeGreaterThanOrEqual(margin - 0.5);
  }
}
