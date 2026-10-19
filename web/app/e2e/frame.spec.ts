// The app frame: one scroller per region, so a node's Details (its History) are reachable in the
// inspector (or the tablet's sheet) at each width, the page itself never scrolls from it; a
// phone's graph is in the page, with the inspector stacked under it. On the in-browser host.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { expectAddress, nodeCard, nodePanel, openAt, openJourney } from "./shell.ts";

/** Opens Details (with the History in it) in `node`'s inspector and scrolls it to its end the way a person does: the wheel over it. */
async function reachHistory(page: Page, node: string, height: number): Promise<void> {
  const panel = nodePanel(page, node);
  const history = panel.getByTestId("details");
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
// screen per width guards it: a canvas at the desktop, the summary on a tablet, and (below)
// the sheet over a phone's map.
test("the inspector scrolls to the end of Details at 1440 and 1024px, a node opens at its top, and the page behind never scrolls", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 768 });
  await openAt(page, "browser", "j_vendor_eval", "graph?depth=actions", { node: "n_plan" });
  await reachHistory(page, "n_plan", 768);
  // The next node opens at its header, not part-way down this one's long body.
  // The card may be off screen: the click is sent to it, as panning to it would.
  await nodeCard(page, "n_access").getByTestId("card-open").dispatchEvent("click");
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await expect.poll(() => page.locator('[data-pane="inspector"]').evaluate((element) => element.scrollTop)).toBe(0);

  // The decision graph takes the wheel to pan, and the page behind it does not scroll.
  await openAt(page, "browser", "j_vendor_eval", "graph?depth=decisions");
  const canvas = page.getByTestId("canvas");
  const viewport = canvas.locator(".react-flow__viewport");
  const before = await viewport.evaluate((element) => (element as HTMLElement).style.transform);
  const box = await canvas.boundingBox();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  await page.mouse.wheel(0, 300);
  await expect.poll(() => viewport.evaluate((element) => (element as HTMLElement).style.transform)).not.toBe(before);
  const region = page.locator(".ws-body");
  expect(await region.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeLessThanOrEqual(1);

  // On a tablet the summary's inspector reaches History, the screen's end is above the sheet, and Esc closes it.
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
  await expectAddress(page, "/journeys/j_vendor_eval");
});

test("the inspector hides to a rail that shows it again, and selecting a node brings it back", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 768 });
  await openAt(page, "browser", "j_vendor_eval", "graph?depth=actions", { node: "n_plan" });
  const show = page.getByRole("button", { name: "Show inspector" });
  await expect(show).toBeHidden();
  await page.getByRole("button", { name: "Hide inspector" }).click();
  await expect(nodePanel(page, "n_plan")).toBeHidden();
  await expect(show).toBeVisible();
  // The rail is the way back, and so is the key.
  await show.click();
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await page.keyboard.press("]");
  await expect(show).toBeVisible();
  // Picking another node on the canvas opens it in the inspector again.
  await nodeCard(page, "n_access").getByTestId("card-open").dispatchEvent("click");
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await expect(show).toBeHidden();
  // So does clicking the node that is already selected, which changes no address.
  await page.keyboard.press("]");
  await expect(show).toBeVisible();
  await nodeCard(page, "n_access").getByTestId("card-open").dispatchEvent("click");
  await expect(nodePanel(page, "n_access")).toBeVisible();
});

test("the rail's shortcuts hint lists the keys on hover and focus, and goes on leaving", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list");
  const hint = page.getByRole("button", { name: "Keyboard shortcuts" });
  await hint.hover();
  await expect(page.getByTestId("keys-hint").locator("kbd").first()).toBeVisible();
  await page.mouse.move(700, 300);
  await expect(page.getByTestId("keys-hint")).toHaveCount(0);
  await hint.focus();
  await expect(page.getByTestId("keys-hint")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("keys-hint")).toHaveCount(0);
});

/**
 * A finger drawn from `x`,`y` by `dx`,`dy` over a third of a second, resting before it lifts so
 * nothing flings on after it. Raw touch points, since Chromium's synthesized touch scroll
 * gesture moves nothing at all in Playwright's browser.
 */
async function swipe(page: Page, at: { x: number; y: number }, by: { dx: number; dy: number }): Promise<void> {
  const finger = await page.context().newCDPSession(page);
  const steps = 20;
  const start = Date.now() / 1000;
  const touch = (type: "touchStart" | "touchMove" | "touchEnd", step: number, seconds: number) =>
    finger.send("Input.dispatchTouchEvent", {
      type,
      touchPoints: type === "touchEnd" ? [] : [{ x: at.x + (by.dx * step) / steps, y: at.y + (by.dy * step) / steps }],
      timestamp: start + seconds,
    });
  await touch("touchStart", 0, 0);
  for (let step = 1; step <= steps; step += 1) {
    await touch("touchMove", step, step / 60);
  }
  await touch("touchEnd", steps, steps / 60 + 0.2);
}

test.describe("a phone's graph", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  /** The canvas's pan and zoom, as React Flow writes it. */
  const transform = (page: Page) => page.locator('[data-testid="canvas"] .react-flow__viewport').evaluate((element) => (element as HTMLElement).style.transform);
  const scrolled = (page: Page) => page.evaluate(() => document.scrollingElement?.scrollTop ?? 0);

  test("is in the page, where a swipe on it pans and a swipe on the head scrolls the page", async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval");
    const canvas = page.getByTestId("canvas");
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    // The graph has the room the bars leave, with the inspector's head peeking under it.
    expect(box?.height).toBeGreaterThan(400);
    expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThan(844);
    expect(await scrolled(page)).toBe(0);
    const before = await transform(page);
    // From empty pane: a finger that lands on a routed edge selects it and does not pan.
    await swipe(page, { x: 195, y: (box?.y ?? 0) + 40 }, { dx: -80, dy: -120 });
    await expect.poll(() => transform(page)).not.toBe(before);
    expect(await scrolled(page)).toBe(0);
    // The head is the page's: a wheel over it scrolls.
    await page.mouse.move(195, 40);
    await page.mouse.wheel(0, 200);
    await expect.poll(() => scrolled(page)).toBeGreaterThan(0);
  });

  test("opens a tapped decision in the inspector under it, which can be hidden and shown", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    const panel = nodePanel(page, "n_make_offer");
    await expect(panel).toBeVisible();
    await expectAddress(page, "/journeys/j_hiring/nodes/n_make_offer?depth=decisions");
    // The page brings the column into view and has no sideways scroll.
    await expect.poll(async () => (await page.locator(".inspector").boundingBox())?.y ?? 9999).toBeLessThan(400);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    // Hiding it folds the column to a slim bar in the page, and the bar opens it again.
    await page.getByRole("button", { name: "Hide inspector" }).click();
    await expect(panel).toBeHidden();
    expect((await page.getByRole("button", { name: "Show inspector" }).boundingBox())?.height).toBeGreaterThanOrEqual(24);
    await page.getByRole("button", { name: "Show inspector" }).click();
    await expect(panel).toBeVisible();
  });
});

test.describe("a phone's inspector", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("stacks the journey card in the inspector right under the graph", async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval");
    const column = page.locator(".inspector");
    await expect(column.getByTestId("journey-card")).toBeAttached();
    const graph = await page.getByTestId("canvas").boundingBox();
    const under = await column.boundingBox();
    expect(graph).not.toBeNull();
    expect(under).not.toBeNull();
    // Only the workspace's own padding lies between the graph and the column's head.
    expect((under?.y ?? 0) - ((graph?.y ?? 0) + (graph?.height ?? 0))).toBeLessThan(64);
  });
});
