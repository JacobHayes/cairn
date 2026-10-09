// Each journey projection at a phone's width (rung 6): the graph, the next list, the list,
// the cards, the decision view, the timeline, and the Summary page each show their content
// with nothing off the side of the window: the page never scrolls sideways. On the
// in-browser host, fresh on every load.
import { expect, test, type Page } from "@playwright/test";

import { card, nextKeys, listKeys, openActing } from "./acting.ts";
import { openAt, openJourney } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

test.use({ viewport: { width: 390, height: 844 } });

/** Whether the page is no wider than the window, so nothing sits off its side. */
function fits(page: Page): Promise<boolean> {
  return page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
}

test("C1: the canvas fits a narrow window", async ({ page }) => {
  await openJourney(page, "browser", "j_bakeoff");
  expect(await fits(page)).toBe(true);
});

const ACTING: { id: string; screen: string; shown: (page: Page) => Promise<unknown> }[] = [
  { id: "C10", screen: "next/list", shown: nextKeys },
  { id: "C9", screen: "plan/list", shown: listKeys },
  { id: "C11", screen: "next/cards", shown: (page) => expect(card(page)).toBeVisible() },
];

for (const { id, screen, shown } of ACTING) {
  test(`${id}: ${screen} fits a narrow window`, async ({ page }) => {
    await openActing(page, "browser", "j_launch", screen);
    await shown(page);
    expect(await fits(page)).toBe(true);
  });
}

const VIEWS = [
  { id: "C12", address: "plan/graph?decisions=1", part: "decision-view" },
  { id: "C13", address: "plan/timeline", part: "timeline" },
  { id: "C18", address: "summary", part: "summary" },
];

for (const { id, address, part } of VIEWS) {
  test(`${id}: ${address} fits a narrow window`, async ({ page }) => {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId(part)).toBeVisible();
    expect(await fits(page)).toBe(true);
  });
}

test("the journey's menus, the lifecycle chip and the filter open inside the window", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "next/list", { fixedToday: FIXED_TODAY });
  for (const trigger of ["journey-menu", "lifecycle-chip", "filter-button"]) {
    await page.getByTestId(trigger).click();
    const box = await page.getByTestId(`${trigger}-menu`).boundingBox();
    expect(box?.x, trigger).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), trigger).toBeLessThanOrEqual(390);
    await page.keyboard.press("Escape");
  }
});
