// The app frame: one scroller per region, so a node's Details (its History) are reachable in the
// inspector (or the tablet's sheet) at each width, the page itself never scrolls from it; a
// phone's graph is in the page, with the inspector a bottom sheet over it. On the in-browser host.
import type { Page } from "@playwright/test";

import { checkpoint } from "./invariants.ts";
import { expect, test } from "./test.ts";

import { expectAddress, nodeCard, nodePanel, openAt, openJourney, openSheet, whenSettled } from "./shell.ts";

/** Opens Details (with the History in it) in `node`'s inspector and scrolls it to its end the way a person does: the wheel over it. */
async function reachHistory(page: Page, node: string, height: number): Promise<void> {
  const panel = nodePanel(page, node);
  const history = panel.getByTestId("details");
  await history.locator("summary").click();
  await expect(history.getByText("Reading...")).toHaveCount(0);
  const body = page.locator('.inspector-body[data-pane="inspector"]');
  const box = await whenSettled(page, () => body.boundingBox());
  expect(box).not.toBeNull();
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  for (let step = 0; step < 12; step += 1) {
    await page.mouse.wheel(0, 600);
  }
  await expect.poll(() => body.evaluate((element) => element.scrollHeight - element.clientHeight - element.scrollTop)).toBeLessThanOrEqual(1);
  const shown = await whenSettled(page, () => history.boundingBox());
  expect(shown?.y).toBeGreaterThanOrEqual(0);
  expect((shown?.y ?? 0) + (shown?.height ?? 0)).toBeLessThanOrEqual(height);
  expect(await whenSettled(page, () => page.evaluate(() => document.scrollingElement?.scrollTop))).toBe(0);
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
  // The inspector hides to a rail that shows it again, by its button and by the key.
  const show = page.getByRole("button", { name: "Show inspector" });
  await expect(show).toBeHidden();
  await page.getByRole("button", { name: "Hide inspector" }).click();
  await expect(nodePanel(page, "n_access")).toBeHidden();
  await expect(show).toBeVisible();
  await show.click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  await page.keyboard.press("]");
  await expect(show).toBeVisible();
  // Picking a node on the canvas opens it in the inspector again, and so does clicking the node that is already selected, which changes no address.
  await nodeCard(page, "n_plan").getByTestId("card-open").dispatchEvent("click");
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await expect(show).toBeHidden();
  await page.keyboard.press("]");
  await expect(show).toBeVisible();
  await nodeCard(page, "n_plan").getByTestId("card-open").dispatchEvent("click");
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await checkpoint(page);

  // The decision graph takes the wheel to pan, and the page behind it does not scroll.
  await openAt(page, "browser", "j_vendor_eval", "graph?depth=decisions");
  const canvas = page.getByTestId("canvas");
  const viewport = canvas.locator(".react-flow__viewport");
  const before = await whenSettled(page, () => viewport.evaluate((element) => (element as HTMLElement).style.transform));
  const box = await whenSettled(page, () => canvas.boundingBox());
  await page.mouse.move((box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2);
  await page.mouse.wheel(0, 300);
  await expect.poll(() => viewport.evaluate((element) => (element as HTMLElement).style.transform)).not.toBe(before);
  const region = page.locator(".ws-body");
  expect(await whenSettled(page, () => region.evaluate((element) => element.scrollHeight - element.clientHeight))).toBeLessThanOrEqual(1);

  // On a tablet the summary's inspector reaches History, the screen's end is above the sheet, and Esc closes it.
  await page.setViewportSize({ width: 1024, height: 700 });
  await openAt(page, "browser", "j_vendor_eval", "summary", { node: "n_plan" });
  // Narrowing from a column starts the sheet at its peek: open it up to read.
  await openSheet(page);
  await reachHistory(page, "n_plan", 700);
  await page.locator(".ws-body").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const sheet = await whenSettled(page, () => page.locator(".inspector").boundingBox());
  const last = await whenSettled(page, () => page.locator(".journey-scroll > *").last().boundingBox());
  expect((last?.y ?? 0) + (last?.height ?? 0)).toBeLessThanOrEqual((sheet?.y ?? 0) + 1);
  await page.keyboard.press("Escape");
  await expect(nodePanel(page, "n_plan")).toHaveCount(0);
  await expectAddress(page, "/journeys/j_vendor_eval");
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

const sheet = (page: Page) => page.locator(".inspector");
const snapOf = (page: Page) => whenSettled(page, () => page.locator(".frame").getAttribute("data-sheet"));
const heightOf = async (page: Page) => (await whenSettled(page, () => sheet(page).boundingBox()))?.height ?? 0;
const handle = (page: Page) => page.getByRole("button", { name: "Resize the inspector" });
/** The canvas's pan and zoom, as React Flow writes it. */
const transform = (page: Page) => whenSettled(page, () => page.locator('[data-testid="canvas"] .react-flow__viewport').evaluate((element) => (element as HTMLElement).style.transform));

/** Drags the handle by `dy` with a mouse pointer, in steps like a hand. */
async function dragHandle(page: Page, dy: number): Promise<void> {
  const box = await whenSettled(page, () => handle(page).boundingBox());
  const [x, y] = [(box?.x ?? 0) + (box?.width ?? 0) / 2, (box?.y ?? 0) + (box?.height ?? 0) / 2];
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x, y + dy / 2, { steps: 6 });
  await page.mouse.move(x, y + dy, { steps: 6 });
  await page.mouse.up();
}

const scrolled = (page: Page) => whenSettled(page, () => page.evaluate(() => document.scrollingElement?.scrollTop ?? 0));
const hiringNode = (page: Page) => page.locator('[data-testid="canvas"] [data-testid="node-card"][data-node="n_make_offer"]');

/** Every drawn card lies whole inside the canvas's own box. */
async function expectCardsInside(page: Page): Promise<void> {
  const canvas = await whenSettled(page, () => page.getByTestId("canvas").boundingBox());
  const cards = await whenSettled(page, () => page.getByTestId("node-card").evaluateAll((all) => all.map((each) => each.getBoundingClientRect().toJSON() as { x: number; y: number; right: number; bottom: number })));
  expect(cards.length).toBeGreaterThan(0);
  for (const card of cards) {
    expect(card.x).toBeGreaterThanOrEqual((canvas?.x ?? 0) - 1);
    expect(card.right).toBeLessThanOrEqual((canvas?.x ?? 0) + (canvas?.width ?? 0) + 1);
    expect(card.y).toBeGreaterThanOrEqual((canvas?.y ?? 0) - 1);
    expect(card.bottom).toBeLessThanOrEqual((canvas?.y ?? 0) + (canvas?.height ?? 0) + 1);
  }
}

async function phoneGraphFills(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(sheet(page)).toBeVisible();
  expect(await snapOf(page)).toBe("peek");
  const peek = await whenSettled(page, () => sheet(page).boundingBox());
  expect((peek?.y ?? 0) + (peek?.height ?? 0)).toBeCloseTo(844, 0);
  await expect(page.locator(".inspector-head")).toBeInViewport();
  // The graph has the room the bars leave, with the inspector's head peeking under it; nothing of it lies under the peek, so the whole canvas is a gesture surface.
  const canvas = await whenSettled(page, () => page.getByTestId("canvas").boundingBox());
  expect(canvas?.height).toBeGreaterThan(400);
  expect((canvas?.y ?? 0) + (canvas?.height ?? 0)).toBeLessThanOrEqual(peek?.y ?? 0);
  expect(await scrolled(page)).toBe(0);
  const before = await transform(page);
  // From empty pane: a finger that lands on a routed edge selects it and does not pan.
  await swipe(page, { x: 195, y: (canvas?.y ?? 0) + 40 }, { dx: -80, dy: -120 });
  await expect.poll(() => transform(page)).not.toBe(before);
  expect(await scrolled(page)).toBe(0);
  // The graph fills the window, so there is nothing below it for the page to scroll to.
  await page.mouse.move(195, 40);
  await page.mouse.wheel(0, 200);
  expect(await scrolled(page)).toBe(0);
  await checkpoint(page);
  // On the list, the page's last row can be reached above the sheet.
  await openAt(page, "browser", "j_vendor_eval", "list");
  await page.evaluate(() => {
    window.scrollTo(0, document.documentElement.scrollHeight);
  });
  const shown = await whenSettled(page, () => page.locator(".ws-body").boundingBox());
  const top = (await whenSettled(page, () => sheet(page).boundingBox()))?.y ?? 0;
  expect((shown?.y ?? 0) + (shown?.height ?? 0)).toBeGreaterThan(top);
  const last = await whenSettled(page, () => page.locator(".ws-body *:visible").last().boundingBox());
  expect((last?.y ?? 0) + (last?.height ?? 0)).toBeLessThanOrEqual(top + 1);
}

async function phoneSheetSnaps(page: Page): Promise<void> {
  await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
  await expectCardsInside(page);
  const zoom = page.locator(".canvas .react-flow__controls");
  // The graph's bottom controls clear the sheet at every height it leaves room at.
  const expectControlsClear = async () => {
    const top = (await whenSettled(page, () => sheet(page).boundingBox()))?.y ?? 0;
    await expect.poll(async () => ((await zoom.boundingBox())?.y ?? 0) + ((await zoom.boundingBox())?.height ?? 0)).toBeLessThanOrEqual(top + 1);
  };
  await hiringNode(page).getByTestId("card-open").click();
  const panel = nodePanel(page, "n_make_offer");
  await expect(panel).toBeAttached();
  await expectAddress(page, "/journeys/j_hiring/nodes/n_make_offer?depth=decisions");
  expect(await snapOf(page)).toBe("peek");
  const peek = await heightOf(page);
  // Picking a node keeps the cards in view above the peek.
  await expectCardsInside(page);
  const picked = await whenSettled(page, () => nodeCard(page, "n_make_offer").boundingBox());
  expect((picked?.y ?? 0) + (picked?.height ?? 0)).toBeLessThanOrEqual((await whenSettled(page, () => sheet(page).boundingBox()))?.y ?? 0);
  await expectControlsClear();
  // Esc closes an open menu and leaves the sheet where it is.
  await page.getByTestId("controls-button").click();
  await expect(page.locator("[data-popover]").first()).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator("[data-popover]")).toHaveCount(0);
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
  expect(await snapOf(page)).toBe("peek");
  await checkpoint(page);

  // A finger's drag up opens it to full, and a drag from there down settles on half.
  const grip = await whenSettled(page, () => handle(page).boundingBox());
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
  await expectControlsClear();
  await checkpoint(page);
  // The keys step it, and a tap on the handle toggles the peek and the full height.
  await handle(page).focus();
  await page.keyboard.press("ArrowDown");
  await expect.poll(() => snapOf(page)).toBe("peek");
  await handle(page).tap();
  await expect.poll(() => snapOf(page)).toBe("full");
  await handle(page).tap();
  await expect.poll(() => snapOf(page)).toBe("peek");

}

