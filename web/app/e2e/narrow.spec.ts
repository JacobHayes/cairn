// A screen at a phone's width: the list with Frontier on shows its content with nothing off the
// side of the window, the journey's menus, the lifecycle chip, Depth and the Filter open inside it. One representative
// screen: the page never scrolls sideways. On the in-browser host, fresh on every load.
import { expect, test } from "@playwright/test";

import { frontierKeys, openActing } from "./acting.ts";

test.use({ viewport: { width: 390, height: 844 } });

test("C10: the list with Frontier on fits a narrow window, the journey's menus open inside it", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "list?frontier=1");
  await frontierKeys(page);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  // The title, the lifecycle chip and the menu share one row.
  const title = await page.getByTestId("journey-name").boundingBox();
  const chip = await page.getByTestId("lifecycle-chip").boundingBox();
  expect(chip?.y).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
  for (const trigger of ["journey-menu", "lifecycle-chip", "depth-button", "filter-button"]) {
    await page.getByTestId(trigger).click();
    const box = await page.getByTestId(`${trigger}-menu`).boundingBox();
    expect(box?.x, trigger).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), trigger).toBeLessThanOrEqual(390);
    await page.keyboard.press("Escape");
  }
});

test("the graph's controls fold into one View menu that holds them all", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "graph");
  await page.getByRole("button", { name: "Open map" }).click();
  const bar = page.getByTestId("view-controls");
  await expect(bar.getByTestId("view-button")).toBeVisible();
  await expect(bar.getByTestId("filter-button")).toHaveCount(0);
  // One row: the button, with the Signal button across from it.
  expect((await bar.boundingBox())?.height).toBeLessThan(48);
  await bar.getByTestId("view-button").click();
  const panel = page.getByTestId("view-button-menu");
  for (const part of ["ladder", "depth-fade", "chip-frontier", "filter-body", "toolbar-search"]) {
    await expect(panel.getByTestId(part)).toBeVisible();
  }
  // The panel ends with its last control: no band of empty space under Search.
  const search = await panel.getByTestId("toolbar-search").boundingBox();
  const whole = await panel.boundingBox();
  expect((whole?.y ?? 0) + (whole?.height ?? 0) - ((search?.y ?? 0) + (search?.height ?? 0))).toBeLessThan(32);
  await panel.getByTestId("chip-frontier").click();
  await expect(page).toHaveURL(/frontier=1/);
});
