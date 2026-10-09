// The proof's pictures for brief 8.7 (briefs/proof/8.7/README.md): the inspector in each state its
// brief names, light, 1440px, on the in-browser host. Steps only navigate and wait for the state
// they picture; the e2e specs assert.
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

import { startJourney } from "../e2e/around.ts";
import { section } from "../e2e/detail.ts";
import { fresh, menuItem, nodePanel, openAt } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
test.use({ viewport: { width: 1440, height: 900 } });

/** The inspector column as a person sees it, once the panel for `node` is up. */
async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(300);
  await page.locator(".inspector").screenshot({ path: join(out, `${name}.png`) });
}

async function show(page: Page, journey: string, node: string): Promise<Locator> {
  await openAt(page, "browser", journey, "plan/graph", { node });
  const panel = nodePanel(page, node);
  await expect(panel).toBeVisible();
  return panel;
}

test("a decision to answer: nothing preselected, then each choice's effect and the preview", async ({ page }) => {
  const journey = await startJourney(page, "browser", fresh("Proof"), { route: "vendor-evaluation", version: 1 });
  const panel = await show(page, journey, "n_partner_runs");
  await shot(page, "decision-ready");
  await panel.getByRole("radio", { name: /^yes\b/i }).check();
  await expect(panel.getByTestId("preview")).toBeVisible();
  await shot(page, "decision-picked");
  await menuItem(panel, "snooze");
  await expect(panel.getByTestId("snooze")).toBeVisible();
  await shot(page, "snooze-chooser");
});

test("a decided decision, a blocked one, and one that does not apply", async ({ page }) => {
  await show(page, "j_hiring", "n_make_offer");
  await shot(page, "decision-decided");
  await show(page, "j_bakeoff", "n_winner");
  await shot(page, "decision-blocked");
  await show(page, "j_vendor_eval", "n_partner_results");
  await shot(page, "not-relevant");
});

test("a deliverable, a note required, a container with a snooze, a milestone", async ({ page }) => {
  const hiring = await startJourney(page, "browser", fresh("Proof"), { route: "hiring-loop", version: 1 });
  const screen = await show(page, hiring, "n_screen");
  await screen.getByRole("button", { name: "Done...", exact: true }).click();
  await expect(screen.getByLabel("Note")).toBeVisible();
  await shot(page, "note-required");
  const vendor = await startJourney(page, "browser", fresh("Proof"), { route: "vendor-evaluation", version: 1 });
  const plan = await show(page, vendor, "n_plan");
  await menuItem(plan, "snooze");
  await plan.getByTestId("snooze").getByRole("button", { name: "Snooze", exact: true }).click();
  await expect(plan.getByTestId("detail-sentence")).toContainText("Snoozed");
  await shot(page, "deliverable-snoozed");
  const setup = await show(page, vendor, "n_setup");
  await menuItem(setup, "snooze");
  await setup.getByTestId("snooze").getByRole("button", { name: "Snooze", exact: true }).click();
  await expect(setup.getByTestId("detail-sentence")).toContainText("Snoozed");
  await shot(page, "container-snoozed");
  await show(page, "j_vendor_eval", "n_final_report");
  await shot(page, "deliverable");
  await show(page, "j_vendor_eval", "n_decision_meeting");
  await shot(page, "milestone");
});

test("why this rank, a container's gravity, and the edge card", async ({ page }) => {
  const meeting = await show(page, "j_vendor_eval", "n_decision_meeting");
  await (await section(meeting, "rank")).scrollIntoViewIfNeeded();
  await shot(page, "why-rank");
  const reporting = await show(page, "j_vendor_eval", "n_reporting");
  await (await section(reporting, "rank")).scrollIntoViewIfNeeded();
  await shot(page, "container");
  await openAt(page, "browser", "j_vendor_eval", "plan/graph/edges/n_access~n_plan");
  await expect(page.getByTestId("edge-card")).toBeVisible();
  await shot(page, "edge-card");
});

test("the overflow menu on a tablet's sheet opens upward when there is no room below", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 700 });
  const panel = await show(page, "j_vendor_eval", "n_final_report");
  await panel.getByTestId("more-actions").click();
  await expect(panel.getByTestId("more-actions-menu")).toBeVisible();
  await shot(page, "menu-sheet");
});
