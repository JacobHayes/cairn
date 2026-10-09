// The app frame (rung 6): one scroller per region, so a node's History is reachable in the
// inspector (or the tablet's sheet) at each width, the page itself never scrolls from it; a
// phone's map is a preview that leaves swipes to the page and opens full screen, where they
// pan. On the in-browser host.
import { expect, test, type Page } from "@playwright/test";

import { nodePanel, open, openAt, openJourney } from "./shell.ts";

/** Opens History in `node`'s inspector and scrolls it to its end the way a person does: the wheel over it. */
async function reachHistory(page: Page, node: string, height: number): Promise<void> {
  const panel = nodePanel(page, node);
  const history = panel.getByTestId("history");
  await history.locator("summary").click();
  await expect(history.getByText("Reading...")).toHaveCount(0);
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
  expect((shown?.y ?? 0) + (shown?.height ?? 0)).toBeLessThanOrEqual(height);
  expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
}

// The inspector is one component and the CSS lint holds each region to one scroller, so one
// screen per width guards it: a canvas at the desktop, the summary on a tablet, and the sheet
// over a phone's map.
test("the plan graph's inspector scrolls to the end of History at 1440px", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 768 });
  await openAt(page, "browser", "j_vendor_eval", "plan/graph", { node: "n_plan" });
  await reachHistory(page, "n_plan", 768);
});

test("on a tablet the summary's inspector reaches History, the screen's end is above the sheet, and Esc closes it", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  await openAt(page, "browser", "j_vendor_eval", "summary", { node: "n_plan" });
  await reachHistory(page, "n_plan", 700);
  await page.locator(".ws-body").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const sheet = await page.locator(".inspector").boundingBox();
  const last = await page.locator(".journey-scroll > *").last().boundingBox();
  expect((last?.y ?? 0) + (last?.height ?? 0)).toBeLessThanOrEqual((sheet?.y ?? 0) + 1);
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, "n_plan")).toHaveCount(0);
  await expect(page).toHaveURL(/\/journeys\/j_vendor_eval\/summary$/);
});

test("the decision graph takes the wheel to pan, and the page behind it does not scroll", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await openAt(page, "browser", "j_vendor_eval", "plan/graph?decisions=1");
  const canvas = page.getByTestId("canvas");
  const viewport = canvas.locator(".react-flow__viewport");
  const before = await viewport.evaluate((element) => (element as HTMLElement).style.transform);
  const box = await canvas.boundingBox();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  await page.mouse.wheel(0, 300);
  await expect.poll(() => viewport.evaluate((element) => (element as HTMLElement).style.transform)).not.toBe(before);
  const region = page.locator(".ws-body");
  expect(await region.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeLessThanOrEqual(1);
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

  test("opens a tapped decision as a sheet over the map, with nothing off the side", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "plan/graph?decisions=1&map=1");
    await page.locator('[data-testid="decision-view"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    const panel = nodePanel(page, "n_make_offer");
    await expect(panel).toBeVisible();
    await expect(page).toHaveURL(/\/plan\/graph\/nodes\/n_make_offer\?decisions=1&map=1$/);
    await expect(page.getByTestId("map-full")).toBeVisible();
    const sheet = (await page.locator(".inspector").boundingBox())?.y ?? 0;
    expect(sheet).toBeGreaterThan(300);
    // The sheet is bounded and its body scrolls, so the end of the node's detail is reachable.
    await reachHistory(page, "n_make_offer", 844);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  });
});

test("on a phone a short page still ends with the strip at the screen's bottom edge", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, "browser", "/me");
  const strip = await page.locator(".strip").boundingBox();
  expect((strip?.y ?? 0) + (strip?.height ?? 0)).toBeCloseTo(844, 0);
});
