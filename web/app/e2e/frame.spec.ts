// The app frame (rung 6): one scroller per region, so a node's History is reachable in the
// inspector (or the tablet's sheet) on every screen that has one, the page itself never scrolls
// from it; a phone's map is a preview that leaves swipes to the page and opens full screen,
// where they pan; and the theme the viewer chose survives a reload. On the in-browser host.
import { expect, test, type Page } from "@playwright/test";

import { nodeCard, nodePanel, open, openJourney } from "./shell.ts";
import { openScreen, showDecisionTable } from "./views.ts";

const SCREENS = [
  { name: "canvas", segment: undefined },
  { name: "decisions", segment: "decisions" },
  { name: "timeline", segment: "timeline" },
  { name: "summary", segment: "summary" },
];
const WINDOWS = [
  { width: 1440, height: 768 },
  { width: 1024, height: 700 },
];

for (const { name, segment } of SCREENS) {
  for (const window of WINDOWS) {
    test(`the ${name} screen's inspector scrolls to the end of History at ${String(window.width)}px`, async ({ page }) => {
      await page.setViewportSize(window);
      if (segment === undefined) {
        await open(page, "browser", "/journeys/j_vendor_eval/nodes/n_plan");
      } else {
        await openScreen(page, "browser", "j_vendor_eval", segment, "n_plan");
      }
      const panel = nodePanel(page, "n_plan");
      const history = panel.getByTestId("history");
      await history.locator("summary").click();
      await expect(history.getByText("Reading...")).toHaveCount(0);
      // The way a person scrolls it: the wheel over the inspector, until it stops moving.
      const body = page.locator('.inspector-body[data-pane="inspector"]');
      const box = await body.boundingBox();
      expect(box).not.toBeNull();
      await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
      for (let step = 0; step < 12; step += 1) {
        await page.mouse.wheel(0, 600);
      }
      await expect.poll(() => body.evaluate((element) => element.scrollHeight - element.clientHeight - element.scrollTop)).toBeLessThanOrEqual(1);
      const shown = await history.boundingBox();
      expect(shown?.y).toBeGreaterThanOrEqual(0);
      expect((shown?.y ?? 0) + (shown?.height ?? 0)).toBeLessThanOrEqual(window.height);
      expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
    });
  }
}

test("the decision view is the graph or the table, so its canvas never competes with a scroller for the wheel", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  // A phone's `map=1` (carried over a rotation) must not outlive the graph: the table has no map to close.
  await open(page, "browser", "/journeys/j_vendor_eval/decisions?map=1");
  await expect(page.getByTestId("screen-decisions")).toBeVisible();
  const canvas = page.getByTestId("canvas");
  const viewport = canvas.locator(".react-flow__viewport");
  const before = await viewport.evaluate((element) => (element as HTMLElement).style.transform);
  const box = await canvas.boundingBox();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  await page.mouse.wheel(0, 300);
  await expect.poll(() => viewport.evaluate((element) => (element as HTMLElement).style.transform)).not.toBe(before);
  const region = page.locator(".ws-body");
  expect(await region.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeLessThanOrEqual(1);
  await showDecisionTable(page);
  await expect(canvas).toHaveCount(0);
  await expect(page).not.toHaveURL(/map=1/);
});

test("on a tablet a scrolling screen's end is above the sheet", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  await openScreen(page, "browser", "j_vendor_eval", "summary", "n_plan");
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  const region = page.locator(".ws-body");
  await region.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const sheet = await page.locator(".inspector").boundingBox();
  const last = await region.locator(":scope > *").last().boundingBox();
  expect((last?.y ?? 0) + (last?.height ?? 0)).toBeLessThanOrEqual((sheet?.y ?? 0) + 1);
});

test.describe("a phone's map", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  /** The canvas's pan and zoom, as React Flow writes it. */
  const transform = (page: Page, within: string) => page.locator(`${within} .react-flow__viewport`).evaluate((element) => (element as HTMLElement).style.transform);
  const scrolled = (page: Page) => page.evaluate(() => document.scrollingElement?.scrollTop ?? 0);
  /** A finger drawn from the middle of `x`,`y` by `dx`,`dy`. */
  async function swipe(page: Page, at: { x: number; y: number }, by: { dx: number; dy: number }): Promise<void> {
    const finger = await page.context().newCDPSession(page);
    await finger.send("Input.synthesizeScrollGesture", { ...at, xDistance: by.dx, yDistance: by.dy, gestureSourceType: "touch", speed: 600 });
  }

  test("is a preview a swipe scrolls past, and opens full screen where a swipe pans", async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval");
    const preview = page.getByTestId("map-preview");
    const box = await preview.boundingBox();
    expect(box).not.toBeNull();
    const at = { x: 195, y: ((box?.y ?? 0) + Math.min((box?.y ?? 0) + (box?.height ?? 0), 844)) / 2 };
    const before = await transform(page, '[data-testid="map-preview"]');
    expect(await scrolled(page)).toBe(0);

    // The page is only a little taller than the window here, so each gesture is small and the next one reaches further.
    await page.mouse.move(at.x, at.y);
    await page.mouse.wheel(0, 20);
    await expect.poll(() => scrolled(page)).toBeGreaterThan(0);
    const wheeled = await scrolled(page);
    await swipe(page, at, { dx: 0, dy: -200 });
    await expect.poll(() => scrolled(page)).toBeGreaterThan(wheeled);
    expect(await transform(page, '[data-testid="map-preview"]')).toBe(before);

    const resting = await scrolled(page);
    await page.getByTestId("map-open").click();
    await expect(page).toHaveURL(/map=1/);
    await expect(page.getByTestId("map-full")).toBeVisible();
    const opened = await transform(page, '[data-testid="map-full"]');
    await swipe(page, { x: 195, y: 500 }, { dx: -80, dy: -120 });
    await expect.poll(() => transform(page, '[data-testid="map-full"]')).not.toBe(opened);
    expect(await scrolled(page)).toBe(resting);

    await page.goBack();
    await expect(page.getByTestId("map-full")).toHaveCount(0);
    expect(await scrolled(page)).toBe(resting);
  });

  test("opens a tapped node as a sheet over the map, with nothing off the side", async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval", "?map=1");
    await nodeCard(page, "n_final_report").getByTestId("card-open").click();
    const panel = nodePanel(page, "n_final_report");
    await expect(panel).toBeVisible();
    await expect(page.getByTestId("map-full")).toBeVisible();
    expect((await panel.boundingBox())?.y ?? 0).toBeGreaterThan(300);
    // The sheet is bounded and its body scrolls, so the end of the node's detail is reachable.
    const body = page.locator('.inspector-body[data-pane="inspector"]');
    expect(await body.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });

  test("keeps the map open when a decision's card is tapped", async ({ page }) => {
    await open(page, "browser", "/journeys/j_hiring/decisions?map=1");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    await expect(nodePanel(page, "n_make_offer")).toBeVisible();
    await expect(page).toHaveURL(/\/decisions\/nodes\/n_make_offer\?map=1$/);
    await expect(page.getByTestId("map-full")).toBeVisible();
  });
});

test("on a tablet Esc closes the sheet and leaves the screen as it was", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  await open(page, "browser", "/journeys/j_vendor_eval/nodes/n_plan?hide=action");
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, "n_plan")).toHaveCount(0);
  await expect(page).toHaveURL(/\/journeys\/j_vendor_eval\?hide=action$/);
});

test("the theme the viewer chose is kept across a reload", async ({ page }) => {
  await open(page, "browser", "/me");
  await page.getByRole("radio", { name: "Dark" }).check();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("radio", { name: "System" }).check();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme");
});
