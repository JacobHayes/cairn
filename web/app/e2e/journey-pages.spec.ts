// A journey's page in a real browser (2.2 to 2.4), on the in-browser host: what only focus,
// layout and the window show. The keys act on the view the pointer left them on (j, Enter, Esc,
// v), the Frontier chip turns the lens on, and no view scrolls the window: the head stays, a
// popover taller than the room scrolls inside itself, the toolbar keeps its row, and a canvas
// fills its region. The addresses, chips and filters behind them are vitest's
// (journeys/address.test.ts and screens/filters.test.ts).
import { expect, test, type Locator } from "@playwright/test";

import { journeyName, startJourney } from "./around.ts";
import { viewportOf, viewportSettled } from "./canvas.ts";
import { frontierItem, frontierKeys } from "./acting.ts";
import { nodeCard, nodePanel, openAt, openFilter, openJourney } from "./shell.ts";
import { FIXED_TODAY } from "./views.ts";

const UP_FRONT = ["n_meeting_date", "n_partner_runs", "n_purpose", "n_who_informed", "n_who_owns"];

test("C10, C11: after a click in the page, j opens the next row, Enter its form, Esc leaves it, and v swaps the view", async ({ page }) => {
  const id = await startJourney(page, "browser", journeyName("Keys"), { route: "vendor-evaluation", version: 1 });
  await openAt(page, "browser", id, "list?frontier=1");
  await page.getByTestId("chip-decisions").click();
  await expect.poll(() => frontierKeys(page).then((keys) => [...keys].sort())).toEqual(UP_FRONT);

  // j opens the first row in the inspector, whose form is then the node's only one; Enter opens it.
  const [first = ""] = await frontierKeys(page);
  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("j");
  await expect(nodePanel(page, first)).toBeVisible();
  await expect(frontierItem(page, first).getByTestId("acts")).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(nodePanel(page, first).getByTestId("answer-editor")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("main").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(page.getByTestId("view-timeline")).toHaveAttribute("aria-current", "page");
  await expect(page).toHaveURL(/\/timeline\?decisions=1&frontier=1$/);
  await page.keyboard.press("v");
  await expect(page).toHaveURL(/\/graph\?decisions=1&frontier=1$/);
});

test("the Frontier chip turns the lens on for every view", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "graph");
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await page.getByTestId("chip-frontier").click();
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  await page.getByTestId("view-list").click();
  await expect(page).toHaveURL(/\/list\?frontier=1$/);
  // The list is the frontier in rank order, numbered by rank, with as many rows as the chip's tooltip counts.
  const places = await page.locator('[data-testid="list-row"] [data-column="rank"]').allTextContents();
  expect(places).toEqual(places.map((_, at) => `#${String(at + 1)}`));
  const counted = /(\d+) can move now/.exec((await page.getByTestId("chip-frontier").getAttribute("title")) ?? "")?.[1];
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", counted ?? "");
});

test("a journey opens as the stock view, switching views keeps the chips, and coming back to the journey starts over", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  const detail = page.getByTestId("detail-button");
  // The stock view: every detail, fitted to the whole graph.
  await expect(detail).toHaveAccessibleName("Detail: All");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await viewportSettled(page);
  const opened = await viewportOf(page);
  await page.keyboard.press("f");
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(opened);
  // A chip follows a switch of view and back; the pan is the view's own.
  await page.getByTestId("chip-frontier").click();
  await page.mouse.move(600, 500);
  await page.mouse.wheel(0, 150);
  await viewportSettled(page);
  await page.getByTestId("view-list").click();
  await expect(page).toHaveURL(/\/list\?frontier=1$/);
  await page.getByTestId("view-graph").click();
  await expect(page).toHaveURL(/\/graph\?frontier=1$/);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(opened);
  // Back at the journey's landing, nothing is held: no chip, no pan.
  await page.evaluate(() => {
    history.pushState(null, "", location.pathname.replace(/\/graph.*/, ""));
    dispatchEvent(new PopStateEvent("popstate"));
  });
  await expect(page).toHaveURL(/\/graph$/);
  await expect(page.getByTestId("chip-frontier")).toHaveAttribute("aria-pressed", "false");
  await expect(detail).toHaveAccessibleName("Detail: All");
});

test("the window does not scroll: the head stays, a popover scrolls inside itself, the toolbar keeps its row with Walk through at one edge, and a canvas fills its region", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  for (const address of ["list?frontier=1", "list"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("journey-toolbar")).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollHeight), address).toBe(800);
  }
  // A popover taller than the room below it scrolls inside itself, not the window.
  await page.setViewportSize({ width: 1280, height: 500 });
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await openFilter(page);
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(500);
  // Walk through sits on the views' row, at the same right edge on every view, between 720 and 1100px.
  await page.setViewportSize({ width: 1024, height: 700 });
  const edges: Record<string, number> = {};
  for (const address of ["walk", "graph", "list", "timeline"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    const views = await page.getByTestId("journey-views").boundingBox();
    const walk = await page.getByTestId("walk-through").boundingBox();
    expect(walk?.y, address).toBeLessThan((views?.y ?? 0) + (views?.height ?? 0));
    edges[address] = Math.round((walk?.x ?? 0) + (walk?.width ?? 0));
  }
  expect(new Set(Object.values(edges)).size, JSON.stringify(edges)).toBe(1);
  // Between 720 and 1100px the projection keeps the width and a canvas fills its region.
  const body = page.locator(".journey-body");
  for (const address of ["graph", "graph?decisions=1"]) {
    await openAt(page, "browser", "j_vendor_eval", address, { fixedToday: FIXED_TODAY });
    await expect(page.getByTestId("canvas")).toBeVisible();
    const [scrolls, shown] = await body.evaluate((region) => [region.scrollHeight, region.clientHeight]);
    expect(scrolls, address).toBe(shown);
  }
  await openAt(page, "browser", "j_vendor_eval", "list", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("list-row").first()).toBeVisible();
  expect(await body.evaluate((region) => region.scrollWidth <= region.clientWidth)).toBe(true);
  await openAt(page, "browser", "j_vendor_eval", "summary", { fixedToday: FIXED_TODAY });
  await expect(page.getByTestId("summary-upcoming")).toBeVisible();
});

test("a done row fades its content like a done card, lifts the fade under keyboard focus, and leaves the checkbox at full strength", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "list");
  const done = page.locator('[data-testid="list-row"][data-state="done"]').first();
  // Computed opacity is not inherited, so the effective one is the product up the ancestors.
  const effective = (target: Locator) =>
    target.evaluate((element) => {
      let product = 1;
      for (let at: Element | null = element; at !== null; at = at.parentElement) {
        product *= Number(getComputedStyle(at).opacity);
      }
      return product;
    });
  const title = done.locator('td[data-column="title"] a').first();
  expect(await effective(title)).toBe(0.5);
  // The checkbox shows on hover.
  await done.hover();
  expect(await effective(done.locator("td.list-check input"))).toBe(1);
  await title.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  expect(await effective(title)).toBe(1);
});
