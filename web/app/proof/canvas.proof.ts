// The proof's media for brief 5.2 (briefs/proof/5.2/prove.sh): a screenshot of each
// acceptance state of the canvas and a short video of its main flow, written to
// CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show, so a picture of the
// wrong state fails the run. Everything runs on the in-browser host, seeded on each load.
import { join } from "node:path";

import { expect, test, type Browser, type Page } from "@playwright/test";

import { marks, showKind, toggle } from "../e2e/canvas.ts";
import { section } from "../e2e/detail.ts";
import { nodeCard, openFromCanvas, openJourney } from "../e2e/shell.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`) });
const beat = (page: Page) => page.waitForTimeout(700);

test.use({ viewport: { width: 1400, height: 900 } });

test("the whole journey, kinds hidden, drilled in", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await expect(page.getByTestId("node-card")).toHaveCount(27);
  await expect(page.getByTestId("card-rank")).toHaveCount(2);
  await shot(page, "1-whole-journey");
  await showKind(page, "action", false);
  await expect(nodeCard(page, "n_plan").getByTestId("card-checklist").locator("li")).toHaveCount(2);
  await shot(page, "2-actions-hidden-as-checklists");
  for (const kind of ["decision", "deliverable", "milestone"]) {
    await showKind(page, kind, false);
  }
  await expect(page.getByTestId("node-card")).toHaveCount(5);
  await shot(page, "3-groups-only");
  await openJourney(page, "browser", "j_vendor_eval", "?kind=decision,deliverable,action");
  await expect(nodeCard(page, "n_final_report").getByTestId("hidden-prerequisites")).toBeVisible();
  await shot(page, "4-groups-and-milestones-hidden-marker");
  await openJourney(page, "browser", "j_vendor_eval");
  await nodeCard(page, "n_setup").getByTestId("card-drill").click();
  await expect(page.getByTestId("crumb-current")).toBeVisible();
  await shot(page, "5-drilled-into-setup");
});

test("the trace, the heat overlay, and relevance", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await openFromCanvas(page, "n_plan");
  await page.getByTestId("trace-start").click();
  await expect.poll(async () => Object.keys(await marks(page)).length).toBe(15);
  await shot(page, "6-trace-of-the-test-plan");
  await openJourney(page, "browser", "j_hiring", "?lens=gravity");
  await expect(page.getByTestId("card-heat").first()).toBeVisible();
  await shot(page, "7-heat-overlay");
});

test("hiding decisions marks the work they block; the marker opens the trace", async ({ page }) => {
  await openJourney(page, "browser", "j_hiring");
  const panel = await openFromCanvas(page, "n_make_offer");
  await panel.getByTestId("actions").getByRole("button", { name: "Reopen" }).click();
  await expect(nodeCard(page, "n_offer")).toHaveAttribute("data-relevance", "undecided");
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await shot(page, "8a-undecided-ghosted");
  await showKind(page, "decision", false);
  await expect(nodeCard(page, "n_close_out").getByTestId("hidden-prerequisites")).toBeVisible();
  await shot(page, "8b-decisions-hidden-marker");
  await nodeCard(page, "n_close_out").getByTestId("hidden-prerequisites").click();
  await expect(page.getByTestId("trace-bar")).toContainText("Make an offer");
  await shot(page, "8c-marker-opened-the-trace");
});

test("the stalled surface", async ({ page }) => {
  await openJourney(page, "browser", "j_hiring");
  const panel = await openFromCanvas(page, "n_offer");
  const blocking = await section(panel, "blocking");
  await blocking.getByRole("button", { name: "Snooze until a date" }).click();
  await blocking.getByLabel("Snooze until").fill(new Date(Date.now() + 30 * 86_400_000).toISOString().slice(0, 10));
  await blocking.getByTestId("snooze").getByRole("button", { name: "Save" }).click();
  await expect(page.getByTestId("stalled")).toBeVisible();
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await shot(page, "9-stalled-surface");
});

test("a route's canvas", async ({ page }) => {
  await openJourney(page, "browser", "j_vendor_eval");
  await page.getByTestId("card-lineage").locator("summary").click();
  await page.getByTestId("lineage").click();
  await expect(page.getByTestId("node-card")).toHaveCount(25);
  await shot(page, "10-route-canvas");
});

/** The main flow: the journey, kinds hidden and shown, drill in and out, a node opened and traced. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 1100, height: 680 },
    recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 680 } },
  });
  const page = await context.newPage();
  await openJourney(page, "browser", "j_vendor_eval");
  await beat(page);
  await showKind(page, "action", false);
  await beat(page);
  await showKind(page, "decision", false);
  await beat(page);
  await toggle(page, "decision", true);
  await toggle(page, "action", true);
  await nodeCard(page, "n_setup").getByTestId("card-drill").click();
  await beat(page);
  await page.getByTestId("crumbs").getByRole("link", { name: "Whole journey" }).click();
  await beat(page);
  await openFromCanvas(page, "n_plan");
  await beat(page);
  await page.getByTestId("trace-start").click();
  await expect.poll(async () => Object.keys(await marks(page)).length).toBe(15);
  await beat(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
