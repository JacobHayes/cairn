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
import { lineBetween, pointOn } from "./canvas.ts";
import { expectAddress, follow, goWithin, importRoute, nodeCard, open, readable, setDepth, settled, whenSettled } from "./shell.ts";
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
  if ((await whenSettled(page, () => view(page).getAttribute("data-screen"))) !== to) {
    await page.getByTestId("view-picker").click();
    await page.getByTestId(`view-${to}`).click();
  }
  await expect(view(page)).toHaveAttribute("data-screen", to);
}

/** The first tick's day on the timeline's axis: it moves when the axis pans. */
async function firstTickDay(page: Page): Promise<string> {
  return (await whenSettled(page, () => page.getByTestId("timeline-tick").first().getAttribute("data-day"))) ?? "";
}

/** Selects `key` on the route canvas, as a click on its card does. */
async function selectCard(page: Page, key: string): Promise<void> {
  await nodeCard(page, key).getByTestId("card-open").click();
  await expect(page).toHaveURL(new RegExp(`/nodes/${key}`));
}

test("C17, C2, C9: v cycles a route's views and m does nothing; the depth and the search follow a route across Graph, List and Timeline and the address keeps them; ?mine=1 is written away", async ({ page }) => {
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await expectAddress(page, "/routes/vendor-evaluation");
  // A route has no map: v cycles its views and m does nothing.
  await page.getByTestId("route-frame").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("v");
  await expect(view(page)).not.toHaveAttribute("data-screen", "graph");
  const after = new URL(page.url()).search;
  await page.keyboard.press("m");
  expect(new URL(page.url()).search).toBe(after);
  await goToView(page, "graph");
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
  // A route has no owners: ?mine=1 lists every node and is normalised away.
  await follow(page, "/routes/vendor-evaluation?mine=1&view=list");
  await expect(page.getByTestId("route-row").first()).toBeVisible();
  await expect.poll(() => new URL(page.url()).searchParams.has("mine")).toBe(false);
  await expect(view(page)).toHaveAttribute("data-screen", "list");
});

