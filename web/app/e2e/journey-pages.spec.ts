// A journey's two pages in a real browser (2.2 to 2.4), on the in-browser host: what only focus,
// layout and the window show. The keys act on the page the pointer left them on (j, Enter, Esc,
// v), and no page scrolls the window: the head stays, a popover taller than the room scrolls
// inside itself, the switcher keeps its row, and a canvas fills its region. The addresses,
// chips, filters and memory behind them are vitest's (journeys/address.test.ts and
// screens/filters.test.ts).
import { expect, test } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { nextItem, nextKeys } from "./acting.ts";
import { nodePanel, openAt, openFilter } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

const UP_FRONT = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];

test("C10, C11: after a click in the page, j opens the next row, Enter its form, Esc leaves it, and v swaps the projection", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Keys"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", id, "next/list");
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => nextKeys(page).then((keys) => [...keys].sort())).toEqual(UP_FRONT);

  // j opens the first row in the inspector, whose form is then the node's only one; Enter opens it.
  const [first = ""] = await nextKeys(page);
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("j");
  await expect(nodePanel(page, first)).toBeVisible();
  await expect(nextItem(page, first).getByTestId("acts")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(nodePanel(page, first).getByTestId("answer-editor")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(page.getByTestId("projection-cards")).toHaveAttribute("aria-current", "page");
  await expect(page).toHaveURL(/\/next\/cards\?decisions=1$/);
  await page.keyboard.press("v");
  await expect(page).toHaveURL(/\/next\/list\?decisions=1$/);
});

test("the window does not scroll: the head stays, a popover scrolls inside itself, the switcher keeps its row at one edge, and a canvas fills its region", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  for (const address of ["next/list", "plan/list"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("journey-toolbar")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollHeight), address).toBe(800);
  }
  // A popover taller than the room below it scrolls inside itself, not the window.
  await page.setViewportSize({ width: 1280, height: 500 });
  await openAt(page, "browser", "j_vendor_eval", "plan/list", { fixedToday: FIXED_TODAY });
  await openFilter(page);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(500);
  // The switcher sits on the tabs' row, at the same right edge on every projection, between 720 and 1100px.
  await page.setViewportSize({ width: 1024, height: 700 });
  const edges: Record<string, number> = {};
  for (const address of ["next/list", "next/cards", "plan/graph", "plan/list", "plan/timeline"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    const tabs = await page.getByTestId("journey-tabs").boundingBox();
    const switcher = await page.getByTestId("projection-switcher").boundingBox();
    expect(switcher?.y, address).toBeLessThan((tabs?.y ?? 0) + (tabs?.height ?? 0));
    edges[address] = Math.round((switcher?.x ?? 0) + (switcher?.width ?? 0));
  }
  expect(new Set(Object.values(edges)).size, JSON.stringify(edges)).toBe(1);
  // Between 720 and 1100px the projection keeps the width and a canvas fills its region.
  const body = page.locator(".journey-body");
  for (const address of ["plan/graph", "plan/graph?decisions=1"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("canvas")).toBeVisible();
    const [scrolls, shown] = await body.evaluate((region) => [region.scrollHeight, region.clientHeight]);
    expect(scrolls, address).toBe(shown);
  }
  await openAt(page, "browser", "j_vendor_eval", "plan/list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  expect(await body.evaluate((region) => region.scrollWidth <= region.clientWidth)).toBe(true);
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("summary-upcoming")).toBeVisible();
});
