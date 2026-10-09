// The journey canvas over the vendor evaluation and the hiring loop (C1 to C7, C15): the level's
// node set with the actions hidden, drilling in and out, the heat toggle, the trace of the test
// plan, the hidden-prerequisites marker when decisions are hidden and the trace it opens, the
// stalled surface, and the layout: a toggled view as a direct one, and one node added on the
// fixture in edit mode.
import { expect, test, type Page } from "@playwright/test";

import { LAYOUT_MOVED_FRACTION_MAX } from "../src/canvas/layout.ts";
import { addNode, openEditing } from "./authoring.ts";
import { cardKeys, containers, lineBetween, marks, places, showKind, toggle } from "./canvas.ts";
import { menuItem, nodeCard, nodePanel, openFromCanvas, openJourney, renameOf, startRename } from "./shell.ts";

/** The vendor evaluation at the end of its scenario, with its actions hidden: the level's node set, each node in its container. */
const ACTIONS_HIDDEN: Record<string, string> = {
  n_decision_meeting: "top", n_kickoff: "top", n_meeting_date: "top", n_partner_runs: "top", n_purpose: "top",
  n_reporting: "top", n_final_review: "n_reporting", n_final_report: "n_final_review", n_findings: "n_reporting",
  n_findings_reviewer: "n_reporting", n_review_opens: "n_reporting", n_setup: "top", n_access: "n_setup",
  n_plan: "n_setup", n_workload: "n_setup", n_workload_ingest: "n_workload", n_workload_query: "n_workload",
  n_testing: "top", n_baseline: "n_testing", n_comparison_set: "n_testing", n_partner_led: "n_testing",
  n_who_informed: "top", n_who_owns: "top",
};

test("C2: hiding the actions renders the level's node set", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?kind=group,decision,deliverable,milestone");
  await expect.poll(() => containers(page)).toEqual(ACTIONS_HIDDEN);
  expect(await cardKeys(page)).toEqual(Object.keys(ACTIONS_HIDDEN).sort());
});

test("C2, C4: hidden actions are their deliverable's checklist, and a hidden prerequisite is marked", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?kind=group,decision,deliverable,milestone");
  const checklist = nodeCard(page, "n_plan").getByTestId("card-checklist").locator("li");
  await expect(checklist).toHaveText(["Draft the plan", "Review the plan"]);
  await showKind(page, "group", false);
  await showKind(page, "milestone", false);
  await showKind(page, "action", true);
  await expect(nodeCard(page, "n_final_report").getByTestId("hidden-prerequisites")).toHaveAttribute("data-nodes", "n_review_opens");
});

test("C4: drill into a container and back out", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await nodeCard(page, "n_setup").getByTestId("card-drill").click();
  await expect(page.getByTestId("crumb-current")).toHaveAttribute("data-node", "n_setup");
  await expect.poll(() => cardKeys(page)).toEqual(["n_access", "n_plan", "n_plan_draft", "n_plan_review", "n_workload", "n_workload_ingest", "n_workload_query"]);
  await page.getByTestId("crumbs").getByRole("link", { name: "Whole journey" }).click();
  await expect(page.getByTestId("node-card")).toHaveCount(27);
});

test("C1: explicit edges solid, implicit gates dotted with their source; not relevant grayed and hideable", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(lineBetween(page, "n_access", "n_plan")).toHaveAttribute("data-dash", "solid");
  for (const [from, to] of [["n_kickoff", "n_setup"], ["n_partner_runs", "n_partner_led"], ["n_comparison_set", "n_baseline"]] as const) {
    await expect(lineBetween(page, from, to)).not.toHaveAttribute("data-dash", "solid");
    await expect(page.locator(`[data-testid=edge-source][data-edge="${from}->${to}"]`)).toBeVisible();
  }
  const partnerLed = nodeCard(page, "n_partner_led");
  await expect(partnerLed).toHaveAttribute("data-relevance", "not_relevant");
  await expect(partnerLed).toHaveClass(/node-not-relevant/);
  await toggle(page, "not-relevant", false);
  await expect(partnerLed).toHaveCount(0);
  await expect(nodeCard(page, "n_partner_results")).toHaveCount(0);
  await expect(nodeCard(page, "n_testing")).toBeVisible();
});

test("C6: heat shows numbers", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("card-heat")).toHaveCount(0);
  await toggle(page, "heat", true);
  await expect(nodeCard(page, "n_review_opens").getByTestId("card-heat")).toContainText("gravity");
});

test("C7: the trace of the test plan starts and stops", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_plan");
  await page.getByTestId("trace-start").click();
  await expect(page.getByTestId("trace-bar")).toBeVisible();
  await expect.poll(() => marks(page)).not.toEqual({});
  await page.getByTestId("trace-stop").click();
  await expect.poll(() => marks(page)).toEqual({});
});

/** Reopens the hiring loop's offer decision, so the offer and the close-out wait on it again. */
async function reopenOffer(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_hiring");
  const panel = await openFromCanvas(page, "n_make_offer");
  await menuItem(panel, "reopen");
  await expect(nodeCard(page, "n_offer")).toHaveAttribute("data-relevance", "undecided");
}

test("C1, C2: hiding decisions marks the work they block, the marker opens the trace, and hiding undecided nodes leaves the decision", async ({ page }) => {
  await reopenOffer(page);
  await expect(nodeCard(page, "n_close_out")).toHaveClass(/node-undecided/);
  await showKind(page, "decision", false);
  for (const node of ["n_offer", "n_close_out"]) {
    await expect(nodeCard(page, node).getByTestId("hidden-prerequisites")).toHaveAttribute("data-nodes", "n_make_offer");
  }
  await nodeCard(page, "n_close_out").getByTestId("hidden-prerequisites").click();
  await expect(nodePanel(page, "n_close_out")).toBeVisible();
  await expect(page.getByTestId("trace-bar")).toContainText("Make an offer");
  await expect(nodeCard(page, "n_close_out")).toHaveAttribute("data-trace", "traced");
  // Hiding undecided nodes takes them off and leaves the decision.
  await showKind(page, "decision", true);
  await toggle(page, "undecided", false);
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

test("C15: a view toggled to lays out as it does when opened directly", async ({ context }) => {
  const [toggled, direct] = [await context.newPage(), await context.newPage()];
  await openJourney(toggled, "browser", "j_vendor_eval");
  await showKind(toggled, "group", false);
  await expect(nodeCard(toggled, "n_setup")).toHaveCount(0);
  await openJourney(direct, "browser", "j_vendor_eval", "?kind=decision,deliverable,action,milestone");
  await expect.poll(() => places(toggled)).toEqual(await places(direct));
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
  await expect(nodeCard(page, "n_plan").getByTestId("card-checklist").locator("li")).toHaveText(["Draft the plan", "Review the plan"]);
  await nodeCard(page, "n_setup").getByTestId("card-drill").click();
  await expect.poll(() => cardKeys(page)).toEqual(["n_access", "n_plan", "n_workload"]);
});
