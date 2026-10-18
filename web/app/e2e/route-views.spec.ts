// A route has a journey's views and view state (C17, C13, C2, C9): the same View picker, Depth
// and search follow it across Graph, List and Timeline, the version it shows is picked in its
// header, and its timeline is the earliest schedule from day 0. Run on the in-browser host,
// which reseeds the store on each load, so moves within a test go through links and
// `goWithin`, never a reload. What the views draw (levels, schedule, rows) is the unit tests'.
import { expect, test, type Page } from "@playwright/test";

import { openRoute, pickVersion, publishDraft, routeMenuItem, versionSelect } from "./around.ts";
import { publishVersionTwo } from "./proposals.ts";
import { expectAddress, follow, goWithin, nodeCard, open, setDepth } from "./shell.ts";
import { search } from "./view-state.ts";

/** Opens a draft on the route being shown (a version) and waits for the draft view. */
async function openDraftHere(page: Page): Promise<void> {
  await page.getByTestId("open-draft-button").click();
  await expect(versionSelect(page)).toContainText("Draft");
}

/** The route frame's current view. */
const view = (page: Page) => page.getByTestId("route-frame");

/** Goes to `to` from the header's View picker. */
async function goToView(page: Page, to: "graph" | "list" | "timeline"): Promise<void> {
  if ((await view(page).getAttribute("data-screen")) !== to) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${to}`).click();
  }
  await expect(view(page)).toHaveAttribute("data-screen", to);
}

/** The first tick's day on the timeline's axis: it moves when the axis pans. */
async function firstTickDay(page: Page): Promise<string> {
  return (await page.getByTestId("timeline-tick").first().getAttribute("data-day")) ?? "";
}

test("C17, C2, C9: the depth and the search follow a route across Graph, List and Timeline, and the address keeps them", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await expectAddress(page, "/routes/vendor-evaluation");
  await setDepth(page, "Stages");
  await search(page, "kickoff");
  for (const to of ["list", "timeline", "graph"] as const) {
    await goToView(page, to);
    await expect(page.getByTestId("depth-button")).toHaveAccessibleName("Depth: Stages");
    await expect(page.getByTestId("toolbar-search")).toHaveValue("kickoff");
    const url = new URL(page.url());
    expect(url.searchParams.get("depth")).toBe("stages");
    expect(url.searchParams.get("q")).toBe("kickoff");
    // The graph is the address's default and writes no view.
    expect(url.searchParams.get("view")).toBe(to === "graph" ? null : to);
  }
});

test("C17: ?mine=1&view=list lists every node and is normalised, since a route has no owners", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await follow(page, "/routes/vendor-evaluation?mine=1&view=list");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await expect.poll(() => new URL(page.url()).searchParams.has("mine")).toBe(false);
  await expect(view(page)).toHaveAttribute("data-screen", "list");
});

test("C17: a selected node is drawn on every view though the search hides it", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan?q=zzzz-no-match");
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan?q=zzzz-no-match&view=list");
  await expect(page.locator('[data-testid="route-row"][data-node="n_plan"]')).toBeVisible();
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan?q=zzzz-no-match&view=timeline");
  await expect(page.locator('[data-testid="timeline-track"][data-node="n_plan"]')).toBeVisible();
});

test("C19: the version selector names the graph shown; picking a version shows its card and keeps the view and the depth", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  await goToView(page, "list");
  await setDepth(page, "Stages");
  await pickVersion(page, 1);
  const url = new URL(page.url());
  expect(url.searchParams.get("version")).toBe("1");
  expect(url.searchParams.get("view")).toBe("list");
  expect(url.searchParams.get("depth")).toBe("stages");
  await expect(page.getByTestId("publish-draft")).toHaveCount(0);
  await expect(page.getByTestId("version-card")).toBeVisible();
  await expect(page.locator('[data-testid="version-journey"][data-journey="j_vendor_eval"]')).toHaveAttribute("data-upgrade", "available");
  await page.getByTestId("route-upgrades").click();
  await expect(page).toHaveURL(/upgrade=true|upgrade=1/);
  await expect(page).toHaveURL(/route=/);
});

test("C19: a draft opened on a version shows without a version in the address, and a saved link to a version stays that version", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  const two = page.url();
  expect(new URL(two).searchParams.get("version")).toBeNull();
  await openDraftHere(page);
  expect(new URL(page.url()).searchParams.get("version")).toBeNull();
  await publishDraft(page);
  // Version 2 by name stays version 2 once a third is published.
  await goWithin(page, "/routes/vendor-evaluation?version=2");
  await expect(versionSelect(page)).toContainText("Version 2");
});

test("C17: a node address with no draft open lands on the route's bare address with a notice", async ({ page }) => {
  await open(page, "browser", "/routes/vendor-evaluation/nodes/n_plan");
  await expect(page.getByTestId("route-notice")).toBeVisible();
  await expectAddress(page, "/routes/vendor-evaluation");
});

test("C19: discarding a route's only draft leaves the empty route with its header and Retire", async ({ page }) => {
  await open(page, "browser", "/routes/new");
  await page.getByLabel("Name").fill("Throwaway");
  await page.getByRole("button", { name: /create/i }).click();
  await expect(page.getByTestId("route-header")).toBeVisible();
  await routeMenuItem(page, "discard-draft");
  await expect(page.getByTestId("route-empty")).toBeVisible();
  await expect(page.getByTestId("route-header")).toBeVisible();
  await expect(page.getByTestId("route-menu")).toBeVisible();
  await page.getByTestId("route-menu").click();
  await expect(page.getByTestId("menu-retire")).toBeVisible();
});

test("C13: the route timeline lays the product launch from day 0; the feature bar starts at the start line, the launch ends it, and a title opens the node", async ({ page }) => {
  await openRoute(page, "browser", "product-launch");
  await openDraftHere(page);
  await goToView(page, "timeline");
  await expect(page.getByTestId("timeline-track").first()).toBeVisible();
  const chart = page.getByTestId("timeline");
  await expect(chart).toHaveAttribute("data-end", "n_launch");
  const features = page.locator('[data-testid="timeline-entry"][data-node="n_features"]');
  await expect(features).toBeVisible();
  // Its bar starts on the start line (day 0): the bar's left edge is the line's.
  const line = await page.locator("line.timeline-today").first().boundingBox();
  const bar = await page.locator('[data-testid="timeline-track"][data-node="n_features"] .timeline-mark').first().boundingBox();
  expect(line).not.toBeNull();
  expect(bar).not.toBeNull();
  expect(Math.abs((bar?.x ?? -99) - (line?.x ?? 99))).toBeLessThan(3);
  await features.getByRole("link").click();
  await expect(page).toHaveURL(/\/nodes\/n_features/);
});

test("C13: panning the route's axis moves it and Fit brings the span back; a draft's track opens its node and a version's opens nothing", async ({ page }) => {
  await openRoute(page, "browser", "product-launch");
  await openDraftHere(page);
  await goToView(page, "timeline");
  const fitted = await firstTickDay(page);
  await page.getByTestId("timeline").hover();
  await page.mouse.wheel(300, 0);
  await expect.poll(() => firstTickDay(page)).not.toBe(fitted);
  await page.getByTestId("timeline-reset").click();
  await expect.poll(() => firstTickDay(page)).toBe(fitted);
  await page.locator('[data-testid="timeline-track"][data-node="n_features"] .timeline-hit').click();
  await expect(page).toHaveURL(/\/nodes\/n_features/);
  await pickVersion(page, 1);
  await expect(page.getByTestId("version-card")).toBeVisible();
  await expect(view(page)).toHaveAttribute("data-screen", "timeline");
  await page.locator('[data-testid="timeline-track"][data-node="n_features"] .timeline-hit').click();
  expect(new URL(page.url()).pathname).toBe("/routes/product-launch");
});

test("C9, C17: v cycles a route's views and m does nothing, since a route has no map", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await page.getByTestId("route-frame").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(view(page)).not.toHaveAttribute("data-screen", "graph");
  const after = new URL(page.url()).search;
  await page.keyboard.press("m");
  expect(new URL(page.url()).search).toBe(after);
});
