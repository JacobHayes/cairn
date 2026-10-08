// Each journey screen at a phone's width (rung 6): the canvas, the next list, the list,
// triage, the decision view, the timeline, and the status summary each show their content
// with nothing off the side of the window: the page never scrolls sideways. On the
// in-browser host, fresh on every load.
import { expect, test, type Page } from "@playwright/test";

import { card, nextKeys, listKeys, openActing } from "./acting.ts";
import { openJourney } from "./shell.ts";
import { openScreen } from "./views.ts";

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
  { id: "C10", screen: "next", shown: nextKeys },
  { id: "C9", screen: "list", shown: listKeys },
  { id: "C11", screen: "triage", shown: (page) => expect(card(page)).toBeVisible() },
];

for (const { id, screen, shown } of ACTING) {
  test(`${id}: the ${screen} screen fits a narrow window`, async ({ page }) => {
    await openActing(page, "browser", "j_launch", screen);
    await shown(page);
    expect(await fits(page)).toBe(true);
  });
}

const VIEWS = [
  { id: "C12", segment: "decisions", part: "decision-table" },
  { id: "C13", segment: "timeline", part: "timeline-axis" },
  { id: "C18", segment: "summary", part: "summary" },
];

for (const { id, segment, part } of VIEWS) {
  test(`${id}: the ${segment} screen fits a narrow window`, async ({ page }) => {
    await openScreen(page, "browser", "j_vendor_eval", segment);
    await expect(page.getByTestId(part)).toBeVisible();
    expect(await fits(page)).toBe(true);
  });
}
