// The journey canvas over the vendor evaluation and the hiring loop (C1 to C7, C15): the ladder's
// node set, edges and the not-relevant nodes it hides, the Signals lens, the flow of reading the
// graph (it opens at its current stage, expands in place, traces a selection, follows an edge,
// pans and zooms), conditional nodes, the stalled surface, and the layout: a step clicked to as
// a direct one, and one node added on the fixture in edit mode.
import { expect, test, type Page } from "@playwright/test";

import { LAYOUT_MOVED_FRACTION_MAX } from "../src/canvas/layout.ts";
import { addNode, openEditing } from "./authoring.ts";
import { cardKeys, containers, lineBetween, places, pointOn, showKind, toggle, viewportOf } from "./canvas.ts";
import { menuItem, nodeCard, nodePanel, openFromCanvas, openJourney, renameOf, startRename } from "./shell.ts";

/** The vendor evaluation at the end of its scenario, at the Work step: the level's node set, each node in its container. */
const WORK: Record<string, string> = {
  n_decision_meeting: "top", n_kickoff: "top", n_meeting_date: "top", n_partner_runs: "top", n_purpose: "top",
  n_reporting: "top", n_final_review: "n_reporting", n_final_report: "n_final_review", n_findings: "n_reporting",
  n_findings_reviewer: "n_reporting", n_review_opens: "n_reporting", n_setup: "top", n_access: "n_setup",
  n_plan: "n_setup", n_workload: "n_setup", n_workload_ingest: "n_workload", n_workload_query: "n_workload",
  n_testing: "top", n_baseline: "n_testing", n_comparison_set: "n_testing",
  n_who_informed: "top", n_who_owns: "top",
};

test("C2: the Work step draws the level's node set, and the hidden actions are in their deliverable's progress", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?detail=work");
  await expect.poll(() => containers(page)).toEqual(WORK);
  expect(await cardKeys(page)).toEqual(Object.keys(WORK).sort());
  await expect(nodeCard(page, "n_plan").getByTestId("card-progress")).toContainText("2 of 2");
});

test("C1: requirements are solid, gates dotted; settled not-relevant nodes are hidden, counted, and shown on request", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?detail=work");
  await expect(lineBetween(page, "n_access", "n_plan")).toHaveAttribute("data-dash", "solid");
  await expect(lineBetween(page, "n_kickoff", "n_setup")).toHaveAttribute("data-marker", "bar");
  await expect(lineBetween(page, "n_comparison_set", "n_baseline")).not.toHaveAttribute("data-dash", "solid");
  await expect(nodeCard(page, "n_partner_led")).toHaveCount(0);
  await expect(page.getByTestId("hidden-count")).toContainText("not relevant hidden");
  await page.getByTestId("hidden-show").click();
  await expect(nodeCard(page, "n_partner_led")).toHaveAttribute("data-relevance", "not_relevant");
  await expect(page.getByTestId("hidden-count")).toHaveCount(0);
});

test("C6: the Signals lens puts a labelled number on each card, and its chip turns it off", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("card-lens")).toHaveCount(0);
  await page.getByTestId("view-menu").click();
  await page.getByTestId("signals-choice").getByText("Gravity").click();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("card-lens").first()).toContainText("Gravity");
  await page.getByRole("button", { name: "Turn the signals off" }).click();
  await expect(page.getByTestId("card-lens")).toHaveCount(0);
});

test("C2 to C7: the graph opens at its current stage, expands in place, traces a selection, follows an edge, and pans and zooms", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "");
  // It opens at Stages: the stage holding the work is open and says so, the others fold.
  await expect(page.getByTestId("ladder").getByLabel("Stages")).toBeChecked();
  await expect(nodeCard(page, "n_reporting").getByTestId("card-current")).toBeVisible();
  // It opens where the work is, at a size titles can be read at; the Fit button shows the rest.
  expect((await viewportOf(page)).zoom).toBeGreaterThanOrEqual(0.6);
  await page.keyboard.press("f");
  await expect(nodeCard(page, "n_setup").getByTestId("card-expand")).toBeVisible();
  await expect(page.locator(".react-flow__attribution")).toHaveCount(0);
  // A folded stage traces what the nodes folded into it trace: the line leaving it is followed.
  await nodeCard(page, "n_setup").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-bar")).not.toContainText("unblocks 0");
  // Expanding opens it in place, and the address remembers.
  await nodeCard(page, "n_setup").getByTestId("card-expand").click();
  await expect(nodeCard(page, "n_plan")).toBeVisible();
  await expect(page).toHaveURL(/open=n_setup/);
  // At far zoom a card's glyph, not its colour, names its state: no two states share one.
  const glyphs = await page.getByTestId("card-glyph").evaluateAll((all) => all.map((each) => [each.getAttribute("data-state"), each.textContent]));
  expect(new Set(glyphs.map(([, glyph]) => glyph)).size).toBe(new Set(glyphs.map(([state]) => state)).size);
  // Selecting a node traces it: what it needs and what it unblocks are tagged, and the bar says how many.
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(page.getByTestId("trace-bar")).toContainText("Test plan");
  await expect(page.getByTestId("card-mark").filter({ hasText: "needs" }).first()).toBeVisible();
  await expect(page.getByTestId("card-mark").filter({ hasText: "unblocks" }).first()).toBeVisible();
  // What the step folds away is counted, and Reveal draws it.
  await page.getByTestId("trace-reveal").click();
  await expect(page.getByTestId("trace-reveal")).toHaveCount(0);
  // Hovering an edge says what it is; clicking it opens its card, which follows it to either end.
  const edge = lineBetween(page, "n_access", "n_plan");
  const at = await pointOn(edge);
  await page.mouse.move(at.x, at.y);
  await expect(page.getByTestId("edge-hover")).toContainText("Test plan needs Environment access");
  await page.mouse.click(at.x, at.y);
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await page.getByRole("link", { name: "Go to Environment access" }).click();
  await expect(nodePanel(page, "n_access")).toBeVisible();
  // A wheel pans and never zooms; with the control key it zooms. A shift-drag outside the Select mode draws no lasso.
  const [start, pointer] = [await viewportOf(page), { x: 600, y: 500 }];
  await page.mouse.move(pointer.x, pointer.y);
  await page.mouse.wheel(0, 120);
  await expect.poll(async () => (await viewportOf(page)).y).not.toBe(start.y);
  expect((await viewportOf(page)).zoom).toBe(start.zoom);
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -120);
  await page.keyboard.up("Control");
  await expect.poll(async () => (await viewportOf(page)).zoom).not.toBe(start.zoom);
  await page.keyboard.down("Shift");
  await page.mouse.down();
  await page.mouse.move(pointer.x + 120, pointer.y + 80);
  await expect(page.locator(".react-flow__selection")).toHaveCount(0);
  await page.mouse.up();
  await page.keyboard.up("Shift");
});

