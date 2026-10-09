// The journey canvas over the vendor evaluation and the hiring loop (C1 to C7, C15): each
// toggle combination 2.6's proof documents renders the level's node set, drilling in and out,
// the trace of the test plan, the hidden-prerequisites marker when decisions are hidden and
// the trace it opens, the stalled surface, and the layout: the same graph twice, and one node
// added on the fixture in edit mode.
import { expect, test, type Page } from "@playwright/test";

import { LAYOUT_MOVED_FRACTION_MAX } from "../src/canvas/layout.ts";
import { addNode, openEditing } from "./authoring.ts";
import { cardKeys, containers, lineBetween, marks, places, showKind, toggle } from "./canvas.ts";
import { section } from "./detail.ts";
import { derivedRevision, fresh, nodeCard, nodePanel, openFromCanvas, openJourney, rename, renameOf, startRename } from "./shell.ts";

/** The vendor evaluation at the end of its scenario, as 2.6's proof walks it at its own steps. */
const COMBINATIONS: { name: string; query: string; nodes: Record<string, string> }[] = [
  {
    name: "actions hidden",
    query: "?hide=action",
    nodes: {
      n_decision_meeting: "top", n_kickoff: "top", n_meeting_date: "top", n_partner_runs: "top", n_purpose: "top",
      n_reporting: "top", n_final_review: "n_reporting", n_final_report: "n_final_review", n_findings: "n_reporting",
      n_findings_reviewer: "n_reporting", n_review_opens: "n_reporting", n_setup: "top", n_access: "n_setup",
      n_plan: "n_setup", n_workload: "n_setup", n_workload_ingest: "n_workload", n_workload_query: "n_workload",
      n_testing: "top", n_baseline: "n_testing", n_comparison_set: "n_testing", n_partner_led: "n_testing",
      n_who_informed: "top", n_who_owns: "top",
    },
  },
  {
    name: "groups only",
    query: "?hide=decision,deliverable,action,milestone",
    nodes: { n_reporting: "top", n_final_review: "n_reporting", n_setup: "top", n_testing: "top", n_partner_led: "n_testing" },
  },
  {
    name: "drilled into Setup, every kind",
    query: "?in=n_setup",
    nodes: {
      n_access: "top", n_plan: "top", n_plan_draft: "n_plan", n_plan_review: "n_plan", n_workload: "top",
      n_workload_ingest: "n_workload", n_workload_query: "n_workload",
    },
  },
  {
    name: "groups and milestones hidden",
    query: "?hide=group,milestone",
    nodes: {
      n_meeting_date: "top", n_partner_runs: "top", n_purpose: "top", n_final_report: "top", n_findings: "top",
      n_findings_reviewer: "top", n_access: "top", n_plan: "top", n_plan_draft: "n_plan", n_plan_review: "n_plan",
      n_workload: "top", n_workload_ingest: "n_workload", n_workload_query: "n_workload", n_baseline: "top",
      n_comparison_set: "top", n_criteria: "top", n_partner_results: "top", n_who_informed: "top", n_who_owns: "top",
    },
  },
];

for (const combination of COMBINATIONS) {
  test(`C2: ${combination.name} renders the level's node set`, async ({ page }) => {
    await openJourney(page, "browser", "j_vendor_eval", combination.query);
    await expect.poll(() => containers(page)).toEqual(combination.nodes);
    expect(await cardKeys(page)).toEqual(Object.keys(combination.nodes).sort());
  });
}

test("C2, C4: hidden actions are their deliverable's checklist, and a hidden prerequisite is marked", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval", "?hide=action");
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
  await expect.poll(() => cardKeys(page)).toEqual(Object.keys(COMBINATIONS[2]?.nodes ?? {}).sort());
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

test("C5, C6: the frontier is marked with rank badges; gravity weighs borders; heat shows numbers", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  const here = page.locator("[data-testid=node-card][data-here*='actionable now']");
  await expect(here).toHaveCount(2);
  const ranked = await page.getByTestId("card-rank").allInnerTexts();
  expect(ranked.sort()).toEqual(["1", "2"]);
  for (const node of ["n_review_opens", "n_decision_meeting"]) {
    await expect(nodeCard(page, node).getByTestId("card-rank")).toBeVisible();
  }
  const borders = await page.getByTestId("node-card").evaluateAll((cards) => cards.map((card) => card.getAttribute("data-border")));
  expect(new Set(borders).size).toBeGreaterThan(1);
  await expect(page.getByTestId("card-heat")).toHaveCount(0);
  await toggle(page, "heat", true);
  await expect(nodeCard(page, "n_review_opens").getByTestId("card-heat")).toContainText("gravity");
});

