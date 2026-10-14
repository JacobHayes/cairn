// A journey's page in a real browser (2.2 to 2.4), on the in-browser host: what only focus,
// layout and the window show. The keys act on the view the pointer left them on (j, Enter, Esc,
// v), the Frontier chip turns the lens on, and no view scrolls the window: the head stays, a
// popover taller than the room scrolls inside itself, the toolbar keeps its row, and a canvas
// fills its region. The addresses, chips, filters and memory behind them are vitest's
// (journeys/address.test.ts and screens/filters.test.ts).
import { expect, test } from "@playwright/test";

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
  // The list is the frontier in rank order, numbered by rank, with as many rows as the chip counts.
  const places = await page.locator('[data-testid="list-row"] [data-column="rank"]').allTextContents();
  expect(places).toEqual(places.map((_, at) => `#${String(at + 1)}`));
  const counted = await page.getByTestId("chip-frontier").locator(".chip-count").textContent();
  await expect(page.getByTestId("list-total")).toHaveAttribute("data-total", counted ?? "");
});

test("a view comes back as it was left: the graph's expanded stage, pan and zoom survive a visit to the list", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  await page.keyboard.press("f");
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await expect(page).toHaveURL(/open=n_setup/);
  await viewportSettled(page);
  await page.mouse.move(600, 500);
  await page.mouse.wheel(0, 150);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");
  await viewportSettled(page);
  const left = await viewportOf(page);
  await page.getByTestId("view-list").click();
  await page.getByTestId("view-graph").click();
  await expect(page).toHaveURL(/\/graph\?open=n_setup$/);
  await expect(page.getByTestId("node-card").first()).toBeVisible();
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(left);
  // A change of detail level is a new view: it fits, even to a level whose pan was kept before.
  const ladder = page.getByTestId("ladder");
  const canvas = page.getByTestId("canvas");
  const before = (await canvas.getAttribute("data-view")) ?? "";
  await ladder.getByText("All", { exact: true }).click();
  // The new level draws, and fits, before its viewport is read: until then it is the default one.
  await expect(canvas).not.toHaveAttribute("data-view", before);
  await expect.poll(async () => (await viewportOf(page)).zoom).not.toBe(1);
  await viewportSettled(page);
  const fitted = await viewportOf(page);
  await page.mouse.move(600, 500);
  await page.mouse.wheel(0, 150);
  await viewportSettled(page);
  expect(await viewportOf(page)).not.toEqual(fitted);
  const allView = (await canvas.getAttribute("data-view")) ?? "";
  await ladder.getByText("Stages", { exact: true }).click();
  // The drawn canvas has to switch before All is chosen again, or the second choice finds nothing changed.
  await expect(canvas).not.toHaveAttribute("data-view", allView);
  await ladder.getByText("All", { exact: true }).click();
  await expect(canvas).toHaveAttribute("data-view", allView);
  await viewportSettled(page);
  expect(await viewportOf(page)).toEqual(fitted);
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
