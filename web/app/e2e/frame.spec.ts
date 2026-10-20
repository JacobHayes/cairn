// The app frame: one scroller per region, so a node's Details (its History) are reachable in the
// inspector (or the tablet's sheet) at each width, the page itself never scrolls from it; a
// phone's graph is in the page, with the inspector a bottom sheet over it. On the in-browser host.
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
// the sheet over a phone's page.
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

  test("is in the page and fills it, where a swipe on it pans", async ({ page }) => {
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
    // The graph fills the window, so there is nothing below it for the page to scroll to.
    await page.mouse.move(195, 40);
    await page.mouse.wheel(0, 200);
    expect(await scrolled(page)).toBe(0);
  });
});

const sheet = (page: Page) => page.locator(".inspector");
const snapOf = (page: Page) => page.locator(".frame").getAttribute("data-sheet");
const heightOf = async (page: Page) => (await sheet(page).boundingBox())?.height ?? 0;
const handle = (page: Page) => page.getByRole("button", { name: "Resize the inspector" });
const transform = (page: Page) => page.locator('[data-testid="canvas"] .react-flow__viewport').evaluate((element) => (element as HTMLElement).style.transform);

/** Drags the handle by `dy` with a mouse pointer, in steps like a hand. */
async function dragHandle(page: Page, dy: number): Promise<void> {
  const box = await handle(page).boundingBox();
  const [x, y] = [(box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x, y + dy / 2, { steps: 6 });
  await page.mouse.move(x, y + dy, { steps: 6 });
  await page.mouse.up();
}

test.describe("a phone's inspector sheet: dragging", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("sits over the page at its peek with the tabs showing, and a swipe on the graph above it still pans", async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval");
    await expect(sheet(page)).toBeVisible();
    expect(await snapOf(page)).toBe("peek");
    const box = await sheet(page).boundingBox();
    expect((box?.y ?? 0) + (box?.height ?? 0)).toBeCloseTo(844, 0);
    await expect(page.locator(".inspector-head")).toBeInViewport();
    // Nothing of the graph lies under the peek, so the whole canvas is still a gesture surface.
    const canvas = await page.getByTestId("canvas").boundingBox();
    expect((canvas?.y ?? 0) + (canvas?.height ?? 0)).toBeLessThanOrEqual(box?.y ?? 0);
    const before = await transform(page);
    await swipe(page, { x: 195, y: (canvas?.y ?? 0) + 40 }, { dx: -80, dy: -60 });
    await expect.poll(() => transform(page)).not.toBe(before);
  });


  test("a tapped node opens it at the peek, and the handle drags it between peek, half and full and closes it", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    const panel = nodePanel(page, "n_make_offer");
    await expect(panel).toBeAttached();
    await expectAddress(page, "/journeys/j_hiring/nodes/n_make_offer?depth=decisions");
    expect(await snapOf(page)).toBe("peek");
    const peek = await heightOf(page);

    // A finger's drag up opens it to full, and a drag from there down settles on half.
    const grip = await handle(page).boundingBox();
    await swipe(page, { x: (grip?.x ?? 0) + (grip?.width ?? 0) / 2, y: (grip?.y ?? 0) + 10 }, { dx: 0, dy: -600 });
    await expect.poll(() => snapOf(page)).toBe("full");
    const full = await heightOf(page);
    expect(full).toBeGreaterThan(550);
    await expect(panel).toBeVisible();
    await dragHandle(page, 360);
    await expect.poll(() => snapOf(page)).toBe("half");
    const half = await heightOf(page);
    expect(half).toBeGreaterThan(peek + 100);
    expect(half).toBeLessThan(full - 100);
    // The keys step it, and a tap on the handle toggles the peek and the full height.
    await handle(page).focus();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => snapOf(page)).toBe("peek");
    await handle(page).tap();
    await expect.poll(() => snapOf(page)).toBe("full");
    await handle(page).tap();
    await expect.poll(() => snapOf(page)).toBe("peek");

    // Dragged below the peek, it closes: what is left is one pill, and the page fills the window under it.
    await dragHandle(page, 120);
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
    const pill = page.getByRole("button", { name: "Show inspector" });
    await expect(pill).toBeInViewport();
    expect((await sheet(page).boundingBox())?.width).toBeLessThan(250);
    expect((await sheet(page).boundingBox())?.height).toBeLessThan(64);
    // Under no sheet the graph fills the window to its foot: no blank strip where the sheet was.
    const canvas = await page.getByTestId("canvas").boundingBox();
    expect((canvas?.y ?? 0) + (canvas?.height ?? 0)).toBeGreaterThan(844 - 24);
    expect(await page.evaluate(() => document.documentElement.scrollHeight <= window.innerHeight + 1)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    // The pill brings it back at the peek.
    await pill.click();
    expect(await snapOf(page)).toBe("peek");
    await expect(handle(page)).toBeVisible();
  });
});

