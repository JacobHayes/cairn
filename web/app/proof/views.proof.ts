// The proof's media for the decision view, the timeline, and the Summary page
// (briefs/proof/5.4/prove.sh): a screenshot of each acceptance state and a short video of the
// main flow, written to CAIRN_PROOF_OUT. Each step asserts what its picture is meant to show,
// so a picture of the wrong state fails the run. Everything runs on the in-browser host,
// seeded on each load, at the fixed day the browser tests read the fixtures on.
import { join } from "node:path";

import { expect, test, type Browser, type Page } from "@playwright/test";

import { pin } from "../e2e/detail.ts";
import { goTo, nodePanel, openAt, openJourney } from "../e2e/shell.ts";
import { FIXED_TODAY } from "../e2e/views.ts";

const out = process.env["CAIRN_PROOF_OUT"] ?? "dist/proof";
const shot = (page: Page, name: string) => page.screenshot({ path: join(out, `${name}.png`), fullPage: true });
const beat = (page: Page) => page.waitForTimeout(800);

test.use({ viewport: { width: 1400, height: 900 } });

const affected = (page: Page, decision: string) => nodePanel(page, decision).getByTestId("decision-affects").getByTestId("affected");

const fixed = { fixedToday: FIXED_TODAY };

/** Revises the partner decision's answer to yes from its detail beside the decision view. */
async function partnerRunsTesting(page: Page): Promise<void> {
  const panel = nodePanel(page, "n_partner_runs");
  await panel.getByRole("button", { name: "Revise the answer" }).click();
  await panel.getByTestId("answer-editor").getByLabel("Answer").selectOption("yes");
  await panel.getByRole("button", { name: "Save the answer" }).click();
  await expect(affected(page, "n_partner_runs").first()).toHaveAttribute("data-relevance", "relevant");
}

test("the decision view, and an answer revised", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/graph?decisions=1", { node: "n_partner_runs", ...fixed });
  await expect(affected(page, "n_partner_runs")).toHaveCount(3);
  await expect(affected(page, "n_partner_runs").first()).toHaveAttribute("data-relevance", "not_relevant");
  await shot(page, "1-decision-view");
  await partnerRunsTesting(page);
  await shot(page, "2-partner-decision-revised");
});

test("the timeline: the final anchor, a shortfall and why, no final milestone", async ({ page }) => {
  await openAt(page, "browser", "j_vendor_eval", "plan/timeline", fixed);
  await expect(page.getByTestId("timeline")).toHaveAttribute("data-end", "n_decision_meeting");
  await shot(page, "3-timeline-final-anchor");
  await openAt(page, "browser", "j_launch", "plan/timeline", fixed);
  const freeze = page.locator('[data-testid="timeline-entry"][data-node="n_code_freeze"]');
  await expect(freeze).toHaveAttribute("data-shortfall", "2");
  await freeze.getByRole("button", { name: "Why" }).click();
  await expect(freeze.getByTestId("shortfall")).toBeVisible();
  await shot(page, "4-timeline-shortfall-and-why");
  await openAt(page, "browser", "j_hiring", "plan/timeline", { node: "n_offer", ...fixed });
  await pin(nodePanel(page, "n_offer"), "2026-10-20");
  await expect(page.locator('[data-testid="timeline-entry"][data-node="n_offer"]')).toBeVisible();
  await expect(page.getByTestId("timeline")).toHaveAttribute("data-end", "");
  await shot(page, "5-timeline-without-a-final-milestone");
});

test("the status summary, on screen and in print", async ({ page }) => {
  await openAt(page, "browser", "j_launch", "summary", fixed);
  await expect(page.getByTestId("card-flag-shortfall")).toContainText("2 short of days");
  await shot(page, "6-summary");
  await openAt(page, "browser", "j_bakeoff", "summary", { node: "n_winner", ...fixed });
  await expect(nodePanel(page, "n_winner")).toBeVisible();
  await page.emulateMedia({ media: "print" });
  await expect(page.getByTestId("journey-toolbar")).toBeHidden();
  await shot(page, "7-summary-printed");
});

/** The main flow: the canvas, the decision view and an answer revised, the timeline and a why, the summary. */
async function mainFlow(browser: Browser, baseURL: string): Promise<void> {
  const context = await browser.newContext({
    baseURL,
    viewport: { width: 1100, height: 680 },
    recordVideo: { dir: join(out, "video"), size: { width: 1100, height: 680 } },
  });
  const page = await context.newPage();
  await page.clock.setFixedTime(new Date(`${FIXED_TODAY}T12:00:00Z`));
  await openJourney(page, "browser", "j_vendor_eval");
  await beat(page);
  await goTo(page, "plan", "graph");
  await page.getByTestId("chip-decisions").click();
  await expect(page.getByTestId("decision-view")).toBeVisible();
  await beat(page);
  await page.locator('[data-testid="decision-view"] [data-testid="node-card"][data-node="n_partner_runs"]').getByTestId("card-open").click();
  await beat(page);
  await partnerRunsTesting(page);
  await beat(page);
  await goTo(page, "plan", "timeline");
  const opens = page.locator('[data-testid="timeline-entry"][data-node="n_review_opens"]');
  await opens.getByRole("button", { name: "Why" }).click();
  await expect(opens.getByTestId("timeline-why")).toBeVisible();
  await beat(page);
  await beat(page);
  await page.getByTestId("card-expand").click();
  await expect(page.getByTestId("summary")).toBeVisible();
  await beat(page);
  await beat(page);
  await context.close();
  await page.video()?.saveAs(join(out, "main-flow.webm"));
}

test("the main flow, on video", async ({ browser, baseURL }) => {
  await mainFlow(browser, baseURL ?? "");
});