/** Reopens the hiring loop's offer decision, so the offer and the close-out wait on it again. */
async function reopenOffer(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_hiring");
  const panel = await openFromCanvas(page, "n_make_offer");
  await menuItem(panel, "reopen");
  await expect(nodeCard(page, "n_offer")).toHaveAttribute("data-relevance", "undecided");
}

test("C1, C2: nodes waiting on an open decision are conditional, and the filter leaves them out", async ({ page }) => {
  await reopenOffer(page);
  await expect(nodeCard(page, "n_close_out")).toHaveAttribute("data-state", "conditional");
  await toggle(page, "conditional", false);
  await expect(nodeCard(page, "n_offer")).toHaveCount(0);
  await expect(nodeCard(page, "n_close_out")).toHaveCount(0);
  await expect(nodeCard(page, "n_make_offer")).toBeVisible();
});

test("C5, D5: the stalled surface names what the journey waits on", async ({ page }) => {
  await openJourney(page, "browser", "j_hiring");
  await expect(page.getByTestId("stalled")).toHaveCount(0);
  const panel = await openFromCanvas(page, "n_offer");
  await menuItem(panel, "snooze");
  const snooze = panel.getByTestId("snooze");
  await snooze.getByLabel("A date").check();
  const until = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
  await snooze.getByLabel("Snooze until").fill(until);
  await snooze.getByRole("button", { name: "Snooze", exact: true }).click();
  const stalled = page.getByTestId("stalled");
  await expect(stalled).toBeVisible();
  await expect(stalled.getByTestId("stall-cause")).toHaveAttribute("data-status", "snooze");
  await expect(stalled.getByTestId("stall-cause")).toContainText(until);
});

test("C15: a step clicked to lays out as it does when opened directly", async ({ context }) => {
  const [clicked, direct] = [await context.newPage(), await context.newPage()];
  await openJourney(clicked, "browser", "j_vendor_eval", "");
  await clicked.getByTestId("ladder").getByText("Work", { exact: true }).click();
  await expect(nodeCard(clicked, "n_workload")).toBeVisible();
  await openJourney(direct, "browser", "j_vendor_eval", "?detail=work");
  await expect.poll(() => places(clicked)).toEqual(await places(direct));
});

test("a rename started on one node does not follow the panel to another", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await startRename(page, "n_access", "Not the plan");
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(nodePanel(page, "n_plan")).toBeVisible();
  await expect(renameOf(page, "n_plan")).toHaveCount(0);
  await nodeCard(page, "n_access").getByTestId("card-open").click();
  await expect(renameOf(page, "n_access").getByRole("textbox")).toHaveValue("Not the plan");
});

test("C15: adding one node to the fixture moves fewer than the stated fraction of its nodes", async ({ page }) => {
  await openEditing(page, "browser", "j_vendor_eval");
  const before = await places(page);
  const key = await addNode(page, "action", "Share the findings", "Reporting");
  await expect(nodeCard(page, key)).toBeVisible();
  const after = await places(page);
  const moved = Object.keys(before).filter((node) => {
    const [was, now] = [before[node], after[node]];
    return was !== undefined && now !== undefined && (Math.abs(was.x - now.x) > 1 || Math.abs(was.y - now.y) > 1 || was.parent !== now.parent);
  });
  expect(moved.length / Object.keys(before).length, moved.join(" ")).toBeLessThan(LAYOUT_MOVED_FRACTION_MAX);
});

test("a route's canvas draws its graph with no journey state, by the same rules", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await page.getByTestId("card-lineage").locator("summary").click();
  await page.getByTestId("lineage").click();
  await expect(page.getByTestId("route-graph")).toHaveAttribute("data-status", "1");
  await expect(page.getByTestId("node-card")).toHaveCount(25);
  await expect(page.locator("[data-testid=node-card][data-relevance]")).toHaveCount(0);
  await expect(page.getByTestId("card-state")).toHaveCount(0);
  await expect(lineBetween(page, "n_partner_runs", "n_partner_led")).not.toHaveAttribute("data-dash", "solid");
  await showKind(page, "action", false);
  await nodeCard(page, "n_setup").getByTestId("card-drill").click();
  await expect.poll(() => cardKeys(page)).toEqual(["n_access", "n_plan", "n_workload"]);
});