test.describe("a phone's inspector sheet: closing", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("Hide inspector and Esc close it for real, and picking a node opens it again at the peek", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    await handle(page).tap();
    await expect.poll(() => snapOf(page)).toBe("full");
    await page.getByRole("button", { name: "Hide inspector" }).click();
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
    // Closed, the keyboard lands on the control that opens it, not nowhere.
    await expect(page.getByRole("button", { name: "Show inspector" })).toBeFocused();
    await page.getByRole("button", { name: "Show inspector" }).click();
    // Opened from closed it is at the peek, whatever height it was closed at.
    expect(await snapOf(page)).toBe("peek");
    // Esc takes the open node back to the graph; a second Esc closes the sheet.
    await page.keyboard.press("Escape");
    await expectAddress(page, "/journeys/j_hiring?depth=decisions&view=graph");
    await page.keyboard.press("Escape");
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
    expect(await snapOf(page)).toBe("peek");
  });


  test("the page's last row can be reached above the sheet", async ({ page }) => {
    await openAt(page, "browser", "j_vendor_eval", "list");
    await page.evaluate(() => {
      window.scrollTo(0, document.documentElement.scrollHeight);
    });
    const shown = await page.locator(".ws-body").boundingBox();
    const top = (await sheet(page).boundingBox())?.y ?? 0;
    expect((shown?.y ?? 0) + (shown?.height ?? 0)).toBeGreaterThan(top);
    const last = await page.locator(".ws-body *:visible").last().boundingBox();
    expect((last?.y ?? 0) + (last?.height ?? 0)).toBeLessThanOrEqual(top + 1);
  });
});

test.describe("a phone's inspector sheet: the graph above it", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the graph's bottom controls clear the sheet at every height", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    const zoom = page.getByRole("button", { name: "Zoom in" });
    for (const [snap, press] of [["peek", 0], ["half", 1], ["full", 1]] as const) {
      for (let step = 0; step < press; step += 1) {
        await handle(page).focus();
        await page.keyboard.press("ArrowUp");
      }
      await expect.poll(() => snapOf(page)).toBe(snap);
      const control = await zoom.boundingBox();
      const top = (await sheet(page).boundingBox())?.y ?? 0;
      // At full the sheet covers the whole canvas, so only the lower heights have somewhere to put them.
      if (snap !== "full") {
        await expect.poll(async () => ((await zoom.boundingBox())?.y ?? 0) + ((await zoom.boundingBox())?.height ?? 0)).toBeLessThanOrEqual(top + 1);
      }
      expect(control).not.toBeNull();
    }
  });

  test("Esc closes an open menu and leaves the sheet where it is", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await page.getByTestId("controls-button").click();
    await expect(page.locator("[data-popover]").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator("[data-popover]")).toHaveCount(0);
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
  });
});

test.describe("a landscape phone's inspector sheet", () => {
  test.use({ viewport: { width: 844, height: 390 }, hasTouch: true, isMobile: true });

  test("its peek is a small share of a short window, so the graph keeps most of it", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").dispatchEvent("click");
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
    const canvas = await page.getByTestId("canvas").boundingBox();
    const sheet = await page.locator(".inspector").boundingBox();
    expect(sheet?.height).toBeLessThan(390 * 0.3);
    // The graph above the sheet is a usable share of the window.
    const visible = Math.min((canvas?.y ?? 0) + (canvas?.height ?? 0), sheet?.y ?? 0) - (canvas?.y ?? 0);
    expect(visible).toBeGreaterThan(390 * 0.5);
    await expect(page.locator(".inspector-head")).toBeInViewport();
  });
});

test.describe("a phone's graph framing", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  /** Every drawn card lies whole inside the canvas's own box. */
  async function expectCardsInside(page: Page): Promise<void> {
    const canvas = await page.getByTestId("canvas").boundingBox();
    const cards = await page.getByTestId("node-card").evaluateAll((all) => all.map((each) => each.getBoundingClientRect().toJSON() as { x: number; y: number; right: number; bottom: number }));
    expect(cards.length).toBeGreaterThan(0);
    for (const card of cards) {
      expect(card.x).toBeGreaterThanOrEqual((canvas?.x ?? 0) - 1);
      expect(card.right).toBeLessThanOrEqual((canvas?.x ?? 0) + (canvas?.width ?? 0) + 1);
      expect(card.y).toBeGreaterThanOrEqual((canvas?.y ?? 0) - 1);
      expect(card.bottom).toBeLessThanOrEqual((canvas?.y ?? 0) + (canvas?.height ?? 0) + 1);
    }
  }

  test("the first view shows every card whole, and picking a node keeps them in view above the peek", async ({ page }) => {
    await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
    await expectCardsInside(page);
    await page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]').getByTestId("card-open").click();
    await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
    await expectCardsInside(page);
    const picked = await nodeCard(page, "n_make_offer").boundingBox();
    expect((picked?.y ?? 0) + (picked?.height ?? 0)).toBeLessThanOrEqual((await page.locator(".inspector").boundingBox())?.y ?? 0);
  });
});