async function phoneSheetFoldsAndReturns(page: Page): Promise<void> {
  // Dragged below the peek, it closes: what is left is one pill, and the page fills the window under it.
  await dragHandle(page, 120);
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
  const pill = page.getByRole("button", { name: "Show inspector" });
  await expect(pill).toBeInViewport();
  expect((await whenSettled(page, () => sheet(page).boundingBox()))?.width).toBeLessThan(250);
  expect((await whenSettled(page, () => sheet(page).boundingBox()))?.height).toBeLessThan(64);
  // Under no sheet the graph fills the window to its foot: no blank strip where the sheet was.
  const canvas = await whenSettled(page, () => page.getByTestId("canvas").boundingBox());
  expect((canvas?.y ?? 0) + (canvas?.height ?? 0)).toBeGreaterThan(844 - 24);
  expect(await whenSettled(page, () => page.evaluate(() => document.documentElement.scrollHeight <= window.innerHeight + 1))).toBe(true);
  await checkpoint(page);
  // The pill brings it back at the peek.
  await pill.click();
  expect(await snapOf(page)).toBe("peek");
  await expect(handle(page)).toBeVisible();

  // Hide inspector closes it for real, and the keyboard lands on the control that opens it, which opens it at the peek whatever height it was closed at.
  await handle(page).tap();
  await expect.poll(() => snapOf(page)).toBe("full");
  await page.getByRole("button", { name: "Hide inspector" }).click();
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
  await expect(pill).toBeFocused();
  await pill.click();
  expect(await snapOf(page)).toBe("peek");
  // Esc takes the open node back to the graph; a second Esc closes the sheet; picking a node opens it again at the peek.
  await page.keyboard.press("Escape");
  await expectAddress(page, "/journeys/j_hiring?depth=decisions&view=graph");
  await page.keyboard.press("Escape");
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "folded");
  await hiringNode(page).getByTestId("card-open").click();
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
  expect(await snapOf(page)).toBe("peek");
}