test("C7: the trace of the test plan marks its upstream, downstream, and gravity contributors", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_plan");
  await page.getByTestId("trace-start").click();
  await expect(page.getByTestId("trace-bar")).toBeVisible();
  await expect.poll(() => marks(page)).toEqual({
    n_plan: "traced",
    n_access: "upstream",
    n_kickoff: "upstream",
    n_plan_draft: "upstream",
    n_plan_review: "upstream",
    n_baseline: "downstream",
    n_comparison_set: "downstream",
    n_final_report: "gravity contributor",
    n_final_review: "downstream",
    n_findings: "downstream",
    n_findings_reviewer: "downstream",
    n_reporting: "downstream",
    n_review_opens: "gravity contributor",
    n_setup: "downstream",
    n_testing: "downstream",
  });
  await expect(nodeCard(page, "n_purpose")).toHaveClass(/node-dim/);
  await page.getByTestId("trace-stop").click();
  await expect.poll(() => marks(page)).toEqual({});
});

/** Reopens the hiring loop's offer decision, so the offer and the close-out wait on it again. */
async function reopenOffer(page: Page): Promise<void> {
  await openJourney(page, "browser", "j_hiring");
  const panel = await openFromCanvas(page, "n_make_offer");
  await panel.getByTestId("actions").getByRole("button", { name: "Reopen" }).click();
  await expect(nodeCard(page, "n_offer")).toHaveAttribute("data-relevance", "undecided");
}

test("C1, C2: hiding decisions marks the work they block, and the marker opens the trace", async ({ page }) => {
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
});

test("C1: hiding undecided nodes takes them off and leaves the decision", async ({ page }) => {
  await reopenOffer(page);
  await toggle(page, "undecided", false);
  await expect(nodeCard(page, "n_offer")).toHaveCount(0);
  await expect(nodeCard(page, "n_close_out")).toHaveCount(0);
  await expect(nodeCard(page, "n_make_offer")).toBeVisible();
});

test("C5, D5: the stalled surface names what the journey waits on", async ({ page }) => {
  await openJourney(page, "browser", "j_hiring");
  await expect(page.getByTestId("stalled")).toHaveCount(0);
  const panel = await openFromCanvas(page, "n_offer");
  const blocking = await section(panel, "blocking");
  await blocking.getByRole("button", { name: "Snooze until a date" }).click();
  const until = new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10);
  await blocking.getByLabel("Snooze until").fill(until);
  await blocking.getByTestId("snooze").getByRole("button", { name: "Save" }).click();
  const stalled = page.getByTestId("stalled");
  await expect(stalled).toBeVisible();
  await expect(stalled.getByTestId("stall-cause")).toHaveAttribute("data-status", "snooze");
  await expect(stalled.getByTestId("stall-cause")).toContainText(until);
});

test("C15: the same graph lays out identically twice", async ({ context }) => {
  const [one, two] = [await context.newPage(), await context.newPage()];
  await openJourney(one, "browser", "j_vendor_eval");
  await openJourney(two, "browser", "j_vendor_eval");
  const first = await places(one);
  expect(Object.keys(first)).toHaveLength(27);
  expect(await places(two)).toEqual(first);
});

test("C15: a view toggled to lays out as it does when opened directly", async ({ context }) => {
  const [toggled, direct] = [await context.newPage(), await context.newPage()];
  await openJourney(toggled, "browser", "j_vendor_eval");
  await showKind(toggled, "group", false);
  await expect(nodeCard(toggled, "n_setup")).toHaveCount(0);
  await openJourney(direct, "browser", "j_vendor_eval", "?hide=group");
  await expect.poll(() => places(toggled)).toEqual(await places(direct));
});

// Every error event the window sees, including the browser's own reports (a ResizeObserver
// loop) that never reach Playwright's pageerror.
test("an edit redraws the canvas with no error in the page", async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.assign(window, { seenErrors: seen });
    window.addEventListener("error", (event) => seen.push(event.message));
  });
  const seenErrors = () => page.evaluate(() => (window as unknown as { seenErrors: string[] }).seenErrors);
  await openJourney(page, "browser", "j_launch");
  const revision = await derivedRevision(page);
  await rename(page, "n_docs", fresh("Docs"));
  await expect.poll(() => derivedRevision(page)).toBe(revision + 1);
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  expect(await seenErrors()).toEqual([]);
});

test("a rename started on one node does not follow the panel to another", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await startRename(page, "n_access", "Not the plan");
  await nodeCard(page, "n_plan").getByTestId("card-open").click();
  await expect(renameOf(page, "n_plan").getByRole("button", { name: /^Rename/ })).toBeVisible();
  await expect(renameOf(page, "n_plan").getByRole("textbox")).toHaveCount(0);
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
