// A screen at a phone's width: the next list shows its content with nothing off the
// side of the window, the journey's menus, the lifecycle chip and the filter open inside it, and
// a short page still ends with the strip at the screen's bottom edge. One representative
// screen: the page never scrolls sideways. On the in-browser host, fresh on every load.
import { expect, test } from "@playwright/test";

import { nextKeys, openActing } from "./acting.ts";
import { open } from "./shell.ts";

test.use({ viewport: { width: 390, height: 844 } });

test("C10: the next list fits a narrow window, the journey's menus open inside it, and a short page ends with the strip", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "next/list");
  await nextKeys(page);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  // The title, the lifecycle chip and the menu share one row.
  const title = await page.getByTestId("journey-name").boundingBox();
  const chip = await page.getByTestId("lifecycle-chip").boundingBox();
  expect(chip?.y).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
  for (const trigger of ["journey-menu", "lifecycle-chip", "filter-button"]) {
    await page.getByTestId(trigger).click();
    const box = await page.getByTestId(`${trigger}-menu`).boundingBox();
    expect(box?.x, trigger).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), trigger).toBeLessThanOrEqual(390);
    await page.keyboard.press("Escape");
  }
  // The strip stays at the screen's bottom edge when the page is short.
  await open(page, "browser", "/me");
  const strip = await page.locator(".strip").boundingBox();
  expect((strip?.y ?? 0) + (strip?.height ?? 0)).toBeCloseTo(844, 0);
});
