// A screen at a phone's width: the list with Frontier on shows its content with nothing off the
// side of the window, the journey's menus, the View picker, Depth and the Filter open inside it. One representative
// screen: the page never scrolls sideways. On the in-browser host, fresh on every load.
import { expect, test } from "./test.ts";

import { frontierKeys, openActing } from "./acting.ts";
import { section } from "./authoring.ts";
import { derived, nodeCard, open } from "./shell.ts";
import { publishStressJourney } from "./stress.ts";

test.use({ viewport: { width: 390, height: 844 } });

test("C10: the list with Frontier on fits a narrow window, the journey's menus open inside it", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "list?frontier=1");
  await frontierKeys(page);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  for (const trigger of ["journey-menu", "view-picker", "controls-button"]) {
    await page.getByTestId(trigger).click();
    const box = await page.getByTestId(`${trigger}-menu`).boundingBox();
    expect(box?.x, trigger).toBeGreaterThanOrEqual(0);
    expect((box?.x ?? 0) + (box?.width ?? 0), trigger).toBeLessThanOrEqual(390);
    await page.keyboard.press("Escape");
  }
});

test("the View picker and the menu share the title's row, and the header is as tall on every view", async ({ page }) => {
  const heights: number[] = [];
  for (const view of ["list", "walk"]) {
    await openActing(page, "browser", "j_launch", view);
    const title = await page.getByTestId("journey-name").boundingBox();
    const picker = await page.getByTestId("view-picker").boundingBox();
    const menu = await page.getByTestId("journey-menu").boundingBox();
    // Beside the title, not under it: the picker starts inside the title's own rows.
    expect(picker?.y, view).toBeLessThan((title?.y ?? 0) + (title?.height ?? 0));
    expect(picker?.x, view).toBeGreaterThanOrEqual((title?.x ?? 0) + (title?.width ?? 0) - 1);
    expect(Math.abs((picker?.y ?? 0) - (menu?.y ?? 99)), view).toBeLessThan(4);
    expect(picker?.x, view).toBeGreaterThanOrEqual(0);
    expect((menu?.x ?? 0) + (menu?.width ?? 0), view).toBeLessThanOrEqual(390);
    heights.push((await page.getByTestId("journey-header").boundingBox())?.height ?? 0);
  }
  expect(heights[0]).toBe(heights[1]);
});

test("the graph's controls fold into one Controls menu that holds them all", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "graph");
  const bar = page.getByTestId("view-controls");
  await expect(bar.getByTestId("controls-button")).toBeVisible();
  await expect(bar.getByTestId("filter-button")).toHaveCount(0);
  // One row: the button, with the Signal button across from it.
  expect((await bar.boundingBox())?.height).toBeLessThan(48);
  await bar.getByTestId("controls-button").click();
  const panel = page.getByTestId("controls-button-menu");
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

test("A3: on a phone the drawing's banner is one banner above the graph, and Escape stops the drawing", async ({ page }) => {
  await open(page, "browser", "/journeys/j_vendor_eval?view=graph&edit=on&depth=actions");
  await derived(page);
  await nodeCard(page, "n_access").getByTestId("card-open").click();
  await (await section(page, "author-edges")).getByRole("button", { name: "Draw it on the canvas" }).click();
  const banner = page.getByTestId("drawing-edge");
  await expect(banner).toHaveCount(1);
  await expect(banner).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(banner).toHaveCount(0);
});

test("the top bar is one short row of the four screens and the sync dot, and You is one tap from the dot", async ({ page }) => {
  await open(page, "browser", "/journeys");
  const nav = page.getByRole("navigation", { name: "Screens", exact: true });
  await expect(nav.getByRole("link")).toHaveText(["Home", "Journeys", "Routes", "Entities"]);
  const bar = await page.locator(".rail").boundingBox();
  expect(bar?.height).toBeLessThanOrEqual(48);
  await expect(page.getByRole("button", { name: "Keyboard shortcuts" })).toBeHidden();
  await page.getByTestId("sync").click();
  await page.getByTestId("sync-popover").getByRole("link", { name: /You/ }).click();
  await expect(page).toHaveURL(/\/me$/);
});

test("a journey name cut by its clamp wears a mark that a tap opens it, and a whole name wears none", async ({ page }) => {
  await open(page, "browser", "/");
  const journey = await publishStressJourney(page);
  await open(page, "browser", `/journeys/${journey}?view=list`);
  const cue = page.getByTestId("journey-name-cue");
  await expect(cue).toBeVisible();
  await page.getByTestId("journey-name").getByRole("button").click();
  await expect(cue).toBeVisible();
  await openActing(page, "browser", "j_launch", "list");
  await expect(cue).toHaveCount(0);
});

test("the timeline's Controls and range switch share one row, and a nested title keeps room", async ({ page }) => {
  await openActing(page, "browser", "j_launch", "timeline");
  const controls = await page.getByTestId("controls-button").boundingBox();
  const range = await page.getByTestId("timeline-range").boundingBox();
  expect(Math.abs((controls?.y ?? 0) - (range?.y ?? 99))).toBeLessThan(12);
  expect((range?.x ?? 0) + (range?.width ?? 0)).toBeLessThanOrEqual(390);
  // Rows below the cap sit at the same indent, so the titles of the deepest rows are as wide as the third level's.
  const widths = await page.locator('[data-testid="timeline-entry"] .timeline-text').evaluateAll((all) => all.map((each) => each.getBoundingClientRect().width));
  expect(Math.min(...widths)).toBeGreaterThan(60);
});
