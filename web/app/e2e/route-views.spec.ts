// A route has a journey's views and view state (C17, C13, C2, C9): the same View picker, Depth
// and search follow it across Graph, List and Timeline, the version it shows is picked in its
// header, and its timeline is the earliest schedule from day 0. Run on the in-browser host,
// which reseeds the store on each load, so moves within a test go through links and
// `goWithin`, never a reload. What the views draw (levels, schedule, rows) is the unit tests'.
import type { Page } from "@playwright/test";

import { expect, test } from "./test.ts";

import { openRoute, pickVersion, publishDraft, routeMenuItem, versionSelect } from "./around.ts";
import { formField, saveForm } from "./authoring.ts";
import { publishVersionTwo } from "./proposals.ts";
import { publishSegment } from "./segments.ts";
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
  // Emptying the box takes the search off at once, on any view: no filter stays on unseen.
  await page.getByTestId("toolbar-search").fill("");
  await expect.poll(() => new URL(page.url()).searchParams.has("q")).toBe(false);
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

test("C13: panning the route's axis moves it and Fit brings the span back; a track opens its node, on a draft and on a version", async ({ page }) => {
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
  await expect(page).toHaveURL(/\/routes\/product-launch\/nodes\/n_features\?version=1/);
  await expect(page.getByTestId("route-node")).toHaveAttribute("data-node", "n_features");
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

/** Selects `key` on the route canvas, as a click on its card does. */
async function selectCard(page: Page, key: string): Promise<void> {
  await nodeCard(page, key).getByTestId("card-open").click();
  await expect(page).toHaveURL(new RegExp(`/nodes/${key}`));
}

/** The trace tag a card carries: its `data-trace`. */
const traceOf = (page: Page, key: string) => nodeCard(page, key);

test("C7: selecting a node on a route draft traces it, as on a journey: tagged cards, faded others, a contributor's mark, Reveal and Hide, and a click on the empty canvas clears it", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await selectCard(page, "n_plan_review");
  await expect(traceOf(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  await expect(traceOf(page, "n_baseline")).toHaveAttribute("data-trace", "unblocks");
  await expect(nodeCard(page, "n_who_owns")).toHaveCSS("opacity", "0.3");
  // A node that adds to the selected node's gravity says so on its tag; a group, which weighs nothing, does not.
  await expect(nodeCard(page, "n_baseline").getByTestId("card-mark")).toHaveAttribute("data-contributor", "");
  await expect(nodeCard(page, "n_testing").getByTestId("card-mark")).not.toHaveAttribute("data-contributor", "");
  // At Stages a folded stage traces what is folded into it; Reveal draws the nodes it reaches and Hide takes them off.
  await setDepth(page, "Stages");
  await selectCard(page, "n_setup");
  await expect(nodeCard(page, "n_reporting").getByTestId("card-mark")).toHaveText("unblocks");
  await expect(page.getByTestId("trace-reveal")).toContainText("Reveal");
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveText("Hide revealed nodes");
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toContainText("Reveal");
  // The empty canvas clears the selection and its marks.
  const pane = (await page.locator(".react-flow__pane").boundingBox()) ?? { x: 0, y: 0, width: 0, height: 0 };
  await page.mouse.click(pane.x + pane.width - 12, pane.y + pane.height - 60);
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page.getByTestId("card-mark")).toHaveCount(0);
});

test("C7: a draft edit while a node is traced keeps the trace on the edited graph", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await selectCard(page, "n_plan_review");
  await expect(traceOf(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  await formField(page, "title").getByRole("textbox").fill("Review the plan twice");
  await saveForm(page);
  await expect(nodeCard(page, "n_plan_review").getByTestId("title")).toHaveText("Review the plan twice");
  await expect(traceOf(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  await expect(traceOf(page, "n_baseline")).toHaveAttribute("data-trace", "unblocks");
});

test("C7: a hidden-prerequisites marker selects its node and traces it", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await setDepth(page, "Stages");
  const marker = page.getByTestId("hidden-prerequisites").first();
  await expect(marker).toBeVisible();
  await marker.click();
  await expect(page).toHaveURL(/\/nodes\/n_/);
  await expect(page.getByTestId("card-mark").first()).toBeVisible();
});

test("C7, C19: a node of a published version is selected like a draft's: its address keeps the version, its panel is read only, it traces, and Esc clears it", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation", "?version=1");
  await selectCard(page, "n_plan_review");
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation\/nodes\/n_plan_review\?version=1/);
  await expect(page.getByTestId("route-node")).toHaveAttribute("data-node", "n_plan_review");
  await expect(page.getByTestId("version-node")).toBeVisible();
  await expect(page.getByTestId("authoring")).toHaveCount(0);
  await expect(traceOf(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  // A search that hides the selected node still draws it.
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan_review?version=1&q=zzzz-no-match");
  await expect(nodeCard(page, "n_plan_review")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page).toHaveURL(/version=1/);
});

test("C19: a version's node says when it applies including what its container's condition says, and a container lists what it holds", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation", "?version=1");
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_partner_results?version=1");
  // Its own node has no condition; the one it follows is its container's, named and linked.
  await expect(page.getByTestId("version-applies").locator('li[data-node="n_partner_led"]')).toContainText("If ");
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_partner_led?version=1");
  await page.getByTestId("version-holds").locator("summary").click();
  await expect(page.getByTestId("version-holds").locator('li[data-node="n_partner_results"]')).toBeVisible();
});

test("C7, C19: on a version, the inspector put away comes back when the same node is picked again from the List and the Timeline", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation", "?version=1");
  await selectCard(page, "n_plan_review");
  await expect(page.getByTestId("route-node")).toBeVisible();
  await page.keyboard.press("]");
  await expect(page.getByTestId("route-node")).toBeHidden();
  await goToView(page, "list");
  await page.locator('[data-testid="route-row"][data-node="n_plan_review"] [data-testid="row-open"]').click();
  await expect(page.getByTestId("route-node")).toBeVisible();
  await page.keyboard.press("]");
  await goToView(page, "timeline");
  await page.locator('[data-testid="timeline-track"][data-node="n_plan_review"] .timeline-hit').click();
  await expect(page.getByTestId("route-node")).toBeVisible();
});

test("C7: a segment's page traces a selected node as a route's does", async ({ page }) => {
  await publishSegment(page, "browser");
  const first = page.getByTestId("node-card").first();
  await first.getByTestId("card-open").click();
  await expect(page).toHaveURL(/\/nodes\/n_/);
  await expect(page.getByTestId("route-node")).toBeVisible();
  await expect(page.getByTestId("card-mark").first()).toBeVisible();
});

test("the Key to the lines opens from the route canvas", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await page.getByTestId("key-menu").click();
  await expect(page.getByTestId("canvas-key")).toBeVisible();
});