const sheetBox = async (page: Page) => (await whenSettled(page, () => sheet(page).boundingBox())) ?? { y: 0, height: 0 };
const graphShare = async (page: Page, height: number) => {
  const canvas = (await whenSettled(page, () => page.getByTestId("canvas").boundingBox())) ?? { y: 0, height: 0 };
  return (Math.min(canvas.y + canvas.height, (await sheetBox(page)).y) - canvas.y) / height;
};


async function phoneTurns(page: Page): Promise<void> {
  await openAt(page, "browser", "j_hiring", "graph?depth=decisions");
  await hiringNode(page).getByTestId("card-open").click();
  // A landscape phone's peek is a small share of a short window, so the graph keeps most of it.
  await page.setViewportSize({ width: 844, height: 390 });
  await expect(page.locator(".frame")).toHaveAttribute("data-inspector", "on");
  expect(await snapOf(page)).toBe("peek");
  expect((await sheetBox(page)).height).toBeLessThan(390 * 0.3);
  await expect.poll(() => graphShare(page, 390)).toBeGreaterThan(0.5);
  await expect(page.locator(".inspector-head")).toBeInViewport();
  await page.setViewportSize({ width: 390, height: 844 });
  // A picked node keeps the sheet on screen at tablet widths too.
  await handle(page).focus();
  await page.keyboard.press("ArrowUp");
  await expect.poll(() => snapOf(page)).toBe("half");
  expect((await sheetBox(page)).height).toBeCloseTo(844 * 0.4, 0);
  await page.setViewportSize({ width: 844, height: 390 });
  await expect.poll(async () => (await sheetBox(page)).height).toBeCloseTo(390 * 0.4, 0);
  expect(await snapOf(page)).toBe("half");
  await expect.poll(() => graphShare(page, 390)).toBeGreaterThan(0.25);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(async () => (await sheetBox(page)).height).toBeCloseTo(844 * 0.4, 0);
  // A desktop width is a column beside the workspace; narrowing again opens the sheet at its peek.
  await page.setViewportSize({ width: 1440, height: 800 });
  await expect.poll(async () => (await sheet(page).boundingBox())?.width ?? 0).toBeLessThan(500);
  expect((await sheetBox(page)).height).toBeGreaterThan(600);
  await page.setViewportSize({ width: 390, height: 844 });
  await expect.poll(() => snapOf(page)).toBe("peek");
}

test.describe("a phone", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });

  test("the graph is in the page and fills it, the inspector sits over it at its peek, a swipe on the graph pans, and the page's last row can be reached above the sheet", async ({ page }) => {
    await phoneGraphFills(page);
  });

  test("a node opens the sheet at its peek with every card in view above it; the handle drags it between peek, half and full and closes it, Esc closes a menu and not the sheet, and Hide inspector and Esc close it for real", async ({ page }) => {
    await phoneSheetSnaps(page);
    await phoneSheetFoldsAndReturns(page);
  });

  test("the sheet keeps its snap and takes the new window's height when the phone turns, its peek on a short window leaves the graph most of it, and a desktop width puts it in its column", async ({ page }) => {
    await phoneTurns(page);
  });
});