test("C19: a draft opened on a version shows without a version in the address, and a saved link to a version stays that version; the version selector names the graph shown, and picking a version shows its card and keeps the view and the depth", async ({ page }) => {
  await publishVersionTwo(page, "browser");
  expect(new URL(page.url()).searchParams.get("version")).toBeNull();
  await openDraftHere(page);
  expect(new URL(page.url()).searchParams.get("version")).toBeNull();
  await publishDraft(page);
  // Version 2 by name stays version 2 once a third is published.
  await goWithin(page, "/routes/vendor-evaluation?version=2");
  await expect(versionSelect(page)).toContainText("Version 2");
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

/** The route timeline lays the product launch from day 0: the feature bar starts at the start line, the launch ends it, and a title opens the node. */
async function timelineLaysFromDayZero(page: Page): Promise<void> {
  await goToView(page, "timeline");
  await expect(page.getByTestId("timeline-track").first()).toBeVisible();
  const chart = page.getByTestId("timeline");
  await expect(chart).toHaveAttribute("data-end", "n_launch");
  const features = page.locator('[data-testid="timeline-entry"][data-node="n_features"]');
  await expect(features).toBeVisible();
  // Its bar starts on the start line (day 0): the bar's left edge is the line's.
  const [line, bar] = await whenSettled(page, () =>
    Promise.all([page.locator("line.timeline-today").first().boundingBox(), page.locator('[data-testid="timeline-track"][data-node="n_features"] .timeline-mark').first().boundingBox()]),
  );
  expect(line).not.toBeNull();
  expect(bar).not.toBeNull();
  expect(Math.abs((bar?.x ?? -99) - (line?.x ?? 99))).toBeLessThan(3);
  await features.getByRole("link").click();
  await expect(page).toHaveURL(/\/nodes\/n_features/);
}

test("C13: the route timeline lays the product launch from day 0, panning its axis moves it and Fit brings the span back; a track opens its node, on a draft and on a version", async ({ page }) => {
  await openRoute(page, "browser", "product-launch");
  await openDraftHere(page);
  await timelineLaysFromDayZero(page);
  const fitted = await firstTickDay(page);
  await page.getByTestId("timeline").hover();
  await page.mouse.wheel(300, 0);
  await expect.poll(() => firstTickDay(page)).not.toBe(fitted);
  await page.getByTestId("timeline-reset").click();
  await expect.poll(() => firstTickDay(page)).toBe(fitted);
  // The title link selected n_features, so a track of another node shows the track's own selection.
  await page.locator('[data-testid="timeline-track"][data-node="n_launch"] .timeline-hit').click();
  await expect(page).toHaveURL(/\/nodes\/n_launch/);
  await pickVersion(page, 1);
  await expect(page.getByTestId("version-card")).toBeVisible();
  await expect(view(page)).toHaveAttribute("data-screen", "timeline");
  await page.locator('[data-testid="timeline-track"][data-node="n_features"] .timeline-hit').click();
  await expect(page).toHaveURL(/\/routes\/product-launch\/nodes\/n_features\?version=1/);
  await expect(page.getByTestId("route-node")).toHaveAttribute("data-node", "n_features");
});

/** Selecting a node on a route draft traces it: tagged cards, faded others, and a contributor's mark; a draft edit keeps the trace on the edited graph. */
async function draftTraces(page: Page): Promise<void> {
  await selectCard(page, "n_plan_review");
  await expect(nodeCard(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  await expect(nodeCard(page, "n_baseline")).toHaveAttribute("data-trace", "unblocks");
  await expect(nodeCard(page, "n_who_owns")).toHaveCSS("opacity", "0.3");
  // A node that adds to the selected node's gravity says so on its tag; a group, which weighs nothing, does not.
  await expect(nodeCard(page, "n_baseline").getByTestId("card-mark")).toHaveAttribute("data-contributor", "");
  await expect(nodeCard(page, "n_testing").getByTestId("card-mark")).not.toHaveAttribute("data-contributor", "");
  // A draft edit while a node is traced keeps the trace on the edited graph.
  await formField(page, "title").getByRole("textbox").fill("Review the plan twice");
  await saveForm(page);
  await expect(nodeCard(page, "n_plan_review").getByTestId("title")).toHaveText("Review the plan twice");
  await expect(nodeCard(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  await expect(nodeCard(page, "n_baseline")).toHaveAttribute("data-trace", "unblocks");
}

test("C7: selecting a node on a route draft traces it, as on a journey, through an edit; Reveal and Hide, a hidden-prerequisites marker, and a click on the empty canvas clears it", async ({ page }) => {
  await readable(page);
  await openRoute(page, "browser", "vendor-evaluation");
  await openDraftHere(page);
  await draftTraces(page);
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
  const pane = (await whenSettled(page, () => page.locator(".react-flow__pane").boundingBox())) ?? { x: 0, y: 0, width: 0, height: 0 };
  await page.mouse.click(pane.x + pane.width - 12, pane.y + pane.height - 60);
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page.getByTestId("card-mark")).toHaveCount(0);
  // With nothing selected, a hidden-prerequisites marker selects its node and traces it.
  const marker = page.getByTestId("hidden-prerequisites").first();
  await expect(marker).toBeVisible();
  await marker.click();
  await expect(page).toHaveURL(/\/nodes\/n_/);
  await expect(page.getByTestId("card-mark").first()).toBeVisible();
});

/** On a version the inspector put away comes back when the same node is picked again from the List and the Timeline. */
async function putAwayInspectorReturns(page: Page): Promise<void> {
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
}

test("C7, C19: a node of a published version is selected like a draft's: its address keeps the version, its panel is read only, it traces, a search that hides it still draws it on every view, Esc clears it, and a version's node says when it applies", async ({ page }) => {
  await readable(page);
  await openRoute(page, "browser", "vendor-evaluation", "?version=1");
  await selectCard(page, "n_plan_review");
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation\/nodes\/n_plan_review\?version=1/);
  await expect(page.getByTestId("route-node")).toHaveAttribute("data-node", "n_plan_review");
  await expect(page.getByTestId("version-node")).toBeVisible();
  await expect(page.getByTestId("authoring")).toHaveCount(0);
  await expect(nodeCard(page, "n_plan_draft")).toHaveAttribute("data-trace", "needs");
  // A search that hides the selected node still draws it, on every view.
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan_review?version=1&q=zzzz-no-match");
  await expect(nodeCard(page, "n_plan_review")).toBeVisible();
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan_review?version=1&q=zzzz-no-match&view=list");
  await expect(page.locator('[data-testid="route-row"][data-node="n_plan_review"]')).toBeVisible();
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan_review?version=1&q=zzzz-no-match&view=timeline");
  await expect(page.locator('[data-testid="timeline-track"][data-node="n_plan_review"]')).toBeVisible();
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_plan_review?version=1&q=zzzz-no-match");
  await page.keyboard.press("Escape");
  await expect(page).not.toHaveURL(/\/nodes\//);
  await expect(page).toHaveURL(/version=1/);
  await goWithin(page, "/routes/vendor-evaluation?version=1");
  await putAwayInspectorReturns(page);
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_partner_results?version=1");
  // Its own node has no condition; the one it follows is its container's, named and linked.
  await expect(page.getByTestId("version-applies").locator('li[data-node="n_partner_led"]')).toContainText("If ");
  // A top-level group is a stage here as on the canvas; a nested one stays a group.
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_testing?version=1");
  await expect(page.getByTestId("route-node")).toContainText("stage");
  await goWithin(page, "/routes/vendor-evaluation/nodes/n_partner_led?version=1");
  await expect(page.getByTestId("route-node")).toContainText("group in testing");
  await page.getByTestId("version-holds").locator("summary").click();
  await expect(page.getByTestId("version-holds").locator('li[data-node="n_partner_results"]')).toBeVisible();
});

/** Clicks the line between two cards on the route canvas. */
async function clickLine(page: Page, from: string, to: string): Promise<void> {
  await settled(page);
  const at = await pointOn(lineBetween(page, from, to));
  await page.mouse.click(at.x, at.y);
}

/** On a version a line opens its card: the address keeps the version, the card says no state, a row's link opens the read-only panel, and the empty canvas clears it. */
async function versionLineOpensCard(page: Page): Promise<void> {
  await openRoute(page, "browser", "vendor-evaluation", "?version=1&depth=stages");
  await clickLine(page, "n_setup", "n_testing");
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation\/edges\/n_setup~n_testing\?.*version=1/);
  const card = page.getByTestId("edge-card");
  await expect(card).toBeVisible();
  await expect(lineBetween(page, "n_setup", "n_testing")).toHaveClass(/line-selected/);
  // The folded stages stand for the link between the nodes inside them, which has no state on a route.
  const row = card.locator('[data-requirement="n_plan"][data-dependent="n_comparison_set"]');
  await expect(row).toBeVisible();
  await expect(card.locator("[data-waits]")).toHaveCount(0);
  await row.getByRole("link", { name: "Comparison set" }).click();
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation\/nodes\/n_comparison_set\?.*version=1/);
  await expect(page.getByTestId("version-node")).toBeVisible();
  await page.goBack();
  const pane = (await whenSettled(page, () => page.locator(".react-flow__pane").boundingBox())) ?? { x: 0, y: 0, width: 0, height: 0 };
  await page.mouse.click(pane.x + pane.width - 12, pane.y + pane.height - 60);
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation\?.*version=1/);
  await expect(card).toHaveCount(0);
}

test("C1, C7, C19: a line on a route opens its card: on a version the address keeps the version and the card says no state, a row's link opens the read-only panel, and the empty canvas clears it; on a draft it takes the draft card's place, a second click brings a hidden inspector back, and Esc brings the draft card back", async ({ page }) => {
  await readable(page);
  await versionLineOpensCard(page);
  await openDraftHere(page);
  await clickLine(page, "n_access", "n_plan");
  await expect(page.getByTestId("edge-card")).toHaveAttribute("data-from", "n_access");
  await expect(page.getByTestId("draft-card")).toHaveCount(0);
  await page.keyboard.press("]");
  await expect(page.getByTestId("edge-card")).toBeHidden();
  await clickLine(page, "n_access", "n_plan");
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(/\/routes\/vendor-evaluation(\?|$)/);
  await expect(page.getByTestId("draft-card")).toBeVisible();
});

test("C1: a line that stands for many links lists the first of them and shows the rest on request", async ({ page }) => {
  // Two stages joined by sixty requirements: more than a card lists at once.
  const leaf = (stage: string, i: number, requires: string) => `- key: n_${stage}${String(i)}\n  id: ${stage}${String(i)}\n  parent: ${stage}\n  kind: action\n  title: ${stage.toUpperCase()} step ${String(i)}\n${requires}`;
  const needs = (i: number) => `  requires:\n  - a/a${String(i % 3)}\n  - a/a${String((i + 1) % 3)}\n`;
  const nodes = [0, 1, 2].map((i) => leaf("a", i, "")).join("") + Array.from({ length: 30 }, (_, i) => leaf("b", i, needs(i))).join("");
  const file = `format: 1\nroute: many-links\nname: Many links\nnodes:\n- key: n_a\n  id: a\n  kind: group\n  title: Prepare\n- key: n_b\n  id: b\n  kind: group\n  title: Deliver\n${nodes}- key: n_end\n  id: end\n  kind: milestone\n  title: Done\n  final: true\n  requires:\n  - b/b0\n`;
  await importRoute(page, "browser", { name: "many-links.yaml", mimeType: "text/yaml", buffer: Buffer.from(file) });
  await expect(versionSelect(page)).toContainText("Draft");
  await goWithin(page, "/routes/many-links/edges/n_a~n_b?depth=stages");
  const rows = page.getByTestId("edge-card").locator("[data-requirement]");
  await expect(rows.first()).toBeVisible();
  const first = await whenSettled(page, () => rows.count());
  expect(first).toBeLessThan(60);
  await page.getByTestId("edge-links-all").click();
  await expect(rows).toHaveCount(60);
  await expect(page.getByTestId("edge-links-all")).toHaveCount(0);
});
