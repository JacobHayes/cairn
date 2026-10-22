// A screen at a phone's width: the list with Frontier on shows its content with nothing off the
// side of the window, the journey's menus, the View picker, Depth and the Filter open inside it. One representative
// journey walked across its views: the page never scrolls sideways. On the in-browser host, fresh on every load.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { frontierKeys, openActing } from "./acting.ts";
import { goTo, whenSettled } from "./shell.ts";

test.use({ viewport: { width: 390, height: 844 } });

/** The journey's menus open inside the window, and the picker shares the title's row with the menu. */
async function menusFit(page: Page): Promise<void> {
  await openActing(page, "browser", "j_launch", "list?frontier=1");
  await frontierKeys(page);
  expect(await whenSettled(page, () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))).toBe(true);
  for (const trigger of ["journey-menu", "view-picker", "controls-button"]) {
    await page.getByTestId(trigger).click();
    const box = await whenSettled(page, () => page.getByTestId(`${trigger}-menu`).boundingBox());
    expect(box?.x, trigger).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), trigger).toBeLessThanOrEqual(390);
    await page.keyboard.press("Escape");
  }
  const [title, picker, menu] = await whenSettled(page, () =>
    Promise.all([page.getByTestId("journey-name").boundingBox(), page.getByTestId("view-picker").boundingBox(), page.getByTestId("journey-menu").boundingBox()]),
  );
  // Beside the title, not under it: the picker starts inside the title's own rows.
  expect(picker?.y).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
  expect(picker?.x).toBeGreaterThanOrEqual((title?.x ?? 0) + (title?.width ?? 0) - 1);
  expect(Math.abs((picker?.y ?? 0) - (menu?.y ?? 99))).toBeLessThan(4);
}

/** The graph's controls fold into one Controls menu that holds them all. */
async function graphControlsFold(page: Page): Promise<void> {
  await goTo(page, "graph");
  const bar = page.getByTestId("view-controls");
  await expect(bar.getByTestId("controls-button")).toBeVisible();
  await expect(bar.getByTestId("filter-button")).toHaveCount(0);
  // One row: the button, with the Signal button across from it.
  expect((await whenSettled(page, () => bar.boundingBox()))?.height).toBeLessThan(48);
  await bar.getByTestId("controls-button").click();
  const panel = page.getByTestId("controls-button-menu");
  for (const part of ["ladder", "depth-fade", "chip-frontier", "filter-body", "toolbar-search"]) {
    await expect(panel.getByTestId(part)).toBeVisible();
  }
  // The panel ends with its last control: no band of empty space under Search.
  const [search, whole] = await whenSettled(page, () => Promise.all([panel.getByTestId("toolbar-search").boundingBox(), panel.boundingBox()]));
  expect((whole?.y ?? 0) + (whole?.height ?? 0) - ((search?.y ?? 0) + (search?.height ?? 0))).toBeLessThan(32);
  await panel.getByTestId("chip-frontier").click();
  // Frontier was on, so the chip in the menu turns it off.
  await expect(page).not.toHaveURL(/frontier=1/);
  await page.keyboard.press("Escape");
}

/** The timeline's Controls and range switch share one row, and a nested title keeps room. */
async function timelineRowShared(page: Page): Promise<void> {
  await goTo(page, "timeline");
  const [controls, range] = await whenSettled(page, () => Promise.all([page.getByTestId("controls-button").boundingBox(), page.getByTestId("timeline-range").boundingBox()]));
  expect(Math.abs((controls?.y ?? 0) - (range?.y ?? 99))).toBeLessThan(12);
  expect((range?.x ?? 0) + (range?.width ?? 0)).toBeLessThanOrEqual(390);
  // Rows below the cap sit at the same indent, so the titles of the deepest rows are as wide as the third level's.
  const widths = await whenSettled(page, () => page.locator('[data-testid="timeline-entry"] .timeline-text').evaluateAll((all) => all.map((each) => each.getBoundingClientRect().width)));
  expect(Math.min(...widths)).toBeGreaterThan(60);
}

test("C10: a journey at a phone's width keeps every menu, the graph's Controls and the timeline's row inside the window, and the top bar is one short row with You one tap from the sync dot", async ({ page }) => {
  await menusFit(page);
  await graphControlsFold(page);
  await timelineRowShared(page);
  const nav = page.getByRole("navigation", { name: "Screens", exact: true });
  await expect(nav.getByRole("link")).toHaveText(["Home", "Journeys", "Routes", "Identities"]);
  expect((await whenSettled(page, () => page.locator(".rail").boundingBox()))?.height).toBeLessThanOrEqual(48);
  await expect(page.getByRole("button", { name: "Keyboard shortcuts" })).toBeHidden();
  await page.getByTestId("sync").click();
  await page.getByTestId("sync-popover").getByRole("link", { name: /You/ }).click();
  await expect(page).toHaveURL(/\/me$/);
});
